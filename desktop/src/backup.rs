//! The sync backup: everything as one JSON document, in exactly the same shape as the Android
//! app's Backup.kt, so a phone and a computer can open each other's backups. It's only ever
//! stored encrypted (see sync/crypto.rs).

use crate::model::*;
use crate::photos;
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Map, Value};
use std::path::Path;

const FORMAT: i64 = 1;

pub struct Contents {
    pub people: Vec<Person>,
    pub collections: Vec<Collection>,
    pub notes: Vec<Note>,
}

pub fn export(people: &[Person], cols: &[Collection], notes: &[Note]) -> Vec<u8> {
    let people: Vec<Value> = people.iter().map(|p| {
        let photo = p.photo_path.as_deref().and_then(|path| std::fs::read(path).ok()).map(|b| STANDARD.encode(b));
        json!({
            "id": p.id, "name": p.name, "relationship": p.relationship, "photo": photo,
            "birthdayMonth": p.birthday_month, "birthdayDay": p.birthday_day, "birthYear": p.birth_year,
            "toneIndex": p.tone_index, "pinned": p.pinned, "sortOrder": p.sort_order, "createdAt": p.created_at,
        })
    }).collect();
    let cols: Vec<Value> = cols.iter().map(|c| json!({
        "id": c.id, "personId": c.person_id, "name": c.name, "kind": c.kind, "slot": c.slot, "sortOrder": c.sort_order,
        "occasion": c.occasion, "occasionDay": c.occasion_day, "yearly": c.yearly,
    })).collect();
    let notes: Vec<Value> = notes.iter().map(|n| json!({
        "id": n.id, "personId": n.person_id, "collectionId": n.collection_id, "text": n.text,
        "mentionedAt": n.mentioned_at, "firstMentionedAt": n.first_mentioned_at, "mentionCount": n.mention_count,
        "dueAt": n.due_at, "status": n.status, "outcome": n.outcome, "doneAt": n.done_at, "createdAt": n.created_at,
    })).collect();
    let root = json!({
        "format": FORMAT, "exportedAt": now_millis(),
        "people": people, "collections": cols, "notes": notes,
    });
    serde_json::to_vec(&root).expect("JSON")
}

fn int(o: &Map<String, Value>, k: &str) -> Option<i64> { o.get(k).and_then(Value::as_i64) }
fn text(o: &Map<String, Value>, k: &str) -> Option<String> { o.get(k).and_then(Value::as_str).map(str::to_string) }
fn flag(o: &Map<String, Value>, k: &str) -> bool { o.get(k).and_then(Value::as_bool).unwrap_or(false) }

/// Reads a backup. New photos are written into `photo_dir`; the caller swaps the data in.
pub fn import(bytes: &[u8], photo_dir: &Path) -> Result<Contents> {
    let root: Value = serde_json::from_slice(bytes).context("Backup isn't valid")?;
    ensure!(root.get("format").and_then(Value::as_i64).unwrap_or(0) <= FORMAT, "This backup was made by a newer version of Your Memi.");
    let list = |k: &str| -> Vec<Map<String, Value>> {
        root.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_object().cloned()).collect()).unwrap_or_default()
    };
    let mut people = vec![];
    for o in list("people") {
        let photo = match o.get("photo").and_then(Value::as_str) {
            Some(b64) if !b64.is_empty() => STANDARD.decode(b64).ok()
                .and_then(|b| photos::save_bytes(&b, photo_dir).ok())
                .map(|p| p.to_string_lossy().into_owned()),
            _ => None,
        };
        people.push(Person {
            id: int(&o, "id").context("person id")?,
            name: text(&o, "name").context("person name")?,
            relationship: text(&o, "relationship").unwrap_or_default(),
            photo_path: photo,
            birthday_month: int(&o, "birthdayMonth").map(|v| v as u32).filter(|m| (1..=12).contains(m)),
            birthday_day: int(&o, "birthdayDay").map(|v| v as u32).filter(|d| (1..=31).contains(d)),
            birth_year: int(&o, "birthYear").map(|v| v as i32),
            tone_index: int(&o, "toneIndex").unwrap_or(0) as i32,
            pinned: flag(&o, "pinned"),
            sort_order: int(&o, "sortOrder").unwrap_or(0) as i32,
            created_at: int(&o, "createdAt").unwrap_or_else(now_millis),
        });
    }
    let collections = list("collections").into_iter().map(|o| Ok(Collection {
        id: int(&o, "id").context("category id")?,
        person_id: int(&o, "personId"),
        name: text(&o, "name").context("category name")?,
        kind: text(&o, "kind").unwrap_or_else(|| kind::CUSTOM.into()),
        slot: (int(&o, "slot").unwrap_or(0) as i32).rem_euclid(SLOT_COUNT),
        sort_order: int(&o, "sortOrder").unwrap_or(0) as i32,
        occasion: text(&o, "occasion").unwrap_or_else(|| occasion::NONE.into()),
        occasion_day: int(&o, "occasionDay"),
        yearly: flag(&o, "yearly"),
    })).collect::<Result<Vec<_>>>()?;
    let notes = list("notes").into_iter().map(|o| {
        let mentioned = int(&o, "mentionedAt").context("note time")?;
        Ok(Note {
            id: int(&o, "id").context("note id")?,
            person_id: int(&o, "personId").context("note person")?,
            collection_id: int(&o, "collectionId").context("note category")?,
            text: text(&o, "text").context("note text")?,
            mentioned_at: mentioned,
            first_mentioned_at: int(&o, "firstMentionedAt").unwrap_or(mentioned),
            mention_count: int(&o, "mentionCount").unwrap_or(1) as i32,
            due_at: int(&o, "dueAt"),
            status: int(&o, "status").unwrap_or(0) as i32,
            outcome: int(&o, "outcome").unwrap_or(0) as i32,
            done_at: int(&o, "doneAt"),
            created_at: int(&o, "createdAt").unwrap_or_else(now_millis),
        })
    }).collect::<Result<Vec<_>>>()?;
    Ok(Contents { people, collections, notes })
}
