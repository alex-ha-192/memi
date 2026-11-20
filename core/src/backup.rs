//! The sync backup: everything in one JSON document, in exactly the same shape as the Android
//! app's Backup.kt, so every app can open every other app's backups. It's only ever stored
//! encrypted (see crypto.rs).
//!
//! Format 2 (Memi 0.6) adds a permanent `uid` and an `updatedAt` time to every record, refers to
//! parents by uid (`personUid`, `collectionUid`), and lists deletions as `tombstones`, so two
//! copies can be merged item by item (merge.rs). The local row ids are still written, but only
//! mean something on the device that wrote them. Format 1 backups (Memi 0.5) still open: their
//! records get the same uids an upgraded device gives them (model::legacy_uid), so they merge.

use crate::model::*;
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub const FORMAT: i64 = 2;

pub struct Doc {
    pub snapshot: Snapshot,
    /// Photo bytes by person uid.
    pub photos: HashMap<String, Vec<u8>>,
    /// Written by Memi 0.5 (no edit times or deletions recorded).
    pub legacy: bool,
}

/// `photo` returns the bytes of a person's photo, if they have one.
pub fn export(s: &Snapshot, photo: impl Fn(&Person) -> Option<Vec<u8>>) -> Vec<u8> {
    let people: Vec<Value> = s.people.iter().map(|p| json!({
        "id": p.id, "uid": p.uid, "updatedAt": p.updated_at,
        "name": p.name, "relationship": p.relationship, "photo": photo(p).map(|b| STANDARD.encode(b)),
        "birthdayMonth": p.birthday_month, "birthdayDay": p.birthday_day, "birthYear": p.birth_year,
        "toneIndex": p.tone_index, "pinned": p.pinned, "sortOrder": p.sort_order, "createdAt": p.created_at,
    })).collect();
    let cols: Vec<Value> = s.collections.iter().map(|c| json!({
        "id": c.id, "uid": c.uid, "updatedAt": c.updated_at, "personId": c.person_id, "personUid": c.person_uid,
        "name": c.name, "kind": c.kind, "slot": c.slot, "sortOrder": c.sort_order,
        "occasion": c.occasion, "occasionDay": c.occasion_day, "yearly": c.yearly,
    })).collect();
    let notes: Vec<Value> = s.notes.iter().map(|n| json!({
        "id": n.id, "uid": n.uid, "updatedAt": n.updated_at,
        "personId": n.person_id, "personUid": n.person_uid, "collectionId": n.collection_id, "collectionUid": n.collection_uid,
        "text": n.text, "mentionedAt": n.mentioned_at, "firstMentionedAt": n.first_mentioned_at, "mentionCount": n.mention_count,
        "dueAt": n.due_at, "status": n.status, "outcome": n.outcome, "doneAt": n.done_at, "createdAt": n.created_at,
        "sortOrder": n.sort_order,
    })).collect();
    let tombstones: Vec<Value> = s.tombstones.iter().map(|t| json!({ "uid": t.uid, "deletedAt": t.deleted_at })).collect();
    let root = json!({
        "format": FORMAT, "exportedAt": now_millis(),
        "people": people, "collections": cols, "notes": notes, "tombstones": tombstones,
    });
    serde_json::to_vec(&root).expect("JSON")
}

fn int(o: &Map<String, Value>, k: &str) -> Option<i64> { o.get(k).and_then(Value::as_i64) }
fn text(o: &Map<String, Value>, k: &str) -> Option<String> { o.get(k).and_then(Value::as_str).map(str::to_string) }
fn flag(o: &Map<String, Value>, k: &str) -> bool { o.get(k).and_then(Value::as_bool).unwrap_or(false) }
fn uid(o: &Map<String, Value>, legacy: impl FnOnce() -> String) -> String { text(o, "uid").filter(|u| !u.is_empty()).unwrap_or_else(legacy) }

pub fn import(bytes: &[u8]) -> Result<Doc> {
    let root: Value = serde_json::from_slice(bytes).context("Backup isn't valid")?;
    let format = root.get("format").and_then(Value::as_i64).unwrap_or(0);
    ensure!(format <= FORMAT, "This backup was made by a newer version of Memi. Update this app to open it.");
    let list = |k: &str| -> Vec<Map<String, Value>> {
        root.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_object().cloned()).collect()).unwrap_or_default()
    };

    let mut photos = HashMap::new();
    let mut person_uid: HashMap<i64, String> = HashMap::new();
    let mut person_created: HashMap<i64, i64> = HashMap::new();
    let mut people = vec![];
    for o in list("people") {
        let id = int(&o, "id").context("person id")?;
        let created_at = int(&o, "createdAt").unwrap_or_else(now_millis);
        let p = Person {
            id,
            uid: uid(&o, || legacy_uid::person(id, created_at)),
            name: text(&o, "name").context("person name")?,
            relationship: text(&o, "relationship").unwrap_or_default(),
            photo_path: None,
            birthday_month: int(&o, "birthdayMonth").map(|v| v as u32).filter(|m| (1..=12).contains(m)),
            birthday_day: int(&o, "birthdayDay").map(|v| v as u32).filter(|d| (1..=31).contains(d)),
            birth_year: int(&o, "birthYear").map(|v| v as i32),
            tone_index: int(&o, "toneIndex").unwrap_or(0) as i32,
            pinned: flag(&o, "pinned"),
            sort_order: int(&o, "sortOrder").unwrap_or(0) as i32,
            created_at,
            updated_at: int(&o, "updatedAt").unwrap_or(0),
        };
        person_created.insert(id, created_at);
        if let Some(bytes) = o.get("photo").and_then(Value::as_str).filter(|b| !b.is_empty()).and_then(|b| STANDARD.decode(b).ok()) {
            photos.insert(p.uid.clone(), bytes);
        }
        person_uid.insert(p.id, p.uid.clone());
        people.push(p);
    }

    let mut col_uid: HashMap<i64, String> = HashMap::new();
    let mut collections = vec![];
    for o in list("collections") {
        let person_id = int(&o, "personId");
        let id = int(&o, "id").context("category id")?;
        let c = Collection {
            id,
            uid: uid(&o, || legacy_uid::collection(id, person_id, person_id.and_then(|p| person_created.get(&p).copied()))),
            person_id,
            person_uid: text(&o, "personUid").or_else(|| person_id.and_then(|id| person_uid.get(&id).cloned())),
            name: text(&o, "name").context("category name")?,
            kind: text(&o, "kind").unwrap_or_else(|| kind::CUSTOM.into()),
            slot: (int(&o, "slot").unwrap_or(0) as i32).rem_euclid(SLOT_COUNT),
            sort_order: int(&o, "sortOrder").unwrap_or(0) as i32,
            occasion: text(&o, "occasion").unwrap_or_else(|| occasion::NONE.into()),
            occasion_day: int(&o, "occasionDay"),
            yearly: flag(&o, "yearly"),
            updated_at: int(&o, "updatedAt").unwrap_or(0),
        };
        col_uid.insert(c.id, c.uid.clone());
        collections.push(c);
    }

    let mut notes = vec![];
    for o in list("notes") {
        let mentioned = int(&o, "mentionedAt").context("note time")?;
        let person_id = int(&o, "personId").context("note person")?;
        let collection_id = int(&o, "collectionId").context("note category")?;
        let (Some(pu), Some(cu)) = (
            text(&o, "personUid").or_else(|| person_uid.get(&person_id).cloned()),
            text(&o, "collectionUid").or_else(|| col_uid.get(&collection_id).cloned()),
        ) else { continue };
        let id = int(&o, "id").context("note id")?;
        let created_at = int(&o, "createdAt").unwrap_or_else(now_millis);
        notes.push(Note {
            id,
            uid: uid(&o, || legacy_uid::note(id, created_at)),
            person_id,
            person_uid: pu,
            collection_id,
            collection_uid: cu,
            text: text(&o, "text").context("note text")?,
            mentioned_at: mentioned,
            first_mentioned_at: int(&o, "firstMentionedAt").unwrap_or(mentioned),
            mention_count: int(&o, "mentionCount").unwrap_or(1) as i32,
            due_at: int(&o, "dueAt"),
            status: int(&o, "status").unwrap_or(0) as i32,
            outcome: int(&o, "outcome").unwrap_or(0) as i32,
            done_at: int(&o, "doneAt"),
            created_at,
            updated_at: int(&o, "updatedAt").unwrap_or(0),
            sort_order: int(&o, "sortOrder").unwrap_or(-mentioned),
        });
    }

    let tombstones = list("tombstones").into_iter()
        .filter_map(|o| Some(Tombstone { uid: text(&o, "uid")?, deleted_at: int(&o, "deletedAt")? }))
        .collect();
    Ok(Doc { snapshot: Snapshot { people, collections, notes, tombstones }, photos, legacy: format < 2 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_uids_parents_tombstones_and_photos() {
        let mut p = Person::new("Maya");
        p.id = 7; p.uid = "p".into(); p.updated_at = 5;
        let c = Collection { id: 3, uid: "c".into(), person_id: Some(7), person_uid: Some("p".into()), name: "Gift ideas".into(),
            kind: kind::GIFT.into(), slot: 0, sort_order: 0, occasion: occasion::BIRTHDAY.into(), occasion_day: None, yearly: false, updated_at: 6 };
        let n = Note { id: 9, uid: "n".into(), person_id: 7, person_uid: "p".into(), collection_id: 3, collection_uid: "c".into(),
            text: "Mug".into(), mentioned_at: 1, first_mentioned_at: 1, mention_count: 1, due_at: None, status: 0, outcome: 0, done_at: None, created_at: 1, updated_at: 8, sort_order: -1 };
        let s = Snapshot { people: vec![p], collections: vec![c], notes: vec![n], tombstones: vec![Tombstone { uid: "gone".into(), deleted_at: 4 }] };
        let doc = import(&export(&s, |_| Some(vec![1, 2, 3]))).unwrap();
        assert!(!doc.legacy);
        assert_eq!(doc.snapshot, s);
        assert_eq!(doc.photos.get("p"), Some(&vec![1, 2, 3]));
    }

    #[test]
    fn old_backups_get_uids_linked_by_row_id() {
        let old = br#"{"format":1,"people":[{"id":1,"name":"Sam","createdAt":5}],"collections":[{"id":2,"personId":1,"name":"Notes"}],
            "notes":[{"id":3,"personId":1,"collectionId":2,"text":"Black coffee","mentionedAt":10}]}"#;
        let doc = import(old).unwrap();
        assert!(doc.legacy);
        let s = doc.snapshot;
        assert_eq!(s.collections[0].person_uid.as_deref(), Some(s.people[0].uid.as_str()));
        assert_eq!(s.notes[0].collection_uid, s.collections[0].uid);
        assert_eq!(s.people[0].uid, legacy_uid::person(1, 5), "the same uid an upgraded device gives Sam");
    }

    /// The Android app's BackupTest checks the same values.
    #[test]
    fn legacy_uids_are_fixed() {
        assert_eq!(legacy_uid::person(3, 1700000000000), "a1e4984a7e39eea1336fa23191d15348");
        assert_eq!(legacy_uid::collection(7, Some(3), Some(1700000000000)), "4fd968f882732d432b8ffa516af1ea10");
        assert_eq!(legacy_uid::collection(9, None, None), "6e53e5417e60db96d0d6bda4d1d200a5");
        assert_eq!(legacy_uid::note(12, 1700000000123), "ed39b1b44db4f7dc2b2ecabca8a47f90");
    }

    #[test]
    fn newer_formats_are_refused() {
        assert!(import(br#"{"format":3}"#).is_err());
    }
}
