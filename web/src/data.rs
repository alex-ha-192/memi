//! Everything in this browser's Memi, in memory: people, categories, notes, deletions and photos.
//! It's saved to IndexedDB as one backup document (the same format the apps sync), after every
//! change.
//!
//! Records carry the same uids and edit times as on the other devices, so the copy in Google Drive
//! merges item by item (memi_core::merge). Row ids (`id`, `person_id`, `collection_id`) only exist
//! because the shared insights use them; they're renumbered whenever the data is loaded or merged,
//! so the UI always refers to people and categories by uid.

use memi_core::model::*;
use std::collections::HashMap;

#[derive(Clone, Default)]
pub struct Data {
    pub snap: Snapshot,
    /// Photo bytes by person uid.
    pub photos: HashMap<String, Vec<u8>>,
}

pub fn now() -> i64 { now_millis() }

impl Data {
    pub fn from_doc(doc: memi_core::backup::Doc) -> Self {
        let mut d = Data { snap: doc.snapshot, photos: doc.photos };
        d.renumber();
        d
    }

    pub fn export(&self) -> Vec<u8> {
        memi_core::backup::export(&self.snap, |p| self.photos.get(&p.uid).cloned())
    }

    /// Gives every record a fresh row id and points notes and categories at their parents' ids.
    pub fn renumber(&mut self) {
        let s = &mut self.snap;
        s.people.sort_by(|a, b| (!a.pinned, a.sort_order, a.name.to_lowercase()).cmp(&(!b.pinned, b.sort_order, b.name.to_lowercase())));
        let mut pid: HashMap<String, i64> = HashMap::new();
        for (i, p) in s.people.iter_mut().enumerate() {
            p.id = i as i64 + 1;
            pid.insert(p.uid.clone(), p.id);
        }
        s.collections.retain(|c| c.person_uid.as_ref().is_some_and(|u| pid.contains_key(u)));
        s.collections.sort_by(|a, b| (&a.person_uid, a.sort_order, &a.uid).cmp(&(&b.person_uid, b.sort_order, &b.uid)));
        let mut cid: HashMap<String, i64> = HashMap::new();
        for (i, c) in s.collections.iter_mut().enumerate() {
            c.id = i as i64 + 1;
            c.person_id = c.person_uid.as_ref().and_then(|u| pid.get(u)).copied();
            cid.insert(c.uid.clone(), c.id);
        }
        s.notes.retain(|n| pid.contains_key(&n.person_uid) && cid.contains_key(&n.collection_uid));
        s.notes.sort_by(|a, b| (a.sort_order, -a.mentioned_at, &a.uid).cmp(&(b.sort_order, -b.mentioned_at, &b.uid)));
        for (i, n) in s.notes.iter_mut().enumerate() {
            n.id = i as i64 + 1;
            n.person_id = pid[&n.person_uid];
            n.collection_id = cid[&n.collection_uid];
        }
        let keep: std::collections::HashSet<&String> = s.people.iter().map(|p| &p.uid).collect();
        self.photos.retain(|uid, _| keep.contains(uid));
    }

    // --- Looking things up ----------------------------------------------------------------------

    pub fn person(&self, uid: &str) -> Option<&Person> { self.snap.people.iter().find(|p| p.uid == uid) }
    pub fn collection(&self, uid: &str) -> Option<&Collection> { self.snap.collections.iter().find(|c| c.uid == uid) }
    pub fn note(&self, uid: &str) -> Option<&Note> { self.snap.notes.iter().find(|n| n.uid == uid) }
    pub fn person_by_id(&self, id: i64) -> Option<&Person> { self.snap.people.iter().find(|p| p.id == id) }
    pub fn collection_by_id(&self, id: i64) -> Option<&Collection> { self.snap.collections.iter().find(|c| c.id == id) }

    pub fn categories_of(&self, person_uid: &str) -> Vec<Collection> {
        self.snap.collections.iter().filter(|c| c.person_uid.as_deref() == Some(person_uid)).cloned().collect()
    }

    /// Open notes in a category, in your order.
    pub fn open_in(&self, collection_uid: &str) -> Vec<Note> {
        self.snap.notes.iter().filter(|n| n.collection_uid == collection_uid && n.status == status::OPEN).cloned().collect()
    }

    fn deleted(&mut self, uid: &str) {
        let at = now();
        self.snap.tombstones.retain(|t| t.uid != uid);
        self.snap.tombstones.push(Tombstone { uid: uid.to_string(), deleted_at: at });
    }

    fn revived(&mut self, uid: &str) { self.snap.tombstones.retain(|t| t.uid != uid); }

    // --- People ---------------------------------------------------------------------------------

    /// Adds someone with the three starter categories; Gift ideas builds up to their birthday.
    pub fn add_person(&mut self, mut p: Person, photo: Option<Vec<u8>>) -> String {
        let t = now();
        p.uid = new_uid();
        p.updated_at = t;
        p.created_at = t;
        p.tone_index = (self.snap.people.len() % 8) as i32;
        p.sort_order = self.snap.people.iter().map(|x| x.sort_order).max().unwrap_or(0) + 1;
        for (i, (name, k, slot, occ)) in [("Gift ideas", kind::GIFT, 0, occasion::BIRTHDAY), ("Plans", kind::PLAN, 1, occasion::NONE), ("Notes", kind::NOTE, 2, occasion::NONE)].into_iter().enumerate() {
            self.snap.collections.push(Collection {
                id: 0, uid: new_uid(), person_id: None, person_uid: Some(p.uid.clone()), name: name.into(), kind: k.into(),
                slot, sort_order: i as i32, occasion: occ.into(), occasion_day: None, yearly: false, updated_at: t,
            });
        }
        if let Some(b) = photo { self.photos.insert(p.uid.clone(), b); }
        let uid = p.uid.clone();
        self.snap.people.push(p);
        self.renumber();
        uid
    }

    /// Saves edits to someone. `photo`: None keeps it, Some(None) removes it, Some(Some) replaces it.
    pub fn update_person(&mut self, p: Person, photo: Option<Option<Vec<u8>>>) {
        let uid = p.uid.clone();
        if let Some(x) = self.snap.people.iter_mut().find(|x| x.uid == uid) {
            *x = Person { updated_at: now(), ..p };
        }
        match photo {
            Some(Some(b)) => { self.photos.insert(uid, b); }
            Some(None) => { self.photos.remove(&uid); }
            None => {}
        }
        self.renumber();
    }

    pub fn remove_person(&mut self, uid: &str) {
        let cols: Vec<String> = self.snap.collections.iter().filter(|c| c.person_uid.as_deref() == Some(uid)).map(|c| c.uid.clone()).collect();
        let notes: Vec<String> = self.snap.notes.iter().filter(|n| n.person_uid == uid).map(|n| n.uid.clone()).collect();
        for u in cols.iter().chain(&notes).chain(std::iter::once(&uid.to_string())) { self.deleted(u); }
        self.snap.people.retain(|p| p.uid != uid);
        self.photos.remove(uid);
        self.renumber();
    }

    pub fn set_pinned(&mut self, uid: &str, pinned: bool) {
        let end = self.snap.people.iter().filter(|p| p.pinned == pinned).map(|p| p.sort_order).max().unwrap_or(0) + 1;
        if let Some(p) = self.snap.people.iter_mut().find(|p| p.uid == uid) {
            p.pinned = pinned;
            p.sort_order = end;
            p.updated_at = now();
        }
        self.renumber();
    }

    /// Drag and drop: puts someone in `target`'s place (joining the pinned or unpinned group they're
    /// dropped into). Same rules as the desktop and Android apps.
    pub fn place_person(&mut self, uid: &str, target: &str) {
        let Some(t) = self.person(target).cloned() else { return };
        let Some(me) = self.person(uid).cloned() else { return };
        let in_group: Vec<&Person> = self.snap.people.iter().filter(|p| p.pinned == t.pinned).collect();
        let mut group: Vec<String> = in_group.iter().filter(|p| p.uid != uid).map(|p| p.uid.clone()).collect();
        let mut at = group.iter().position(|u| *u == target).unwrap_or(group.len());
        if let Some(old) = in_group.iter().position(|p| p.uid == uid) { if old <= at { at += 1; } }
        group.insert(at.min(group.len()), me.uid.clone());
        let t_now = now();
        for (i, u) in group.iter().enumerate() {
            if let Some(p) = self.snap.people.iter_mut().find(|p| &p.uid == u) {
                if p.sort_order != i as i32 || p.pinned != t.pinned {
                    p.sort_order = i as i32;
                    p.pinned = t.pinned;
                    p.updated_at = t_now;
                }
            }
        }
        self.renumber();
    }

    // --- Notes ----------------------------------------------------------------------------------

    pub fn add_note(&mut self, person_uid: &str, collection_uid: &str, text: &str, due_at: Option<i64>) -> String {
        let t = now();
        let n = Note {
            id: 0, uid: new_uid(), person_id: 0, person_uid: person_uid.into(), collection_id: 0, collection_uid: collection_uid.into(),
            text: text.trim().into(), mentioned_at: t, first_mentioned_at: t, mention_count: 1, due_at, status: status::OPEN,
            outcome: outcome::NONE, done_at: None, created_at: t, updated_at: t, sort_order: -t,
        };
        let uid = n.uid.clone();
        self.snap.notes.push(n);
        self.renumber();
        uid
    }

    pub fn update_note(&mut self, n: Note) {
        if let Some(x) = self.snap.notes.iter_mut().find(|x| x.uid == n.uid) {
            *x = Note { updated_at: now(), ..n };
        }
        self.renumber();
    }

    pub fn remove_note(&mut self, uid: &str) {
        self.deleted(uid);
        self.snap.notes.retain(|n| n.uid != uid);
        self.renumber();
    }

    /// Undo: puts a note back exactly as it was, as a new change so it wins over the deletion.
    pub fn restore_note(&mut self, n: Note) {
        self.revived(&n.uid);
        self.snap.notes.retain(|x| x.uid != n.uid);
        self.snap.notes.push(Note { updated_at: now(), ..n });
        self.renumber();
    }

    pub fn mark_done(&mut self, uid: &str, result: i32) {
        if let Some(n) = self.note(uid).cloned() {
            self.update_note(Note { status: status::DONE, outcome: result, done_at: n.done_at.or(Some(now())), ..n });
        }
    }

    pub fn reopen(&mut self, uid: &str) {
        if let Some(n) = self.note(uid).cloned() {
            self.update_note(Note { status: status::OPEN, outcome: outcome::NONE, done_at: None, ..n });
        }
    }

    pub fn move_note(&mut self, uid: &str, to_collection: &str) {
        if let Some(n) = self.note(uid).cloned() {
            self.update_note(Note { collection_uid: to_collection.into(), ..n });
        }
    }

    /// Drag and drop: puts a note in `target`'s place within their category.
    pub fn place_note(&mut self, uid: &str, target: &str) {
        let (Some(me), Some(t)) = (self.note(uid).cloned(), self.note(target).cloned()) else { return };
        if me.collection_uid != t.collection_uid { return; }
        let list: Vec<String> = self.snap.notes.iter().filter(|n| n.collection_uid == t.collection_uid && n.status == t.status).map(|n| n.uid.clone()).collect();
        let order = place(&list, uid, target);
        let t_now = now();
        for (i, u) in order.iter().enumerate() {
            if let Some(n) = self.snap.notes.iter_mut().find(|n| &n.uid == u) {
                if n.sort_order != i as i64 { n.sort_order = i as i64; n.updated_at = t_now; }
            }
        }
        self.renumber();
    }

    // --- Categories -----------------------------------------------------------------------------

    pub fn add_collection(&mut self, person_uid: &str, name: &str, occ: &str, occasion_day: Option<i64>, yearly: bool) -> String {
        let mine = self.categories_of(person_uid);
        let used: Vec<i32> = mine.iter().map(|c| c.slot).collect();
        let slot = (0..SLOT_COUNT).find(|s| !used.contains(s)).unwrap_or(mine.len() as i32 % SLOT_COUNT);
        let c = Collection {
            id: 0, uid: new_uid(), person_id: None, person_uid: Some(person_uid.into()), name: name.trim().into(), kind: kind::CUSTOM.into(),
            slot, sort_order: mine.iter().map(|c| c.sort_order).max().unwrap_or(0) + 1, occasion: occ.into(), occasion_day, yearly, updated_at: now(),
        };
        let uid = c.uid.clone();
        self.snap.collections.push(c);
        self.renumber();
        uid
    }

    pub fn update_collection(&mut self, c: Collection) {
        if let Some(x) = self.snap.collections.iter_mut().find(|x| x.uid == c.uid) {
            *x = Collection { name: c.name.trim().to_string(), updated_at: now(), ..c };
        }
        self.renumber();
    }

    /// Removes a category. Its notes move to another of the person's categories so nothing is lost.
    pub fn remove_collection(&mut self, uid: &str) {
        let Some(c) = self.collection(uid).cloned() else { return };
        let person = c.person_uid.clone().unwrap_or_default();
        let others: Vec<Collection> = self.categories_of(&person).into_iter().filter(|x| x.uid != uid).collect();
        let target = others.iter().find(|x| x.kind == kind::NOTE).or(others.first()).map(|x| x.uid.clone());
        let t = now();
        let moving: Vec<String> = self.snap.notes.iter().filter(|n| n.collection_uid == uid).map(|n| n.uid.clone()).collect();
        for u in moving {
            match &target {
                Some(to) => if let Some(n) = self.snap.notes.iter_mut().find(|n| n.uid == u) { n.collection_uid = to.clone(); n.updated_at = t; },
                None => self.remove_note(&u),
            }
        }
        self.deleted(uid);
        self.snap.collections.retain(|x| x.uid != uid);
        self.renumber();
    }

    pub fn place_collection(&mut self, uid: &str, target: &str) {
        let Some(t) = self.collection(target).cloned() else { return };
        let list: Vec<String> = self.categories_of(t.person_uid.as_deref().unwrap_or_default()).iter().map(|c| c.uid.clone()).collect();
        if !list.iter().any(|u| u == uid) { return; }
        let order = place(&list, uid, target);
        let t_now = now();
        for (i, u) in order.iter().enumerate() {
            if let Some(c) = self.snap.collections.iter_mut().find(|c| &c.uid == u) {
                if c.sort_order != i as i32 { c.sort_order = i as i32; c.updated_at = t_now; }
            }
        }
        self.renumber();
    }

    /// Replaces everything with a merged copy (photos from Drive for the people that came from there).
    pub fn apply_merged(&mut self, merged: Snapshot, from_remote: &std::collections::HashSet<String>, remote_photos: &HashMap<String, Vec<u8>>) {
        for uid in from_remote {
            match remote_photos.get(uid) {
                Some(b) => { self.photos.insert(uid.clone(), b.clone()); }
                None => { self.photos.remove(uid); }
            }
        }
        self.snap = merged;
        self.renumber();
    }
}

/// `list` with `id` moved to where `target` is.
fn place(list: &[String], id: &str, target: &str) -> Vec<String> {
    let (Some(from), Some(to)) = (list.iter().position(|x| x == id), list.iter().position(|x| x == target)) else { return list.to_vec() };
    let mut v = list.to_vec();
    let x = v.remove(from);
    v.insert(to, x);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_categories_and_notes_link_up() {
        let mut d = Data::default();
        let maya = d.add_person(Person::new("Maya"), None);
        let sam = d.add_person(Person::new("Sam"), None);
        let gifts = d.categories_of(&maya)[0].uid.clone();
        let n = d.add_note(&maya, &gifts, "Blue mug", None);
        assert_eq!(d.note(&n).unwrap().person_id, d.person(&maya).unwrap().id);
        d.place_person(&sam, &maya);
        assert_eq!(d.snap.people[0].uid, sam);
        d.remove_person(&maya);
        assert!(d.note(&n).is_none());
        assert_eq!(d.snap.tombstones.len(), 5, "person, three categories, one note");
        let round = Data::from_doc(memi_core::backup::import(&d.export()).unwrap());
        assert_eq!(round.snap.people.len(), 1);
    }
}
