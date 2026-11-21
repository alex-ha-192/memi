//! A whole Memi in memory (people, categories, notes and recent deletions), and every edit you can
//! make to it. Pure: no storage, network or photos here (see memi.rs for those).
//!
//! Records carry the same uids and edit times on every device, so copies merge item by item
//! (memi_core::merge). Every edit stamps the record's edit time. Row ids (`id`, `person_id`,
//! `collection_id`) stay the same for a record for as long as the app runs, so the screens can use
//! them; new records get new ones. They mean nothing on another device.

use memi_core::model::*;
use std::collections::HashMap;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Data {
    pub snap: Snapshot,
    /// Row ids handed out so far, by uid, so a record keeps its id across edits and merges.
    ids: HashMap<String, i64>,
    next_id: i64,
}

pub fn now() -> i64 { now_millis() }

impl Data {
    pub fn new(snap: Snapshot) -> Self {
        let mut d = Data { snap, ids: HashMap::new(), next_id: 1 };
        // Keep the ids records were loaded with (a database's row ids), if they're usable.
        for p in &d.snap.people { if p.id > 0 { d.ids.insert(p.uid.clone(), p.id); } }
        for c in &d.snap.collections { if c.id > 0 { d.ids.insert(c.uid.clone(), c.id); } }
        for n in &d.snap.notes { if n.id > 0 { d.ids.insert(n.uid.clone(), n.id); } }
        d.next_id = d.ids.values().copied().max().unwrap_or(0) + 1;
        d.renumber();
        d
    }

    fn id_for(&mut self, uid: &str) -> i64 {
        if let Some(id) = self.ids.get(uid) { return *id; }
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.ids.insert(uid.to_string(), id);
        id
    }

    /// Puts everything in display order, gives new records ids, points notes and categories at
    /// their parents' ids, and drops anything whose parent is gone.
    pub fn renumber(&mut self) {
        let mut s = std::mem::take(&mut self.snap);
        s.people.sort_by(|a, b| (!a.pinned, a.sort_order, a.name.to_lowercase(), &a.uid).cmp(&(!b.pinned, b.sort_order, b.name.to_lowercase(), &b.uid)));
        let mut pid: HashMap<String, i64> = HashMap::new();
        for p in s.people.iter_mut() {
            p.id = self.id_for(&p.uid);
            pid.insert(p.uid.clone(), p.id);
        }
        s.collections.retain(|c| c.person_uid.as_ref().is_some_and(|u| pid.contains_key(u)));
        s.collections.sort_by(|a, b| (&a.person_uid, a.sort_order, &a.uid).cmp(&(&b.person_uid, b.sort_order, &b.uid)));
        let mut cid: HashMap<String, i64> = HashMap::new();
        for c in s.collections.iter_mut() {
            c.id = self.id_for(&c.uid);
            c.person_id = c.person_uid.as_ref().and_then(|u| pid.get(u)).copied();
            cid.insert(c.uid.clone(), c.id);
        }
        s.notes.retain(|n| pid.contains_key(&n.person_uid) && cid.contains_key(&n.collection_uid));
        s.notes.sort_by(|a, b| (a.sort_order, -a.mentioned_at, &a.uid).cmp(&(b.sort_order, -b.mentioned_at, &b.uid)));
        for n in s.notes.iter_mut() {
            n.id = self.id_for(&n.uid);
            n.person_id = pid[&n.person_uid];
            n.collection_id = cid[&n.collection_uid];
        }
        self.snap = s;
    }

    // --- By row id, for the screens ---------------------------------------------------------------

    pub fn person_uid(&self, id: i64) -> Option<String> { self.person_by_id(id).map(|p| p.uid.clone()) }
    pub fn collection_uid(&self, id: i64) -> Option<String> { self.collection_by_id(id).map(|c| c.uid.clone()) }
    pub fn note_uid(&self, id: i64) -> Option<String> { self.snap.notes.iter().find(|n| n.id == id).map(|n| n.uid.clone()) }

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

    /// Adds someone with the three starter categories. None builds up to a date until you choose
    /// one (Gift ideas used to count down to the birthday by default, before 1.0).
    pub fn add_person(&mut self, mut p: Person) -> String {
        let t = now();
        p.uid = new_uid();
        p.updated_at = t;
        p.created_at = t;
        p.tone_index = (self.snap.people.len() % 8) as i32;
        p.sort_order = self.snap.people.iter().map(|x| x.sort_order).max().unwrap_or(0) + 1;
        for (i, (name, k, slot, occ)) in [("Gift ideas", kind::GIFT, 0, occasion::NONE), ("Plans", kind::PLAN, 1, occasion::NONE), ("Notes", kind::NOTE, 2, occasion::NONE)].into_iter().enumerate() {
            self.snap.collections.push(Collection {
                id: 0, uid: new_uid(), person_id: None, person_uid: Some(p.uid.clone()), name: name.into(), kind: k.into(),
                slot, sort_order: i as i32, occasion: occ.into(), occasion_day: None, yearly: false, updated_at: t,
            });
        }
        let uid = p.uid.clone();
        self.snap.people.push(p);
        self.renumber();
        uid
    }

    /// Saves edits to someone (including their photo key).
    pub fn update_person(&mut self, p: Person) {
        if let Some(x) = self.snap.people.iter_mut().find(|x| x.uid == p.uid) {
            *x = Person { updated_at: now(), ..p };
        }
        self.renumber();
    }

    pub fn remove_person(&mut self, uid: &str) {
        let cols: Vec<String> = self.snap.collections.iter().filter(|c| c.person_uid.as_deref() == Some(uid)).map(|c| c.uid.clone()).collect();
        let notes: Vec<String> = self.snap.notes.iter().filter(|n| n.person_uid == uid).map(|n| n.uid.clone()).collect();
        for u in cols.iter().chain(&notes).chain(std::iter::once(&uid.to_string())) { self.deleted(u); }
        self.snap.people.retain(|p| p.uid != uid);
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

    /// Replaces everything with a merged copy, keeping row ids for records that were already here.
    pub fn replace(&mut self, merged: Snapshot) {
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
        let maya = d.add_person(Person::new("Maya"));
        let sam = d.add_person(Person::new("Sam"));
        let gifts = d.categories_of(&maya)[0].uid.clone();
        let n = d.add_note(&maya, &gifts, "Blue mug", None);
        assert_eq!(d.note(&n).unwrap().person_id, d.person(&maya).unwrap().id);
        d.place_person(&sam, &maya);
        assert_eq!(d.snap.people[0].uid, sam);
        d.remove_person(&maya);
        assert!(d.note(&n).is_none());
        assert_eq!(d.snap.tombstones.len(), 5, "person, three categories, one note");
        let round = Data::new(memi_core::backup::import(&memi_core::backup::export(&d.snap, |_| None)).unwrap().snapshot);
        assert_eq!(round.snap.people.len(), 1);
    }

    #[test]
    fn ids_stay_put() {
        let mut d = Data::default();
        let a = d.add_person(Person::new("Zed"));
        let id = d.person(&a).unwrap().id;
        d.add_person(Person::new("Amy"));
        assert_eq!(d.person(&a).unwrap().id, id, "adding someone who sorts first keeps Zed's id");
        let mut merged = d.snap.clone();
        merged.people.reverse();
        d.replace(merged);
        assert_eq!(d.person(&a).unwrap().id, id);
    }
}
