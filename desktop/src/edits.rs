//! The edits the desktop's screens make, by row id, carried out by memi-engine (which works by uid,
//! stamps edit times and keeps deletions for sync). Every call comes through MemiView::change, so
//! each one is saved and queued for sync.

use anyhow::{anyhow, Result};
use memi_engine::core::model::*;
use memi_engine::Data;

pub struct Edits<'a> {
    pub d: &'a mut Data,
}

fn missing() -> anyhow::Error { anyhow!("That's no longer here.") }

impl Edits<'_> {
    fn person_uid(&self, id: i64) -> Result<String> { self.d.person_uid(id).ok_or_else(missing) }
    fn collection_uid(&self, id: i64) -> Result<String> { self.d.collection_uid(id).ok_or_else(missing) }
    fn note_uid(&self, id: i64) -> Result<String> { self.d.note_uid(id).ok_or_else(missing) }

    // --- People --------------------------------------------------------------------------------------

    /// Adds someone (photo already saved, in `photo_path`) with the starter categories; returns their id.
    pub fn add_person(&mut self, p: &Person) -> Result<i64> {
        let uid = self.d.add_person(p.clone());
        self.d.person(&uid).map(|p| p.id).ok_or_else(missing)
    }

    pub fn update_person(&mut self, p: &Person) -> Result<()> {
        let uid = self.person_uid(p.id)?;
        self.d.update_person(Person { uid, ..p.clone() });
        Ok(())
    }

    pub fn delete_person(&mut self, id: i64) -> Result<()> {
        let uid = self.person_uid(id)?;
        self.d.remove_person(&uid);
        Ok(())
    }

    pub fn set_pinned(&mut self, id: i64, pinned: bool) -> Result<()> {
        let uid = self.person_uid(id)?;
        self.d.set_pinned(&uid, pinned);
        Ok(())
    }

    pub fn place_person(&mut self, id: i64, target: i64) -> Result<()> {
        let (a, b) = (self.person_uid(id)?, self.person_uid(target)?);
        self.d.place_person(&a, &b);
        Ok(())
    }

    /// Moves someone one place up (-1) or down (1) within the pinned or unpinned group.
    pub fn move_person(&mut self, id: i64, delta: i32) -> Result<()> {
        let me = self.d.person_by_id(id).cloned().ok_or_else(missing)?;
        let group: Vec<&Person> = self.d.snap.people.iter().filter(|p| p.pinned == me.pinned).collect();
        let Some(at) = group.iter().position(|p| p.id == id) else { return Ok(()) };
        let Some(next) = at.checked_add_signed(delta as isize).and_then(|i| group.get(i)) else { return Ok(()) };
        let target = next.uid.clone();
        self.d.place_person(&me.uid, &target);
        Ok(())
    }

    // --- Categories ----------------------------------------------------------------------------------

    pub fn add_collection(&mut self, person_id: i64, name: &str, occasion: &str, occasion_day: Option<i64>, yearly: bool) -> Result<i64> {
        let p = self.person_uid(person_id)?;
        let uid = self.d.add_collection(&p, name, occasion, occasion_day, yearly);
        self.d.collection(&uid).map(|c| c.id).ok_or_else(missing)
    }

    pub fn update_collection(&mut self, c: &Collection) -> Result<()> {
        let uid = self.collection_uid(c.id)?;
        let person_uid = c.person_id.and_then(|id| self.d.person_uid(id));
        self.d.update_collection(Collection { uid, person_uid, ..c.clone() });
        Ok(())
    }

    pub fn delete_collection(&mut self, c: &Collection) -> Result<()> {
        let uid = self.collection_uid(c.id)?;
        self.d.remove_collection(&uid);
        Ok(())
    }

    pub fn place_collection(&mut self, id: i64, target: i64) -> Result<()> {
        let (a, b) = (self.collection_uid(id)?, self.collection_uid(target)?);
        self.d.place_collection(&a, &b);
        Ok(())
    }

    /// Moves a category one place left (-1) or right (1) among the person's categories.
    pub fn move_collection(&mut self, c: &Collection, delta: i32) -> Result<()> {
        let uid = self.collection_uid(c.id)?;
        let person = self.d.collection(&uid).and_then(|c| c.person_uid.clone()).ok_or_else(missing)?;
        let mine = self.d.categories_of(&person);
        let Some(at) = mine.iter().position(|x| x.uid == uid) else { return Ok(()) };
        let Some(next) = at.checked_add_signed(delta as isize).and_then(|i| mine.get(i)) else { return Ok(()) };
        self.d.place_collection(&uid, &next.uid);
        Ok(())
    }

    // --- Notes ---------------------------------------------------------------------------------------

    /// Adds a note; returns its id.
    pub fn add_note(&mut self, person_id: i64, collection_id: i64, text: &str, due_at: Option<i64>) -> Result<i64> {
        let (p, c) = (self.person_uid(person_id)?, self.collection_uid(collection_id)?);
        let uid = self.d.add_note(&p, &c, text, due_at);
        self.d.note(&uid).map(|n| n.id).ok_or_else(missing)
    }

    /// Saves a note as given, including a move to another category (by its id).
    pub fn update_note(&mut self, n: &Note) -> Result<()> {
        let uid = self.note_uid(n.id)?;
        let collection_uid = self.collection_uid(n.collection_id)?;
        let person_uid = self.person_uid(n.person_id)?;
        self.d.update_note(Note { uid, collection_uid, person_uid, ..n.clone() });
        Ok(())
    }

    pub fn delete_note(&mut self, id: i64) -> Result<()> {
        let uid = self.note_uid(id)?;
        self.d.remove_note(&uid);
        Ok(())
    }

    /// Undo: puts a note back as it was (it keeps its uid, so it's the same note everywhere).
    pub fn restore_note(&mut self, n: &Note) -> Result<()> {
        self.d.restore_note(n.clone());
        Ok(())
    }

    pub fn mark_done(&mut self, n: &Note, result: i32) -> Result<()> {
        let uid = self.note_uid(n.id)?;
        self.d.mark_done(&uid, result);
        Ok(())
    }

    pub fn reopen(&mut self, n: &Note) -> Result<()> {
        let uid = self.note_uid(n.id)?;
        self.d.reopen(&uid);
        Ok(())
    }

    pub fn place_note(&mut self, id: i64, target: i64) -> Result<()> {
        let (a, b) = (self.note_uid(id)?, self.note_uid(target)?);
        self.d.place_note(&a, &b);
        Ok(())
    }
}
