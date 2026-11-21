//! The native apps' storage (Android and desktop): one SQLite file. Its layout is the one Android's
//! Room database has always used, so this opens the Android app's existing file (Room versions 3
//! and 4) and the desktop app's (versions 0 to 2), upgrading either in place.
//!
//! The engine keeps the whole Memi in memory; [SqliteStore::save] writes only the records that
//! changed, matched by uid. Triggers also keep uids, edit times and tombstones right for any
//! write that doesn't set them itself.

use anyhow::Result;
use memi_core::model::*;
use rusqlite::{params, Connection, Row};
use std::collections::HashMap;
use std::path::Path;

pub struct SqliteStore {
    db: Connection,
    /// What's in the file now, by uid, so saving can skip what hasn't changed.
    saved: Snapshot,
    /// Row ids in the file by uid (people, categories, notes), kept between saves instead of
    /// being read back each time. None until the first save, and after a save fails.
    rows: Option<[HashMap<String, i64>; 3]>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS people (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    relationship TEXT NOT NULL DEFAULT '',
    photoPath TEXT,
    birthdayMonth INTEGER,
    birthdayDay INTEGER,
    birthYear INTEGER,
    toneIndex INTEGER NOT NULL DEFAULT 0,
    pinned INTEGER NOT NULL DEFAULT 0,
    sortOrder INTEGER NOT NULL DEFAULT 0,
    createdAt INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    personId INTEGER,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    slot INTEGER NOT NULL,
    sortOrder INTEGER NOT NULL,
    occasion TEXT NOT NULL DEFAULT 'NONE',
    occasionDay INTEGER,
    yearly INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS index_collections_personId ON collections(personId);
CREATE TABLE IF NOT EXISTS notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    personId INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    collectionId INTEGER NOT NULL,
    text TEXT NOT NULL,
    mentionedAt INTEGER NOT NULL,
    firstMentionedAt INTEGER NOT NULL,
    mentionCount INTEGER NOT NULL DEFAULT 1,
    dueAt INTEGER,
    status INTEGER NOT NULL DEFAULT 0,
    outcome INTEGER NOT NULL DEFAULT 0,
    doneAt INTEGER,
    createdAt INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS index_notes_personId ON notes(personId);
CREATE INDEX IF NOT EXISTS index_notes_collectionId ON notes(collectionId);
"#;

/// Version 2 (Memi 0.6): uids, edit times, note order, tombstones and the triggers that keep them.
/// The same SQL runs on Android (platform/AndroidDatabase.kt).
const NOW: &str = "CAST(ROUND((julianday('now') - 2440587.5) * 86400000) AS INTEGER)";

fn sync_schema() -> String {
    let mut sql = String::from(
        "CREATE TABLE IF NOT EXISTS tombstones (uid TEXT NOT NULL PRIMARY KEY, deletedAt INTEGER NOT NULL);\n",
    );
    for t in ["people", "collections", "notes"] {
        sql += &format!(
            r#"
CREATE UNIQUE INDEX IF NOT EXISTS index_{t}_uid ON {t}(uid) WHERE uid != '';
-- A new row gets a uid and an edit time unless it came with them (from a merge).
CREATE TRIGGER IF NOT EXISTS {t}_stamp_insert AFTER INSERT ON {t}
WHEN NEW.uid = '' OR NEW.updatedAt = 0 BEGIN
    UPDATE {t} SET
        uid = CASE WHEN NEW.uid = '' THEN lower(hex(randomblob(16))) ELSE NEW.uid END,
        updatedAt = CASE WHEN NEW.updatedAt = 0 THEN {NOW} ELSE NEW.updatedAt END
    WHERE id = NEW.id;
END;
-- Something brought back (Undo, or edited elsewhere after a deletion) is no longer deleted.
CREATE TRIGGER IF NOT EXISTS {t}_revive AFTER INSERT ON {t} BEGIN
    DELETE FROM tombstones WHERE uid = (SELECT uid FROM {t} WHERE id = NEW.id);
END;
-- Any change made here moves the edit time on, unless the change set it (a merge does).
CREATE TRIGGER IF NOT EXISTS {t}_stamp_update AFTER UPDATE ON {t}
WHEN NEW.updatedAt = OLD.updatedAt BEGIN
    UPDATE {t} SET updatedAt = {NOW} WHERE id = NEW.id;
END;
CREATE TRIGGER IF NOT EXISTS {t}_tombstone AFTER DELETE ON {t}
WHEN OLD.uid != '' BEGIN
    INSERT OR REPLACE INTO tombstones (uid, deletedAt) VALUES (OLD.uid, {NOW});
END;
"#
        );
    }
    sql
}

fn person(r: &Row) -> rusqlite::Result<Person> {
    Ok(Person {
        id: r.get("id")?,
        uid: r.get("uid")?,
        name: r.get("name")?,
        relationship: r.get("relationship")?,
        photo_path: r.get("photoPath")?,
        birthday_month: r.get("birthdayMonth")?,
        birthday_day: r.get("birthdayDay")?,
        birth_year: r.get("birthYear")?,
        tone_index: r.get("toneIndex")?,
        pinned: r.get("pinned")?,
        sort_order: r.get("sortOrder")?,
        created_at: r.get("createdAt")?,
        updated_at: r.get("updatedAt")?,
    })
}

fn collection(r: &Row) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: r.get("id")?,
        uid: r.get("uid")?,
        person_id: r.get("personId")?,
        person_uid: r.get("personUid")?,
        name: r.get("name")?,
        kind: r.get("kind")?,
        slot: r.get("slot")?,
        sort_order: r.get("sortOrder")?,
        occasion: r.get("occasion")?,
        occasion_day: r.get("occasionDay")?,
        yearly: r.get("yearly")?,
        updated_at: r.get("updatedAt")?,
    })
}

fn note(r: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get("id")?,
        uid: r.get("uid")?,
        person_id: r.get("personId")?,
        person_uid: r.get::<_, Option<String>>("personUid")?.unwrap_or_default(),
        collection_id: r.get("collectionId")?,
        collection_uid: r.get::<_, Option<String>>("collectionUid")?.unwrap_or_default(),
        text: r.get("text")?,
        mentioned_at: r.get("mentionedAt")?,
        first_mentioned_at: r.get("firstMentionedAt")?,
        mention_count: r.get("mentionCount")?,
        due_at: r.get("dueAt")?,
        status: r.get("status")?,
        outcome: r.get("outcome")?,
        done_at: r.get("doneAt")?,
        created_at: r.get("createdAt")?,
        updated_at: r.get("updatedAt")?,
        sort_order: r.get("sortOrder")?,
    })
}

const COLLECTIONS: &str = "SELECT c.*, p.uid AS personUid FROM collections c LEFT JOIN people p ON p.id = c.personId";
const NOTES: &str = "SELECT n.*, p.uid AS personUid, c.uid AS collectionUid FROM notes n
    LEFT JOIN people p ON p.id = n.personId LEFT JOIN collections c ON c.id = n.collectionId";

impl SqliteStore {
    /// Keeps a copy of the database at `path` as it is now, before anything in it changes (for
    /// the first time a new version opens it). Includes anything still in the write-ahead log.
    /// Does nothing if there's no database yet or the copy already exists.
    pub fn copy_untouched(path: &Path, to: &Path) -> Result<()> {
        if !path.exists() || to.exists() { return Ok(()); }
        let db = Connection::open(path)?;
        db.execute("VACUUM INTO ?", [to.to_string_lossy()])?;
        Ok(())
    }

    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "foreign_keys", true)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        let mut s = SqliteStore { db, saved: Snapshot::default(), rows: None };
        s.prepare()?;
        s.saved = s.snapshot()?;
        Ok(s)
    }

    pub fn in_memory() -> Result<Self> {
        let db = Connection::open_in_memory()?;
        db.pragma_update(None, "foreign_keys", true)?;
        let mut s = SqliteStore { db, saved: Snapshot::default(), rows: None };
        s.prepare()?;
        Ok(s)
    }

    /// Creates the tables, and upgrades a database from Memi 0.5 in place.
    /// Files from the first Android versions (Room versions 1 and 2), upgraded the way Room's
    /// migrations did: people gain pinning and their own order (1 → 2), then categories become per
    /// person (2 → 3), each shared category copied to everyone along with their notes in it, and
    /// Gift ideas building up to the person's birthday.
    fn upgrade_oldest(&mut self) -> Result<()> {
        let column = |db: &Connection, t: &str, c: &str| -> Result<bool> {
            Ok(db.query_row(&format!("SELECT COUNT(*) FROM pragma_table_info('{t}') WHERE name = ?"), [c], |r| r.get::<_, i64>(0))? > 0)
        };
        let table = |db: &Connection, t: &str| -> Result<bool> {
            Ok(db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?", [t], |r| r.get::<_, i64>(0))? > 0)
        };
        if !table(&self.db, "people")? || !table(&self.db, "collections")? { return Ok(()); }
        let tx = self.db.transaction()?;
        if !column(&tx, "people", "pinned")? {
            tx.execute_batch("ALTER TABLE people ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
                              ALTER TABLE people ADD COLUMN sortOrder INTEGER NOT NULL DEFAULT 0;")?;
        }
        if !column(&tx, "collections", "personId")? {
            tx.execute_batch("ALTER TABLE collections ADD COLUMN personId INTEGER;
                              ALTER TABLE collections ADD COLUMN occasion TEXT NOT NULL DEFAULT 'NONE';
                              ALTER TABLE collections ADD COLUMN occasionDay INTEGER;
                              ALTER TABLE collections ADD COLUMN yearly INTEGER NOT NULL DEFAULT 0;
                              CREATE INDEX IF NOT EXISTS index_collections_personId ON collections (personId);
                              UPDATE collections SET occasion = 'BIRTHDAY' WHERE kind = 'GIFT';")?;
            let shared: Vec<(i64, String, String, i64, i64, String)> = tx
                .prepare("SELECT id, name, kind, slot, sortOrder, occasion FROM collections WHERE personId IS NULL")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
                .collect::<rusqlite::Result<_>>()?;
            let people: Vec<i64> = tx.prepare("SELECT id FROM people")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            for p in &people {
                for (id, name, kind, slot, order, occasion) in &shared {
                    tx.execute(
                        "INSERT INTO collections (personId, name, kind, slot, sortOrder, occasion, yearly) VALUES (?, ?, ?, ?, ?, ?, 0)",
                        params![p, name, kind, slot, order, occasion],
                    )?;
                    let new_id = tx.last_insert_rowid();
                    tx.execute("UPDATE notes SET collectionId = ? WHERE personId = ? AND collectionId = ?", params![new_id, p, id])?;
                }
            }
            tx.execute("DELETE FROM collections WHERE personId IS NULL", [])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn prepare(&mut self) -> Result<()> {
        self.upgrade_oldest()?;
        self.db.execute_batch(SCHEMA)?;
        let version: i64 = self.db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        // Room's files carry Room's own version numbers (3 and 4), so check the columns themselves.
        {
            let tx = self.db.transaction()?;
            for t in ["people", "collections", "notes"] {
                let has: bool = tx.query_row(&format!("SELECT COUNT(*) FROM pragma_table_info('{t}') WHERE name = 'uid'"), [], |r| r.get::<_, i64>(0))? > 0;
                if !has {
                    tx.execute_batch(&format!(
                        "ALTER TABLE {t} ADD COLUMN uid TEXT NOT NULL DEFAULT '';
                         ALTER TABLE {t} ADD COLUMN updatedAt INTEGER NOT NULL DEFAULT 0;"
                    ))?;
                }
            }
            let has_order: bool = tx.query_row("SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'sortOrder'", [], |r| r.get::<_, i64>(0))? > 0;
            if !has_order {
                tx.execute_batch("ALTER TABLE notes ADD COLUMN sortOrder INTEGER NOT NULL DEFAULT 0;
                                  UPDATE notes SET sortOrder = -mentionedAt;")?;
            }
            // Existing records get the uids every other upgraded device gives them.
            let people: Vec<(i64, i64)> = tx.prepare("SELECT id, createdAt FROM people WHERE uid = ''")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            for (id, created) in people {
                tx.execute("UPDATE people SET uid = ? WHERE id = ?", params![legacy_uid::person(id, created), id])?;
            }
            let cols: Vec<(i64, Option<i64>, Option<i64>)> = tx.prepare(
                "SELECT c.id, c.personId, p.createdAt FROM collections c LEFT JOIN people p ON p.id = c.personId WHERE c.uid = ''")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
            for (id, pid, pcreated) in cols {
                tx.execute("UPDATE collections SET uid = ? WHERE id = ?", params![legacy_uid::collection(id, pid, pcreated), id])?;
            }
            let notes: Vec<(i64, i64)> = tx.prepare("SELECT id, createdAt FROM notes WHERE uid = ''")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            for (id, created) in notes {
                tx.execute("UPDATE notes SET uid = ? WHERE id = ?", params![legacy_uid::note(id, created), id])?;
            }
            tx.execute_batch("CREATE TABLE IF NOT EXISTS tombstones (uid TEXT NOT NULL PRIMARY KEY, deletedAt INTEGER NOT NULL);")?;
            tx.execute_batch(&sync_schema())?;
            if version < 2 { tx.pragma_update(None, "user_version", 2)?; }
            tx.commit()?;
        }
        Ok(())
    }

    // --- Reading -----------------------------------------------------------------------------

    pub fn people(&self) -> Result<Vec<Person>> {
        let mut st = self.db.prepare("SELECT * FROM people ORDER BY pinned DESC, sortOrder, name COLLATE NOCASE")?;
        let rows = st.query_map([], person)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut st = self.db.prepare(&format!("{COLLECTIONS} ORDER BY c.personId, c.sortOrder"))?;
        let rows = st.query_map([], collection)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Notes in your order: within a category, as you've arranged them (newest first by default).
    pub fn notes(&self) -> Result<Vec<Note>> {
        let mut st = self.db.prepare(&format!("{NOTES} ORDER BY n.sortOrder, n.mentionedAt DESC"))?;
        let rows = st.query_map([], note)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn tombstones(&self) -> Result<Vec<Tombstone>> {
        let mut st = self.db.prepare("SELECT uid, deletedAt FROM tombstones")?;
        let rows = st.query_map([], |r| Ok(Tombstone { uid: r.get(0)?, deleted_at: r.get(1)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Everything, for a backup or a merge.
    pub fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot { people: self.people()?, collections: self.collections()?, notes: self.notes()?, tombstones: self.tombstones()? })
    }

    // --- People ------------------------------------------------------------------------------

    /// Adds someone with the three starter categories; Gift ideas builds up to their birthday.
    /// What the store holds, for starting the engine.
    pub fn load(&self) -> Snapshot { self.saved.clone() }

    /// Writes `s`, touching only the records that differ from what's in the file.
    pub fn write(&mut self, s: &Snapshot) -> Result<()> {
        let rows = match self.rows.take() { Some(r) => r, None => self.row_ids()? };
        let rows = self.write_with(s, rows)?;
        self.rows = Some(rows);
        self.saved = s.clone();
        Ok(())
    }

    fn row_ids(&self) -> Result<[HashMap<String, i64>; 3]> {
        let read = |table: &str| -> Result<HashMap<String, i64>> {
            let mut st = self.db.prepare(&format!("SELECT uid, id FROM {table}"))?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
        };
        Ok([read("people")?, read("collections")?, read("notes")?])
    }

    fn write_with(&mut self, s: &Snapshot, rows: [HashMap<String, i64>; 3]) -> Result<[HashMap<String, i64>; 3]> {
        let [mut person_id, mut col_id, mut note_id] = rows;
        let mut inserted = false;
        let before = &self.saved;
        let old_people: HashMap<&str, &Person> = before.people.iter().map(|p| (p.uid.as_str(), p)).collect();
        let old_cols: HashMap<&str, &Collection> = before.collections.iter().map(|c| (c.uid.as_str(), c)).collect();
        let old_notes: HashMap<&str, &Note> = before.notes.iter().map(|n| (n.uid.as_str(), n)).collect();
        let tx = self.db.transaction()?;

        // Gone: delete (the tombstones are rewritten below).
        let keep: std::collections::HashSet<&str> = s.people.iter().map(|p| p.uid.as_str())
            .chain(s.collections.iter().map(|c| c.uid.as_str()))
            .chain(s.notes.iter().map(|n| n.uid.as_str())).collect();
        let mut deleted = false;
        for n in before.notes.iter().filter(|n| !keep.contains(n.uid.as_str())) { tx.prepare_cached("DELETE FROM notes WHERE uid = ?")?.execute([&n.uid])?; note_id.remove(&n.uid); deleted = true; }
        for c in before.collections.iter().filter(|c| !keep.contains(c.uid.as_str())) { tx.prepare_cached("DELETE FROM collections WHERE uid = ?")?.execute([&c.uid])?; col_id.remove(&c.uid); deleted = true; }
        for p in before.people.iter().filter(|p| !keep.contains(p.uid.as_str())) { tx.prepare_cached("DELETE FROM people WHERE uid = ?")?.execute([&p.uid])?; person_id.remove(&p.uid); deleted = true; }

        // Row ids in the file (the engine's own ids may differ for new records) come from `rows`.
        for p in &s.people {
            if old_people.get(p.uid.as_str()).is_some_and(|o| same_person(o, p)) { continue; }
            match person_id.get(&p.uid) {
                Some(id) => {
                    tx.prepare_cached(
                        "UPDATE people SET name=?, relationship=?, photoPath=?, birthdayMonth=?, birthdayDay=?, birthYear=?, toneIndex=?, pinned=?, sortOrder=?, createdAt=?, updatedAt=? WHERE id=?",
                    )?.execute(params![p.name, p.relationship, p.photo_path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.created_at, p.updated_at, id])?;
                    // If the edit time didn't change, the trigger stamped one: put it back.
                    tx.prepare_cached("UPDATE people SET updatedAt = ? WHERE id = ? AND updatedAt != ?")?.execute(params![p.updated_at, id, p.updated_at])?;
                }
                None => {
                    tx.prepare_cached(
                        "INSERT INTO people (uid, name, relationship, photoPath, birthdayMonth, birthdayDay, birthYear, toneIndex, pinned, sortOrder, createdAt, updatedAt)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )?.execute(params![p.uid, p.name, p.relationship, p.photo_path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.created_at, p.updated_at])?;
                    person_id.insert(p.uid.clone(), tx.last_insert_rowid());
                    inserted = true;
                }
            }
        }

        for c in &s.collections {
            let Some(pid) = c.person_uid.as_ref().and_then(|u| person_id.get(u)).copied() else { continue };
            if old_cols.get(c.uid.as_str()).is_some_and(|o| same_collection(o, c)) { continue; }
            match col_id.get(&c.uid) {
                Some(id) => {
                    tx.prepare_cached(
                        "UPDATE collections SET personId=?, name=?, kind=?, slot=?, sortOrder=?, occasion=?, occasionDay=?, yearly=?, updatedAt=? WHERE id=?",
                    )?.execute(params![pid, c.name, c.kind, c.slot, c.sort_order, c.occasion, c.occasion_day, c.yearly, c.updated_at, id])?;
                    tx.prepare_cached("UPDATE collections SET updatedAt = ? WHERE id = ? AND updatedAt != ?")?.execute(params![c.updated_at, id, c.updated_at])?;
                }
                None => {
                    tx.prepare_cached(
                        "INSERT INTO collections (uid, personId, name, kind, slot, sortOrder, occasion, occasionDay, yearly, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )?.execute(params![c.uid, pid, c.name, c.kind, c.slot, c.sort_order, c.occasion, c.occasion_day, c.yearly, c.updated_at])?;
                    col_id.insert(c.uid.clone(), tx.last_insert_rowid());
                    inserted = true;
                }
            }
        }

        for n in &s.notes {
            let (Some(pid), Some(cid)) = (person_id.get(&n.person_uid).copied(), col_id.get(&n.collection_uid).copied()) else { continue };
            if old_notes.get(n.uid.as_str()).is_some_and(|o| same_note(o, n)) { continue; }
            match note_id.get(&n.uid) {
                Some(id) => {
                    tx.prepare_cached(
                        "UPDATE notes SET personId=?, collectionId=?, text=?, mentionedAt=?, firstMentionedAt=?, mentionCount=?, dueAt=?, status=?, outcome=?, doneAt=?, createdAt=?, sortOrder=?, updatedAt=? WHERE id=?",
                    )?.execute(params![pid, cid, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at, n.sort_order, n.updated_at, id])?;
                    // If only the category changed, the edit time didn't, and the trigger stamped it: put it back.
                    tx.prepare_cached("UPDATE notes SET updatedAt = ? WHERE id = ? AND updatedAt != ?")?.execute(params![n.updated_at, id, n.updated_at])?;
                }
                None => {
                    tx.prepare_cached(
                        "INSERT INTO notes (uid, personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, doneAt, createdAt, sortOrder, updatedAt)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )?.execute(params![n.uid, pid, cid, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at, n.sort_order, n.updated_at])?;
                    note_id.insert(n.uid.clone(), tx.last_insert_rowid());
                    inserted = true;
                }
            }
        }
        // Deleting rows adds tombstones by trigger, and inserting revives them, so rewrite the
        // list whenever either could have touched it.
        if deleted || inserted || s.tombstones != before.tombstones {
            tx.execute("DELETE FROM tombstones", [])?;
            for t in &s.tombstones {
                tx.prepare_cached("INSERT OR REPLACE INTO tombstones (uid, deletedAt) VALUES (?, ?)")?.execute(params![t.uid, t.deleted_at])?;
            }
        }
        tx.commit()?;
        Ok([person_id, col_id, note_id])
    }
}

// Whether a record needs writing: every field but the row ids, which mean nothing in the file.
// Destructured in full, so a new field can't be forgotten.
fn same_person(a: &Person, b: &Person) -> bool {
    let Person { id: _, uid, name, relationship, photo_path, birthday_month, birthday_day, birth_year, tone_index, pinned, sort_order, created_at, updated_at } = a;
    *uid == b.uid && *name == b.name && *relationship == b.relationship && *photo_path == b.photo_path && *birthday_month == b.birthday_month
        && *birthday_day == b.birthday_day && *birth_year == b.birth_year && *tone_index == b.tone_index && *pinned == b.pinned
        && *sort_order == b.sort_order && *created_at == b.created_at && *updated_at == b.updated_at
}

fn same_collection(a: &Collection, b: &Collection) -> bool {
    let Collection { id: _, uid, person_id: _, person_uid, name, kind, slot, sort_order, occasion, occasion_day, yearly, updated_at } = a;
    *uid == b.uid && *person_uid == b.person_uid && *name == b.name && *kind == b.kind && *slot == b.slot && *sort_order == b.sort_order
        && *occasion == b.occasion && *occasion_day == b.occasion_day && *yearly == b.yearly && *updated_at == b.updated_at
}

fn same_note(a: &Note, b: &Note) -> bool {
    let Note { id: _, uid, person_id: _, person_uid, collection_id: _, collection_uid, text, mentioned_at, first_mentioned_at, mention_count,
        due_at, status, outcome, done_at, created_at, updated_at, sort_order } = a;
    *uid == b.uid && *person_uid == b.person_uid && *collection_uid == b.collection_uid && *text == b.text && *mentioned_at == b.mentioned_at
        && *first_mentioned_at == b.first_mentioned_at && *mention_count == b.mention_count && *due_at == b.due_at && *status == b.status
        && *outcome == b.outcome && *done_at == b.done_at && *created_at == b.created_at && *updated_at == b.updated_at && *sort_order == b.sort_order
}
