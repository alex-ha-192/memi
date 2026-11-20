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
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "foreign_keys", true)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        let mut s = SqliteStore { db, saved: Snapshot::default() };
        s.prepare()?;
        s.saved = s.snapshot()?;
        Ok(s)
    }

    pub fn in_memory() -> Result<Self> {
        let db = Connection::open_in_memory()?;
        db.pragma_update(None, "foreign_keys", true)?;
        let mut s = SqliteStore { db, saved: Snapshot::default() };
        s.prepare()?;
        Ok(s)
    }

    /// Creates the tables, and upgrades a database from Memi 0.5 in place.
    fn prepare(&mut self) -> Result<()> {
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
        let before = &self.saved;
        let old_people: HashMap<&str, &Person> = before.people.iter().map(|p| (p.uid.as_str(), p)).collect();
        let old_cols: HashMap<&str, &Collection> = before.collections.iter().map(|c| (c.uid.as_str(), c)).collect();
        let old_notes: HashMap<&str, &Note> = before.notes.iter().map(|n| (n.uid.as_str(), n)).collect();
        let tx = self.db.transaction()?;

        // Gone: delete (the tombstones are rewritten below).
        let keep: std::collections::HashSet<&str> = s.people.iter().map(|p| p.uid.as_str())
            .chain(s.collections.iter().map(|c| c.uid.as_str()))
            .chain(s.notes.iter().map(|n| n.uid.as_str())).collect();
        for n in before.notes.iter().filter(|n| !keep.contains(n.uid.as_str())) { tx.execute("DELETE FROM notes WHERE uid = ?", [&n.uid])?; }
        for c in before.collections.iter().filter(|c| !keep.contains(c.uid.as_str())) { tx.execute("DELETE FROM collections WHERE uid = ?", [&c.uid])?; }
        for p in before.people.iter().filter(|p| !keep.contains(p.uid.as_str())) { tx.execute("DELETE FROM people WHERE uid = ?", [&p.uid])?; }

        // Row ids in the file, by uid (the engine's own ids may differ for new records).
        let row_ids = |tx: &rusqlite::Transaction, table: &str| -> Result<HashMap<String, i64>> {
            let mut st = tx.prepare(&format!("SELECT uid, id FROM {table}"))?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
        };

        let mut person_id = row_ids(&tx, "people")?;
        for p in &s.people {
            if old_people.get(p.uid.as_str()).is_some_and(|o| **o == Person { id: o.id, ..p.clone() }) { continue; }
            match person_id.get(&p.uid) {
                Some(id) => {
                    tx.execute(
                        "UPDATE people SET name=?, relationship=?, photoPath=?, birthdayMonth=?, birthdayDay=?, birthYear=?, toneIndex=?, pinned=?, sortOrder=?, createdAt=?, updatedAt=? WHERE id=?",
                        params![p.name, p.relationship, p.photo_path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.created_at, p.updated_at, id],
                    )?;
                }
                None => {
                    tx.execute(
                        "INSERT INTO people (uid, name, relationship, photoPath, birthdayMonth, birthdayDay, birthYear, toneIndex, pinned, sortOrder, createdAt, updatedAt)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![p.uid, p.name, p.relationship, p.photo_path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.created_at, p.updated_at],
                    )?;
                    person_id.insert(p.uid.clone(), tx.last_insert_rowid());
                }
            }
        }

        let mut col_id = row_ids(&tx, "collections")?;
        for c in &s.collections {
            let Some(pid) = c.person_uid.as_ref().and_then(|u| person_id.get(u)).copied() else { continue };
            if old_cols.get(c.uid.as_str()).is_some_and(|o| o.updated_at == c.updated_at && o.person_uid == c.person_uid && **o == Collection { id: o.id, person_id: o.person_id, ..c.clone() }) { continue; }
            match col_id.get(&c.uid) {
                Some(id) => {
                    tx.execute(
                        "UPDATE collections SET personId=?, name=?, kind=?, slot=?, sortOrder=?, occasion=?, occasionDay=?, yearly=?, updatedAt=? WHERE id=?",
                        params![pid, c.name, c.kind, c.slot, c.sort_order, c.occasion, c.occasion_day, c.yearly, c.updated_at, id],
                    )?;
                }
                None => {
                    tx.execute(
                        "INSERT INTO collections (uid, personId, name, kind, slot, sortOrder, occasion, occasionDay, yearly, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![c.uid, pid, c.name, c.kind, c.slot, c.sort_order, c.occasion, c.occasion_day, c.yearly, c.updated_at],
                    )?;
                    col_id.insert(c.uid.clone(), tx.last_insert_rowid());
                }
            }
        }

        let note_id = row_ids(&tx, "notes")?;
        for n in &s.notes {
            let (Some(pid), Some(cid)) = (person_id.get(&n.person_uid).copied(), col_id.get(&n.collection_uid).copied()) else { continue };
            if old_notes.get(n.uid.as_str()).is_some_and(|o| **o == Note { id: o.id, person_id: o.person_id, collection_id: o.collection_id, ..n.clone() }) { continue; }
            match note_id.get(&n.uid) {
                Some(id) => {
                    tx.execute(
                        "UPDATE notes SET personId=?, collectionId=?, text=?, mentionedAt=?, firstMentionedAt=?, mentionCount=?, dueAt=?, status=?, outcome=?, doneAt=?, createdAt=?, sortOrder=?, updatedAt=? WHERE id=?",
                        params![pid, cid, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at, n.sort_order, n.updated_at, id],
                    )?;
                    // If only the category changed, the edit time didn't, and the trigger stamped it: put it back.
                    tx.execute("UPDATE notes SET updatedAt = ? WHERE id = ? AND updatedAt != ?", params![n.updated_at, id, n.updated_at])?;
                }
                None => {
                    tx.execute(
                        "INSERT INTO notes (uid, personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, doneAt, createdAt, sortOrder, updatedAt)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![n.uid, pid, cid, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at, n.sort_order, n.updated_at],
                    )?;
                }
            }
        }
        // The same goes for people and categories whose only change was to be stamped by a trigger.
        for p in &s.people { tx.execute("UPDATE people SET updatedAt = ? WHERE uid = ? AND updatedAt != ?", params![p.updated_at, p.uid, p.updated_at])?; }
        for c in &s.collections { tx.execute("UPDATE collections SET updatedAt = ? WHERE uid = ? AND updatedAt != ?", params![c.updated_at, c.uid, c.updated_at])?; }

        tx.execute("DELETE FROM tombstones", [])?;
        for t in &s.tombstones {
            tx.execute("INSERT OR REPLACE INTO tombstones (uid, deletedAt) VALUES (?, ?)", params![t.uid, t.deleted_at])?;
        }
        tx.commit()?;
        self.saved = s.clone();
        Ok(())
    }
}
