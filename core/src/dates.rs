//! Dates and the friendly labels for them, matching the Android app's Dates.kt.

use crate::model::{occasion, Collection, Person};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn to_date(millis: i64) -> NaiveDate {
    Local.timestamp_millis_opt(millis).single().map(|t| t.date_naive()).unwrap_or_else(today)
}

/// The end of the given day, as milliseconds (used for "by Friday" deadlines).
pub fn end_of_day(date: NaiveDate) -> i64 {
    let t = date.and_hms_opt(23, 59, 0).unwrap();
    Local.from_local_datetime(&t).earliest().map(|t| t.timestamp_millis()).unwrap_or(0)
}

pub fn from_epoch_day(day: i64) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(1970, 1, 1)?.checked_add_signed(Duration::days(day))
}

pub fn epoch_day(date: NaiveDate) -> i64 {
    (date - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days()
}

fn on(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    // Clamp to the month's length (29 February in a normal year becomes the 28th).
    (0..4).find_map(|back| NaiveDate::from_ymd_opt(year, month, day.saturating_sub(back)))
}

pub fn next_birthday(p: &Person, from: NaiveDate) -> Option<NaiveDate> {
    let (m, d) = (p.birthday_month?, p.birthday_day?);
    let this_year = on(from.year(), m, d)?;
    if this_year < from { on(from.year() + 1, m, d) } else { Some(this_year) }
}

/// The next day a category is building up to: their birthday, a one-off date, or an anniversary.
pub fn next_occasion(c: &Collection, p: &Person, from: NaiveDate) -> Option<NaiveDate> {
    match c.occasion.as_str() {
        occasion::BIRTHDAY => next_birthday(p, from),
        occasion::DATE => {
            let d = from_epoch_day(c.occasion_day?)?;
            if c.yearly {
                let this_year = on(from.year(), d.month(), d.day())?;
                if this_year < from { on(from.year() + 1, d.month(), d.day()) } else { Some(this_year) }
            } else if d < from {
                None
            } else {
                Some(d)
            }
        }
        _ => None,
    }
}

pub fn turning_age(p: &Person, from: NaiveDate) -> Option<i32> {
    Some(next_birthday(p, from)?.year() - p.birth_year?)
}

pub fn days_until(date: NaiveDate, from: NaiveDate) -> i64 {
    (date - from).num_days()
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
pub const MONTHS_LONG: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];

fn weekday(date: NaiveDate) -> &'static str {
    ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][date.weekday().num_days_from_monday() as usize]
}

/// "22 Nov".
pub fn short_date(date: NaiveDate) -> String {
    format!("{} {}", date.day(), MONTHS[date.month0() as usize])
}

/// "Today", "Tomorrow", "Sunday" (within a week) or "22 Nov".
pub fn day_label(date: NaiveDate, from: NaiveDate) -> String {
    match days_until(date, from) {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        2..=6 => weekday(date).into(),
        _ => short_date(date),
    }
}

/// "Thu" within a week, else "22 Nov".
pub fn short_day_label(date: NaiveDate, from: NaiveDate) -> String {
    match days_until(date, from) {
        0 => "today".into(),
        1 => "tomorrow".into(),
        2..=6 => weekday(date)[..3].into(),
        _ => short_date(date),
    }
}

/// "in 6 days".
pub fn in_days(date: NaiveDate, from: NaiveDate) -> String {
    match days_until(date, from) {
        0 => "today".into(),
        1 => "tomorrow".into(),
        n if n < 0 => format!("{} days ago", -n),
        n => format!("in {n} days"),
    }
}

/// "today", "yesterday", "3 days ago", "last week", "in March 2025" (as on Android).
pub fn ago(millis: i64, now: NaiveDate) -> String {
    let d = to_date(millis);
    let days = (now - d).num_days();
    match days {
        i64::MIN..=0 => "today".into(),
        1 => "yesterday".into(),
        2..=6 => format!("{days} days ago"),
        7..=13 => "last week".into(),
        14..=30 => format!("{} weeks ago", days / 7),
        31..=59 => "last month".into(),
        60..=329 => format!("{} months ago", days / 30),
        _ => format!("in {} {}", MONTHS_LONG[d.month0() as usize], d.year()),
    }
}

/// "2d", "3w".
pub fn ago_short(millis: i64, now: NaiveDate) -> String {
    let days = (now - to_date(millis)).num_days().max(0);
    match days {
        0 => "today".into(),
        1..=6 => format!("{days}d"),
        _ => format!("{}w", days / 7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_day_birthdays_fall_on_the_28th() {
        let mut p = Person::new("Sam");
        p.birthday_month = Some(2);
        p.birthday_day = Some(29);
        let from = NaiveDate::from_ymd_opt(2027, 1, 10).unwrap();
        assert_eq!(next_birthday(&p, from), NaiveDate::from_ymd_opt(2027, 2, 28));
    }

    #[test]
    fn past_one_off_dates_are_gone() {
        let c = Collection {
            id: 1, uid: "c1".into(), person_id: Some(1), person_uid: Some("p1".into()), updated_at: 0, name: "Trip".into(), kind: "CUSTOM".into(), slot: 3, sort_order: 3,
            occasion: occasion::DATE.into(), occasion_day: Some(epoch_day(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())), yearly: false,
        };
        let p = Person::new("Sam");
        assert_eq!(next_occasion(&c, &p, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()), None);
        let yearly = Collection { yearly: true, ..c };
        assert_eq!(next_occasion(&yearly, &p, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()), NaiveDate::from_ymd_opt(2027, 1, 1));
    }
}
