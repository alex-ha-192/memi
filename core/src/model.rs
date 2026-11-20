//! The same data as the Android app (app/src/main/java/app/memi/data/Entities.kt), so backups
//! made on any of the apps can be opened by the others.
//!
//! Every record has two ids: `id`, the row number on this device, and `uid`, a random id that is
//! the same on every device, so copies of your Memi can be merged item by item. `updated_at` is
//! when the record last changed, anywhere; the newest change wins.

pub mod kind {
    pub const GIFT: &str = "GIFT";
    pub const PLAN: &str = "PLAN";
    pub const NOTE: &str = "NOTE";
    pub const CUSTOM: &str = "CUSTOM";
}

pub mod occasion {
    pub const NONE: &str = "NONE";
    pub const BIRTHDAY: &str = "BIRTHDAY";
    pub const DATE: &str = "DATE";
}

pub mod status {
    pub const OPEN: i32 = 0;
    pub const DONE: i32 = 1;
}

pub mod outcome {
    pub const NONE: i32 = 0;
    pub const HIT: i32 = 1;
    pub const MISS: i32 = 2;
}

/// Five tile colours per scheme; each category gets one.
pub const SLOT_COUNT: i32 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Person {
    pub id: i64,
    pub uid: String,
    pub name: String,
    pub relationship: String,
    /// Absolute path of Your Memi's own copy of the photo.
    pub photo_path: Option<String>,
    pub birthday_month: Option<u32>,
    pub birthday_day: Option<u32>,
    pub birth_year: Option<i32>,
    pub tone_index: i32,
    pub pinned: bool,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Person {
    pub fn new(name: &str) -> Self {
        Person {
            id: 0,
            uid: String::new(),
            name: name.to_string(),
            relationship: String::new(),
            photo_path: None,
            birthday_month: None,
            birthday_day: None,
            birth_year: None,
            tone_index: 0,
            pinned: false,
            sort_order: 0,
            created_at: now_millis(),
            updated_at: 0,
        }
    }

    pub fn first_name(&self) -> &str {
        self.name.trim().split(' ').next().unwrap_or("")
    }

    pub fn has_birthday(&self) -> bool {
        self.birthday_month.is_some() && self.birthday_day.is_some()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collection {
    pub id: i64,
    pub uid: String,
    pub person_id: Option<i64>,
    pub person_uid: Option<String>,
    pub name: String,
    pub kind: String,
    pub slot: i32,
    pub sort_order: i32,
    pub occasion: String,
    /// For a dated category: days since 1970-01-01.
    pub occasion_day: Option<i64>,
    /// For a dated category: comes round every year.
    pub yearly: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub id: i64,
    pub uid: String,
    pub person_id: i64,
    pub person_uid: String,
    pub collection_id: i64,
    pub collection_uid: String,
    pub text: String,
    pub mentioned_at: i64,
    pub first_mentioned_at: i64,
    pub mention_count: i32,
    pub due_at: Option<i64>,
    pub status: i32,
    pub outcome: i32,
    pub done_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    /// Your own order within the category (lower comes first). New notes go to the top; until
    /// you drag one, notes stay newest first.
    pub sort_order: i64,
}

/// Left behind when something is deleted, so the deletion reaches your other devices instead of
/// the item coming back from their copy.
#[derive(Clone, Debug, PartialEq)]
pub struct Tombstone {
    pub uid: String,
    pub deleted_at: i64,
}

/// Everything in one Memi: what a backup holds, and what two devices merge.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub people: Vec<Person>,
    pub collections: Vec<Collection>,
    pub notes: Vec<Note>,
    pub tombstones: Vec<Tombstone>,
}

/// A new random id, as 32 hex digits (the same shape the databases make).
pub fn new_uid() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Ids for records made before Memi 0.6, worked out from what every synced device already has in
/// common (row ids are copied when a backup is restored), so devices that upgrade separately give
/// the same item the same uid. The Android app's Uids.kt does exactly the same.
pub mod legacy_uid {
    use sha2::{Digest, Sha256};
    fn hash(s: String) -> String {
        Sha256::digest(s.as_bytes()).iter().take(16).map(|b| format!("{b:02x}")).collect()
    }
    pub fn person(id: i64, created_at: i64) -> String { hash(format!("memi-person:{id}:{created_at}")) }
    pub fn collection(id: i64, person_id: Option<i64>, person_created_at: Option<i64>) -> String {
        hash(format!("memi-collection:{id}:{}:{}", person_id.unwrap_or(0), person_created_at.unwrap_or(0)))
    }
    pub fn note(id: i64, created_at: i64) -> String { hash(format!("memi-note:{id}:{created_at}")) }
}

pub fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
