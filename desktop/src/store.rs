//! Everything Your Memi keeps on this computer, in one SQLite file in its own data folder.
//! Mirrors the Android Repository: same rules for ordering, starter categories and deletion.

use crate::model::*;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
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

fn person(r: &Row) -> rusqlite::Result<Person> {
    Ok(Person {
        id: r.get("id")?,
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
    })
}

fn collection(r: &Row) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: r.get("id")?,
        person_id: r.get("personId")?,
        name: r.get("name")?,
        kind: r.get("kind")?,
        slot: r.get("slot")?,
        sort_order: r.get("sortOrder")?,
        occasion: r.get("occasion")?,
        occasion_day: r.get("occasionDay")?,
        yearly: r.get("yearly")?,
    })
}

fn note(r: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get("id")?,
        person_id: r.get("personId")?,
        collection_id: r.get("collectionId")?,
        text: r.get("text")?,
        mentioned_at: r.get("mentionedAt")?,
        first_mentioned_at: r.get("firstMentionedAt")?,
        mention_count: r.get("mentionCount")?,
        due_at: r.get("dueAt")?,
        status: r.get("status")?,
        outcome: r.get("outcome")?,
        done_at: r.get("doneAt")?,
        created_at: r.get("createdAt")?,
    })
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        db.pragma_update(None, "foreign_keys", true)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.execute_batch(SCHEMA)?;
        Ok(Store { db })
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        let db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "foreign_keys", true).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        Store { db }
    }

    // --- Reading -----------------------------------------------------------------------------

    pub fn people(&self) -> Result<Vec<Person>> {
        let mut st = self.db.prepare("SELECT * FROM people ORDER BY pinned DESC, sortOrder, name COLLATE NOCASE")?;
        let rows = st.query_map([], person)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut st = self.db.prepare("SELECT * FROM collections ORDER BY personId, sortOrder")?;
        let rows = st.query_map([], collection)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn notes(&self) -> Result<Vec<Note>> {
        let mut st = self.db.prepare("SELECT * FROM notes ORDER BY mentionedAt DESC")?;
        let rows = st.query_map([], note)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn collections_for(&self, person_id: i64) -> Result<Vec<Collection>> {
        let mut st = self.db.prepare("SELECT * FROM collections WHERE personId = ? ORDER BY sortOrder")?;
        let rows = st.query_map([person_id], collection)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn person(&self, id: i64) -> Result<Option<Person>> {
        Ok(self.db.query_row("SELECT * FROM people WHERE id = ?", [id], person).optional()?)
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
        let mut group: Vec<Person> = all.into_iter().filter(|p| p.pinned == me.pinned).collect();
        let i = group.iter().position(|p| p.id == id).unwrap() as i32;
        let j = i + delta;
        if j < 0 || j >= group.len() as i32 { return Ok(()); }
        let p = group.remove(i as usize);
        group.insert(j as usize, p);
        let tx = self.db.transaction()?;
        for (n, p) in group.iter().enumerate() {
            tx.execute("UPDATE people SET sortOrder = ? WHERE id = ?", params![n as i32, p.id])?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- Notes -------------------------------------------------------------------------------

    pub fn add_note(&self, person_id: i64, collection_id: i64, text: &str, due_at: Option<i64>) -> Result<i64> {
        let now = now_millis();
        self.db.execute(
            "INSERT INTO notes (personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, createdAt)
             VALUES (?, ?, ?, ?, ?, 1, ?, 0, 0, ?)",
            params![person_id, collection_id, text.trim(), now, now, due_at, now],
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

    /// Puts a deleted note back exactly as it was (for Undo).
    pub fn restore_note(&self, n: &Note) -> Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO notes (id, personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, doneAt, createdAt)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![n.id, n.person_id, n.collection_id, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at],
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
        let mut mine = self.collections_for(pid)?;
        let Some(i) = mine.iter().position(|x| x.id == c.id) else { return Ok(()) };
        let j = i as i32 + delta;
        if j < 0 || j >= mine.len() as i32 { return Ok(()); }
        let x = mine.remove(i);
        mine.insert(j as usize, x);
        let tx = self.db.transaction()?;
        for (n, c) in mine.iter().enumerate() {
            tx.execute("UPDATE collections SET sortOrder = ? WHERE id = ?", params![n as i32, c.id])?;
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

    // --- Whole-database replace, for restoring a sync backup ---------------------------------

    pub fn replace_all(&mut self, people: &[Person], collections: &[Collection], notes: &[Note]) -> Result<()> {
        let tx = self.db.transaction()?;
        tx.execute_batch("DELETE FROM notes; DELETE FROM collections; DELETE FROM people;")?;
        for p in people {
            tx.execute(
                "INSERT INTO people (id, name, relationship, photoPath, birthdayMonth, birthdayDay, birthYear, toneIndex, pinned, sortOrder, createdAt)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![p.id, p.name, p.relationship, p.photo_path, p.birthday_month, p.birthday_day, p.birth_year, p.tone_index, p.pinned, p.sort_order, p.created_at],
            )?;
        }
        for c in collections {
            tx.execute(
                "INSERT INTO collections (id, personId, name, kind, slot, sortOrder, occasion, occasionDay, yearly) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![c.id, c.person_id, c.name, c.kind, c.slot, c.sort_order, c.occasion, c.occasion_day, c.yearly],
            )?;
        }
        for n in notes {
            tx.execute(
                "INSERT INTO notes (id, personId, collectionId, text, mentionedAt, firstMentionedAt, mentionCount, dueAt, status, outcome, doneAt, createdAt)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![n.id, n.person_id, n.collection_id, n.text, n.mentioned_at, n.first_mentioned_at, n.mention_count, n.due_at, n.status, n.outcome, n.done_at, n.created_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
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
    }

    #[test]
    fn reordering_people_stays_within_pinned_group() {
        let mut s = Store::in_memory();
        let a = s.add_person(&Person::new("A")).unwrap();
        let b = s.add_person(&Person::new("B")).unwrap();
        s.move_person(b, -1).unwrap();
        let names: Vec<_> = s.people().unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(names, vec![b, a]);
    }
}
