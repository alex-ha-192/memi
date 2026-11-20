//! The same data as the Android app (app/src/main/java/app/memi/data/Entities.kt), so backups
//! made on either can be opened by the other.

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
}

impl Person {
    pub fn new(name: &str) -> Self {
        Person {
            id: 0,
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
    pub person_id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub slot: i32,
    pub sort_order: i32,
    pub occasion: String,
    /// For a dated category: days since 1970-01-01.
    pub occasion_day: Option<i64>,
    /// For a dated category: comes round every year.
    pub yearly: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub id: i64,
    pub person_id: i64,
    pub collection_id: i64,
    pub text: String,
    pub mentioned_at: i64,
    pub first_mentioned_at: i64,
    pub mention_count: i32,
    pub due_at: Option<i64>,
    pub status: i32,
    pub outcome: i32,
    pub done_at: Option<i64>,
    pub created_at: i64,
}

pub fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
