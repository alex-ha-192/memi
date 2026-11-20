//! Everything Memi keeps on this computer, in one SQLite file in its own data folder.
//! Mirrors the Android Repository: same rules for ordering, starter categories and deletion.
//!
//! Every row has a `uid` (the same on every device) and an `updatedAt` time, both kept up to date
//! by triggers, so no write here has to remember them. Deleting a row leaves a tombstone, so the
//! deletion reaches your other devices when your Memi syncs. See memi_core::merge.

use crate::model::*;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct Store {
    db: Connection,
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

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "foreign_keys", true)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        let mut s = Store { db };
        s.prepare()?;
        Ok(s)
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        let db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "foreign_keys", true).unwrap();
        let mut s = Store { db };
        s.prepare().unwrap();
        s
    }

    /// Creates the tables, and upgrades a database from Memi 0.5 in place.
    fn prepare(&mut self) -> Result<()> {
        self.db.execute_batch(SCHEMA)?;
        let version: i64 = self.db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < 2 {
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
            tx.execute_batch(&sync_schema())?;
            tx.pragma_update(None, "user_version", 2)?;
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

    fn collections_for(&self, person_id: i64) -> Result<Vec<Collection>> {
        let mut st = self.db.prepare(&format!("{COLLECTIONS} WHERE c.personId = ? ORDER BY c.sortOrder"))?;
        let rows = st.query_map([person_id], collection)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn person(&self, id: i64) -> Result<Option<Person>> {
        Ok(self.db.query_row("SELECT * FROM people WHERE id = ?", [id], person).optional()?)
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
    pub fn add_person(&mut self, p: &Person) -> Result<i64> {
        let tx = self.db.transaction()?;
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM people", [], |r| r.get(0))?;
        let max_order: i64 = tx.query_row("SELECT COALESCE(MAX(sortOrder), 0) FROM people", [], |r| r.get(0))?;
        tx.execute(
            "INSERT INTO people (name, relationship, photoPath, birthdayMonth, birthdayDay, birthYear, toneIndex, pinned, sortOrder, createdAt)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![p.name.trim(), p.relationship.trim(), p.photo_path, p.birthday_month, p.birthday_day, p.birth_year, count % 8, p.pinned, max_order + 1, p.created_at],
        )?;
        let id = tx.last_insert_rowid();
        for (name, kind, slot, occ) in [
            ("Gift ideas", kind::GIFT, 0, occasion::BIRTHDAY),
            ("Plans", kind::PLAN, 1, occasion::NONE),
            ("Notes", kind::NOTE, 2, occasion::NONE),
        ] {
            tx.execute(
                "INSERT INTO collections (personId, name, kind, slot, sortOrder, occasion, yearly) VALUES (?, ?, ?, ?, ?, ?, 0)",
                params![id, name, kind, slot, slot, occ],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn update_person(&self, p: &Person) -> Result<()> {
        self.db.execute(
            "UPDATE people SET name=?, relationship=?, photoPath=?, birthdayMonth=?, birthdayDay=?, birthYear=?, toneIndex=?, pinned=?, sortOrder=? WHERE id=?",
            params![p.name.trim(), p.relationship.trim(), p.photo_path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.id],
        )?;
        Ok(())
    }

    /// Removes someone with their categories and notes (their photo is deleted by the caller).
    pub fn delete_person(&mut self, id: i64) -> Result<()> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM notes WHERE personId = ?", [id])?;
        tx.execute("DELETE FROM collections WHERE personId = ?", [id])?;
        tx.execute("DELETE FROM people WHERE id = ?", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        self.db.execute("UPDATE people SET pinned = ? WHERE id = ?", params![pinned, id])?;
        Ok(())
    }

    /// Moves someone one place earlier (-1) or later (+1) among the pinned or unpinned group.
    pub fn move_person(&mut self, id: i64, delta: i32) -> Result<()> {
        let all = self.people()?;
        let Some(me) = all.iter().find(|p| p.id == id).cloned() else { return Ok(()) };
        let group: Vec<i64> = all.iter().filter(|p| p.pinned == me.pinned).map(|p| p.id).collect();
        let i = group.iter().position(|p| *p == id).unwrap() as i32;
        let j = (i + delta).clamp(0, group.len() as i32 - 1);
        self.place_person(id, group[j as usize])
    }

    /// Drag and drop: puts `id` where `target` is, in the target's group (pinned or not).
    pub fn place_person(&mut self, id: i64, target: i64) -> Result<()> {
        let all = self.people()?;
        let (Some(me), Some(t)) = (all.iter().find(|p| p.id == id).cloned(), all.iter().find(|p| p.id == target).cloned()) else { return Ok(()) };
        let mut group: Vec<i64> = all.iter().filter(|p| p.pinned == t.pinned && p.id != id).map(|p| p.id).collect();
        let at = group.iter().position(|p| *p == target).unwrap_or(group.len());
        let old = all.iter().filter(|p| p.pinned == t.pinned).position(|p| p.id == id);
        // Dropping onto someone further down puts you after them; further up, before them.
        let at = match old { Some(o) if o <= at => at + 1, _ => at };
        group.insert(at.min(group.len()), id);
        let tx = self.db.transaction()?;
        for (n, pid) in group.iter().enumerate() {
            tx.execute("UPDATE people SET sortOrder = ?, pinned = ? WHERE id = ? AND (sortOrder != ? OR pinned != ?)",
                params![n as i32, t.pinned, pid, n as i32, t.pinned])?;
        }
        tx.commit()?;
        let _ = me;
        Ok(())
    }

    // --- Notes -------------------------------------------------------------------------------

    pub fn add_note(&self, person_id: i64, collection_id: i64, text: &str, due_at: Option<i64>) -> Result<i64> {
        let now = now_millis();
        self.db.execute(
            "INSERT INTO notes (personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, createdAt, sortOrder)
             VALUES (?, ?, ?, ?, ?, 1, ?, 0, 0, ?, ?)",
            params![person_id, collection_id, text.trim(), now, now, due_at, now, -now],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn update_note(&self, n: &Note) -> Result<()> {
        self.db.execute(
            "UPDATE notes SET collectionId=?, text=?, mentionedAt=?, dueAt=?, status=?, outcome=?, doneAt=? WHERE id=?",
            params![n.collection_id, n.text.trim(), n.mentioned_at, n.due_at, n.status, n.outcome, n.done_at, n.id],
        )?;
        Ok(())
    }

    /// Puts a deleted note back exactly as it was (for Undo). It counts as a new change, so the
    /// deletion doesn't win when your Memi syncs.
    pub fn restore_note(&self, n: &Note) -> Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO notes (id, uid, personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, doneAt, createdAt, sortOrder, updatedAt)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
            params![n.id, n.uid, n.person_id, n.collection_id, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at, n.sort_order],
        )?;
        Ok(())
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        self.db.execute("DELETE FROM notes WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn mark_done(&self, n: &Note, outcome: i32) -> Result<()> {
        self.update_note(&Note { status: status::DONE, outcome, done_at: Some(now_millis()), ..n.clone() })
    }

    pub fn reopen(&self, n: &Note) -> Result<()> {
        self.update_note(&Note { status: status::OPEN, outcome: outcome::NONE, done_at: None, ..n.clone() })
    }

    /// Drag and drop within a category: puts note `id` where `target` is.
    pub fn place_note(&mut self, id: i64, target: i64) -> Result<()> {
        let all = self.notes()?;
        let (Some(me), Some(t)) = (all.iter().find(|n| n.id == id), all.iter().find(|n| n.id == target)) else { return Ok(()) };
        if me.collection_id != t.collection_id { return Ok(()); }
        let list: Vec<i64> = all.iter().filter(|n| n.collection_id == t.collection_id && n.status == t.status).map(|n| n.id).collect();
        let reordered = place(&list, id, target);
        let tx = self.db.transaction()?;
        for (n, nid) in reordered.iter().enumerate() {
            tx.execute("UPDATE notes SET sortOrder = ? WHERE id = ? AND sortOrder != ?", params![n as i64, nid, n as i64])?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- Categories (each belongs to one person) ---------------------------------------------

    pub fn add_collection(&self, person_id: i64, name: &str, occ: &str, occasion_day: Option<i64>, yearly: bool) -> Result<i64> {
        let mine = self.collections_for(person_id)?;
        let used: Vec<i32> = mine.iter().map(|c| c.slot).collect();
        let slot = (0..SLOT_COUNT).find(|s| !used.contains(s)).unwrap_or(mine.len() as i32 % SLOT_COUNT);
        let order = mine.iter().map(|c| c.sort_order).max().unwrap_or(0) + 1;
        self.db.execute(
            "INSERT INTO collections (personId, name, kind, slot, sortOrder, occasion, occasionDay, yearly) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![person_id, name.trim(), kind::CUSTOM, slot, order, occ, occasion_day, yearly],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn update_collection(&self, c: &Collection) -> Result<()> {
        self.db.execute(
            "UPDATE collections SET name=?, occasion=?, occasionDay=?, yearly=?, sortOrder=? WHERE id=?",
            params![c.name.trim(), c.occasion, c.occasion_day, c.yearly, c.sort_order, c.id],
        )?;
        Ok(())
    }

    pub fn move_collection(&mut self, c: &Collection, delta: i32) -> Result<()> {
        let Some(pid) = c.person_id else { return Ok(()) };
        let mine: Vec<i64> = self.collections_for(pid)?.iter().map(|x| x.id).collect();
        let Some(i) = mine.iter().position(|x| *x == c.id) else { return Ok(()) };
        let j = (i as i32 + delta).clamp(0, mine.len() as i32 - 1) as usize;
        self.place_collection(c.id, mine[j])
    }

    /// Drag and drop: puts category `id` where `target` is, among that person's categories.
    pub fn place_collection(&mut self, id: i64, target: i64) -> Result<()> {
        let all = self.collections()?;
        let Some(t) = all.iter().find(|c| c.id == target) else { return Ok(()) };
        let list: Vec<i64> = all.iter().filter(|c| c.person_id == t.person_id).map(|c| c.id).collect();
        if !list.contains(&id) { return Ok(()); }
        let reordered = place(&list, id, target);
        let tx = self.db.transaction()?;
        for (n, cid) in reordered.iter().enumerate() {
            tx.execute("UPDATE collections SET sortOrder = ? WHERE id = ? AND sortOrder != ?", params![n as i32, cid, n as i32])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Removes a category. Its notes move to another of the person's categories so nothing is lost.
    pub fn delete_collection(&mut self, c: &Collection) -> Result<()> {
        let others: Vec<Collection> = match c.person_id {
            Some(pid) => self.collections_for(pid)?.into_iter().filter(|x| x.id != c.id).collect(),
            None => vec![],
        };
        let target = others.iter().find(|x| x.kind == kind::NOTE).or(others.first()).map(|x| x.id);
        let tx = self.db.transaction()?;
        match target {
            Some(t) => { tx.execute("UPDATE notes SET collectionId = ? WHERE collectionId = ?", params![t, c.id])?; }
            None => { tx.execute("DELETE FROM notes WHERE collectionId = ?", [c.id])?; }
        }
        tx.execute("DELETE FROM collections WHERE id = ?", [c.id])?;
        tx.commit()?;
        Ok(())
    }

    // --- Sync --------------------------------------------------------------------------------

    /// Makes this computer's copy match `s` (the result of a merge), keeping row ids where it can.
    /// `photo` gives the new photo path for people whose version came from elsewhere. Returns
    /// photo files that are no longer used, for the caller to delete.
    pub fn apply(&mut self, s: &Snapshot, from_remote: &HashSet<String>, mut photo: impl FnMut(&Person) -> Option<String>) -> Result<Vec<String>> {
        let before = self.snapshot()?;
        let old_people: HashMap<&str, &Person> = before.people.iter().map(|p| (p.uid.as_str(), p)).collect();
        let old_cols: HashMap<&str, &Collection> = before.collections.iter().map(|c| (c.uid.as_str(), c)).collect();
        let old_notes: HashMap<&str, &Note> = before.notes.iter().map(|n| (n.uid.as_str(), n)).collect();
        let mut unused_photos = vec![];
        let tx = self.db.transaction()?;

        // Gone everywhere: delete (their tombstones are replaced below).
        let keep: HashSet<&str> = s.people.iter().map(|p| p.uid.as_str())
            .chain(s.collections.iter().map(|c| c.uid.as_str()))
            .chain(s.notes.iter().map(|n| n.uid.as_str())).collect();
        for n in before.notes.iter().filter(|n| !keep.contains(n.uid.as_str())) { tx.execute("DELETE FROM notes WHERE id = ?", [n.id])?; }
        for c in before.collections.iter().filter(|c| !keep.contains(c.uid.as_str())) { tx.execute("DELETE FROM collections WHERE id = ?", [c.id])?; }
        for p in before.people.iter().filter(|p| !keep.contains(p.uid.as_str())) {
            tx.execute("DELETE FROM people WHERE id = ?", [p.id])?;
            unused_photos.extend(p.photo_path.clone());
        }

        let mut person_id: HashMap<String, i64> = before.people.iter().map(|p| (p.uid.clone(), p.id)).collect();
        for p in &s.people {
            let old = old_people.get(p.uid.as_str());
            if old.map_or(false, |o| o.updated_at == p.updated_at) { continue; }
            let path = if from_remote.contains(&p.uid) { photo(p) } else { old.and_then(|o| o.photo_path.clone()) };
            if let Some(o) = old {
                if o.photo_path != path { unused_photos.extend(o.photo_path.clone()); }
                tx.execute(
                    "UPDATE people SET name=?, relationship=?, photoPath=?, birthdayMonth=?, birthdayDay=?, birthYear=?, toneIndex=?, pinned=?, sortOrder=?, createdAt=?, updatedAt=? WHERE id=?",
                    params![p.name, p.relationship, path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.created_at, p.updated_at, o.id],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO people (uid, name, relationship, photoPath, birthdayMonth, birthdayDay, birthYear, toneIndex, pinned, sortOrder, createdAt, updatedAt)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![p.uid, p.name, p.relationship, path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.created_at, p.updated_at],
                )?;
                person_id.insert(p.uid.clone(), tx.last_insert_rowid());
            }
        }

        let mut col_id: HashMap<String, i64> = before.collections.iter().map(|c| (c.uid.clone(), c.id)).collect();
        for c in &s.collections {
            let Some(pid) = c.person_uid.as_ref().and_then(|u| person_id.get(u)).copied() else { continue };
            let old = old_cols.get(c.uid.as_str());
            if old.map_or(false, |o| o.updated_at == c.updated_at && o.person_id == Some(pid)) { continue; }
            if let Some(o) = old {
                tx.execute(
                    "UPDATE collections SET personId=?, name=?, kind=?, slot=?, sortOrder=?, occasion=?, occasionDay=?, yearly=?, updatedAt=? WHERE id=?",
                    params![pid, c.name, c.kind, c.slot, c.sort_order, c.occasion, c.occasion_day, c.yearly, c.updated_at, o.id],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO collections (uid, personId, name, kind, slot, sortOrder, occasion, occasionDay, yearly, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![c.uid, pid, c.name, c.kind, c.slot, c.sort_order, c.occasion, c.occasion_day, c.yearly, c.updated_at],
                )?;
                col_id.insert(c.uid.clone(), tx.last_insert_rowid());
            }
        }

        for n in &s.notes {
            let (Some(pid), Some(cid)) = (person_id.get(&n.person_uid).copied(), col_id.get(&n.collection_uid).copied()) else { continue };
            let old = old_notes.get(n.uid.as_str());
            if old.map_or(false, |o| o.updated_at == n.updated_at && o.collection_id == cid) { continue; }
            if let Some(o) = old {
                tx.execute(
                    "UPDATE notes SET personId=?, collectionId=?, text=?, mentionedAt=?, firstMentionedAt=?, mentionCount=?, dueAt=?, status=?, outcome=?, doneAt=?, createdAt=?, sortOrder=?, updatedAt=? WHERE id=?",
                    params![pid, cid, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at, n.sort_order, n.updated_at, o.id],
                )?;
                // A note that only moved category keeps its time; the trigger mustn't stamp it.
                if o.updated_at != n.updated_at || o.collection_id != cid {
                    tx.execute("UPDATE notes SET updatedAt = ? WHERE id = ? AND updatedAt != ?", params![n.updated_at, o.id, n.updated_at])?;
                }
            } else {
                tx.execute(
                    "INSERT INTO notes (uid, personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, doneAt, createdAt, sortOrder, updatedAt)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![n.uid, pid, cid, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at, n.sort_order, n.updated_at],
                )?;
            }
        }

        tx.execute("DELETE FROM tombstones", [])?;
        for t in &s.tombstones {
            tx.execute("INSERT OR REPLACE INTO tombstones (uid, deletedAt) VALUES (?, ?)", params![t.uid, t.deleted_at])?;
        }
        tx.commit()?;
        Ok(unused_photos)
    }
}

/// `list` with `id` moved to where `target` is: after it if moving down, before it if moving up.
fn place(list: &[i64], id: i64, target: i64) -> Vec<i64> {
    let (Some(from), Some(to)) = (list.iter().position(|x| *x == id), list.iter().position(|x| *x == target)) else { return list.to_vec() };
    let mut v = list.to_vec();
    let x = v.remove(from);
    v.insert(to, x);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_people_get_three_categories_and_deleting_cleans_up() {
        let mut s = Store::in_memory();
        let id = s.add_person(&Person::new("Maya Okafor")).unwrap();
        let cats = s.collections().unwrap();
        assert_eq!(cats.len(), 3);
        assert_eq!(cats[0].occasion, occasion::BIRTHDAY);
        s.add_note(id, cats[0].id, "Mug", None).unwrap();
        s.delete_collection(&cats[0]).unwrap();
        let notes = s.notes().unwrap();
        assert_eq!(notes[0].collection_id, cats[2].id, "notes move to Notes");
        s.delete_person(id).unwrap();
        assert!(s.notes().unwrap().is_empty() && s.collections().unwrap().is_empty());
        assert_eq!(s.tombstones().unwrap().len(), 5, "a person, three categories and a note");
    }

    #[test]
    fn reordering_people_stays_within_pinned_group() {
        let mut s = Store::in_memory();
        let a = s.add_person(&Person::new("A")).unwrap();
        let b = s.add_person(&Person::new("B")).unwrap();
        let c = s.add_person(&Person::new("C")).unwrap();
        s.move_person(b, -1).unwrap();
        let ids = |s: &Store| s.people().unwrap().into_iter().map(|p| p.id).collect::<Vec<_>>();
        assert_eq!(ids(&s), vec![b, a, c]);
        s.place_person(b, c).unwrap();
        assert_eq!(ids(&s), vec![a, c, b], "dragged down, it lands after the target");
        s.place_person(b, a).unwrap();
        assert_eq!(ids(&s), vec![b, a, c], "dragged up, it lands before the target");
    }

    #[test]
    fn rows_get_uids_and_edits_move_the_time_on() {
        let mut s = Store::in_memory();
        let id = s.add_person(&Person::new("Maya")).unwrap();
        let p = s.person(id).unwrap().unwrap();
        assert_eq!(p.uid.len(), 32);
        assert!(p.updated_at > 0);
        s.db.execute("UPDATE people SET updatedAt = 1 WHERE id = ?", [id]).unwrap();
        s.update_person(&Person { name: "Maya O".into(), ..p.clone() }).unwrap();
        assert!(s.person(id).unwrap().unwrap().updated_at > 1, "an edit is stamped");
    }

    #[test]
    fn notes_keep_newest_first_until_dragged() {
        let mut s = Store::in_memory();
        let id = s.add_person(&Person::new("Maya")).unwrap();
        let c = s.collections().unwrap()[0].id;
        let a = s.add_note(id, c, "First", None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        let b = s.add_note(id, c, "Second", None).unwrap();
        let texts = |s: &Store| s.notes().unwrap().into_iter().map(|n| n.id).collect::<Vec<_>>();
        assert_eq!(texts(&s), vec![b, a]);
        s.place_note(a, b).unwrap();
        assert_eq!(texts(&s), vec![a, b]);
    }

    #[test]
    fn applying_a_merge_adds_updates_and_removes_by_uid() {
        let mut here = Store::in_memory();
        let id = here.add_person(&Person::new("Maya")).unwrap();
        let mut other = Store::in_memory();
        let local = here.snapshot().unwrap();
        // The other device has Maya renamed (newer) and a new person.
        let mut remote = local.clone();
        remote.people[0].name = "Maya Okafor".into();
        remote.people[0].updated_at += 1000;
        let sam = Person { uid: "sam".into(), updated_at: 5, ..Person::new("Sam") };
        remote.people.push(sam);
        let m = memi_core::merge::merge(&local, &remote, now_millis());
        here.apply(&m.snapshot, &m.people_from_remote, |_| None).unwrap();
        let people = here.people().unwrap();
        assert_eq!(people.len(), 2);
        assert_eq!(here.person(id).unwrap().unwrap().name, "Maya Okafor", "same row, updated");
        // Now delete Maya elsewhere.
        let mut later = here.snapshot().unwrap();
        let maya = later.people.iter().find(|p| p.id == id).unwrap().uid.clone();
        later.tombstones.push(Tombstone { uid: maya, deleted_at: now_millis() + 10_000 });
        let m = memi_core::merge::merge(&here.snapshot().unwrap(), &later, now_millis());
        here.apply(&m.snapshot, &m.people_from_remote, |_| None).unwrap();
        assert_eq!(here.people().unwrap().len(), 1);
        assert!(here.collections().unwrap().is_empty() && here.notes().unwrap().is_empty());
        let _ = &mut other;
    }

    #[test]
    fn upgrading_gives_the_same_uids_on_every_device() {
        let dir = std::env::temp_dir().join(format!("memi-upgrade-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.db");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(SCHEMA).unwrap();
            db.execute("INSERT INTO people (id, name, createdAt) VALUES (4, 'Sam', 1234)", []).unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.people().unwrap()[0].uid, legacy_uid::person(4, 1234));
        std::fs::remove_dir_all(dir).ok();
    }
}
