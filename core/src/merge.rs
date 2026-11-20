//! Combining two copies of a Memi (this device's and the one in your Google Drive) item by item.
//!
//! - Every person, category and note is matched by its uid. Where both copies have it, the one
//!   changed most recently wins; ties keep this device's version.
//! - A deletion (tombstone) beats every version of the item from before it was deleted. An item
//!   edited after it was deleted somewhere else comes back.
//! - Afterwards, anything whose person is gone is dropped, and a note whose category is gone moves
//!   to another of that person's categories, the same way every device does it.
//!
//! The Android app's Merge.kt follows exactly the same rules.

use crate::model::*;
use std::collections::{HashMap, HashSet};

/// Deletions older than this are forgotten. A device that stays offline for longer could bring
/// a deleted item back.
pub const TOMBSTONE_DAYS: i64 = 180;

pub struct Merged {
    pub snapshot: Snapshot,
    /// This device's copy needs updating to match.
    pub local_changed: bool,
    /// The copy in Drive needs updating to match.
    pub remote_changed: bool,
    /// People whose version came from Drive (their photo comes from there too).
    pub people_from_remote: HashSet<String>,
}

trait Record: Clone {
    fn uid(&self) -> &str;
    fn updated(&self) -> i64;
}
impl Record for Person { fn uid(&self) -> &str { &self.uid } fn updated(&self) -> i64 { self.updated_at } }
impl Record for Collection { fn uid(&self) -> &str { &self.uid } fn updated(&self) -> i64 { self.updated_at } }
impl Record for Note { fn uid(&self) -> &str { &self.uid } fn updated(&self) -> i64 { self.updated_at } }

/// The newer of each pair, in a stable order. Returns the uids taken from `remote`.
fn pick<T: Record>(local: &[T], remote: &[T]) -> (Vec<T>, HashSet<String>) {
    let mut out: HashMap<String, T> = local.iter().map(|r| (r.uid().to_string(), r.clone())).collect();
    let mut from_remote = HashSet::new();
    for r in remote {
        match out.get(r.uid()) {
            Some(mine) if mine.updated() >= r.updated() => {}
            _ => {
                out.insert(r.uid().to_string(), r.clone());
                from_remote.insert(r.uid().to_string());
            }
        }
    }
    let mut v: Vec<T> = out.into_values().collect();
    v.sort_by(|a, b| a.uid().cmp(b.uid()));
    (v, from_remote)
}

fn versions<T: Record>(v: &[T]) -> HashMap<&str, i64> {
    v.iter().map(|r| (r.uid(), r.updated())).collect()
}

fn same(a: &Snapshot, b: &Snapshot) -> bool {
    let tombs = |s: &Snapshot| -> HashMap<String, i64> { s.tombstones.iter().map(|t| (t.uid.clone(), t.deleted_at)).collect() };
    versions(&a.people) == versions(&b.people)
        && versions(&a.collections) == versions(&b.collections)
        && versions(&a.notes) == versions(&b.notes)
        && notes_placed(a) == notes_placed(b)
        && tombs(a) == tombs(b)
}

/// Which category each note is in (a merge can move a note without changing its time).
fn notes_placed(s: &Snapshot) -> HashMap<&str, &str> {
    s.notes.iter().map(|n| (n.uid.as_str(), n.collection_uid.as_str())).collect()
}

pub fn merge(local: &Snapshot, remote: &Snapshot, now: i64) -> Merged {
    // Deletions: the latest of each, minus very old ones.
    let mut tombs: HashMap<String, i64> = HashMap::new();
    for t in local.tombstones.iter().chain(&remote.tombstones) {
        let e = tombs.entry(t.uid.clone()).or_insert(t.deleted_at);
        *e = (*e).max(t.deleted_at);
    }
    let cutoff = now - TOMBSTONE_DAYS * 24 * 60 * 60 * 1000;
    tombs.retain(|_, at| *at >= cutoff);
    let alive = |uid: &str, updated: i64| tombs.get(uid).map_or(true, |at| updated > *at);

    let (people, people_from_remote) = pick(&local.people, &remote.people);
    let people: Vec<Person> = people.into_iter().filter(|p| alive(&p.uid, p.updated_at)).collect();
    let person_uids: HashSet<&str> = people.iter().map(|p| p.uid.as_str()).collect();

    let (cols, _) = pick(&local.collections, &remote.collections);
    let cols: Vec<Collection> = cols.into_iter()
        .filter(|c| alive(&c.uid, c.updated_at))
        .filter(|c| c.person_uid.as_deref().map_or(false, |p| person_uids.contains(p)))
        .collect();
    let col_uids: HashSet<&str> = cols.iter().map(|c| c.uid.as_str()).collect();

    // Where a note goes if its category was deleted: the person's Notes, else their first category.
    let fallback = |person: &str| -> Option<String> {
        let mut mine: Vec<&Collection> = cols.iter().filter(|c| c.person_uid.as_deref() == Some(person)).collect();
        mine.sort_by(|a, b| (a.sort_order, &a.uid).cmp(&(b.sort_order, &b.uid)));
        mine.iter().find(|c| c.kind == kind::NOTE).or(mine.first()).map(|c| c.uid.clone())
    };
    let (notes, _) = pick(&local.notes, &remote.notes);
    let notes: Vec<Note> = notes.into_iter()
        .filter(|n| alive(&n.uid, n.updated_at) && person_uids.contains(n.person_uid.as_str()))
        .filter_map(|mut n| {
            if !col_uids.contains(n.collection_uid.as_str()) { n.collection_uid = fallback(&n.person_uid)?; }
            Some(n)
        })
        .collect();

    // Keep tombstones only for things that stayed deleted.
    let survivors: HashSet<&str> = people.iter().map(|p| p.uid.as_str())
        .chain(cols.iter().map(|c| c.uid.as_str()))
        .chain(notes.iter().map(|n| n.uid.as_str()))
        .collect();
    let mut tombstones: Vec<Tombstone> = tombs.iter()
        .filter(|(uid, _)| !survivors.contains(uid.as_str()))
        .map(|(uid, at)| Tombstone { uid: uid.clone(), deleted_at: *at })
        .collect();
    tombstones.sort_by(|a, b| a.uid.cmp(&b.uid));

    let snapshot = Snapshot { people, collections: cols, notes, tombstones };
    Merged {
        local_changed: !same(&snapshot, local),
        remote_changed: !same(&snapshot, remote),
        people_from_remote: people_from_remote.into_iter().filter(|u| snapshot.people.iter().any(|p| &p.uid == u)).collect(),
        snapshot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(uid: &str, name: &str, at: i64) -> Person {
        Person { uid: uid.into(), updated_at: at, ..Person::new(name) }
    }
    fn col(uid: &str, person: &str, kind_: &str, at: i64) -> Collection {
        Collection { id: 0, uid: uid.into(), person_id: None, person_uid: Some(person.into()), name: uid.into(), kind: kind_.into(),
            slot: 0, sort_order: 0, occasion: occasion::NONE.into(), occasion_day: None, yearly: false, updated_at: at }
    }
    fn note(uid: &str, person: &str, col: &str, text: &str, at: i64) -> Note {
        Note { id: 0, uid: uid.into(), person_id: 0, person_uid: person.into(), collection_id: 0, collection_uid: col.into(),
            text: text.into(), mentioned_at: at, first_mentioned_at: at, mention_count: 1, due_at: None, status: 0, outcome: 0,
            done_at: None, created_at: at, updated_at: at, sort_order: -at }
    }
    // Close to the made-up edit times below, so no deletion counts as old.
    const NOW: i64 = 100;

    #[test]
    fn edits_on_both_sides_are_kept() {
        let base_p = person("p", "Maya", 1);
        let local = Snapshot {
            people: vec![base_p.clone()], collections: vec![col("c", "p", kind::NOTE, 1)],
            notes: vec![note("a", "p", "c", "Added on the phone", 10)], tombstones: vec![],
        };
        let remote = Snapshot {
            people: vec![Person { name: "Maya Okafor".into(), updated_at: 5, ..base_p }], collections: vec![col("c", "p", kind::NOTE, 1)],
            notes: vec![note("b", "p", "c", "Added on the computer", 11)], tombstones: vec![],
        };
        let m = merge(&local, &remote, NOW);
        assert_eq!(m.snapshot.notes.len(), 2);
        assert_eq!(m.snapshot.people[0].name, "Maya Okafor", "the newer edit wins");
        assert!(m.local_changed && m.remote_changed);
        assert!(m.people_from_remote.contains("p"));
    }

    #[test]
    fn deletions_spread_but_later_edits_win() {
        let p = person("p", "Maya", 1);
        let c = col("c", "p", kind::NOTE, 1);
        let local = Snapshot { people: vec![p.clone()], collections: vec![c.clone()],
            notes: vec![note("a", "p", "c", "Old", 5), note("b", "p", "c", "Edited after", 20)], tombstones: vec![] };
        let remote = Snapshot { people: vec![p], collections: vec![c], notes: vec![],
            tombstones: vec![Tombstone { uid: "a".into(), deleted_at: 10 }, Tombstone { uid: "b".into(), deleted_at: 10 }] };
        let m = merge(&local, &remote, NOW);
        let texts: Vec<&str> = m.snapshot.notes.iter().map(|n| n.text.as_str()).collect();
        assert_eq!(texts, vec!["Edited after"]);
        assert_eq!(m.snapshot.tombstones.len(), 1, "the tombstone for the survivor is dropped");
    }

    #[test]
    fn removing_a_person_takes_their_things_and_orphans_move() {
        let local = Snapshot {
            people: vec![person("p", "Maya", 1), person("q", "Sam", 1)],
            collections: vec![col("gone", "q", kind::GIFT, 1), col("notes", "q", kind::NOTE, 1), col("c", "p", kind::NOTE, 1)],
            notes: vec![note("x", "p", "c", "Maya's", 1), note("y", "q", "gone", "Sam's", 1)],
            tombstones: vec![],
        };
        let remote = Snapshot { tombstones: vec![Tombstone { uid: "p".into(), deleted_at: 5 }, Tombstone { uid: "gone".into(), deleted_at: 5 }], ..Default::default() };
        let m = merge(&local, &remote, NOW);
        assert_eq!(m.snapshot.people.len(), 1);
        assert_eq!(m.snapshot.notes.len(), 1);
        assert_eq!(m.snapshot.notes[0].collection_uid, "notes", "moved to Sam's Notes");
    }

    #[test]
    fn identical_copies_need_nothing() {
        let s = Snapshot { people: vec![person("p", "Maya", 3)], ..Default::default() };
        let m = merge(&s, &s, NOW);
        assert!(!m.local_changed && !m.remote_changed);
    }

    #[test]
    fn merging_is_the_same_from_both_sides() {
        let a = Snapshot { people: vec![person("p", "A", 2), person("r", "R", 1)], ..Default::default() };
        let b = Snapshot { people: vec![person("p", "B", 3)], tombstones: vec![Tombstone { uid: "r".into(), deleted_at: 9 }], ..Default::default() };
        assert_eq!(merge(&a, &b, NOW).snapshot, merge(&b, &a, NOW).snapshot);
    }
}
