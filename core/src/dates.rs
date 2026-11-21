//! Dates and the friendly labels for them, matching the Android app's Dates.kt.

use crate::model::{occasion, Collection, Person};
use jiff::{civil, tz::TimeZone, Timestamp, Unit};

/// A calendar day, in the person's own time zone where it comes from a moment.
pub type Date = civil::Date;

pub fn today() -> Date {
    jiff::Zoned::now().date()
}

fn zone() -> TimeZone {
    TimeZone::system()
}

/// The local day a moment (milliseconds since 1970) falls on.
pub fn to_date(millis: i64) -> Date {
    Timestamp::from_millisecond(millis).map(|t| t.to_zoned(zone()).date()).unwrap_or_else(|_| today())
}

/// The end of the given day, as milliseconds (used for "by Friday" deadlines).
pub fn end_of_day(date: Date) -> i64 {
    date.at(23, 59, 0, 0).to_zoned(zone()).map(|z| z.timestamp().as_millisecond()).unwrap_or(0)
}

const EPOCH: Date = civil::date(1970, 1, 1);

pub fn from_epoch_day(day: i64) -> Option<Date> {
    add_days(EPOCH, day)
}

pub fn epoch_day(date: Date) -> i64 {
    days_until(date, EPOCH)
}

/// A day from year, month (1–12) and day of the month; None if there's no such day.
pub fn ymd(year: i32, month: u32, day: u32) -> Option<Date> {
    Date::new(i16::try_from(year).ok()?, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()
}

pub fn year(d: Date) -> i32 { d.year() as i32 }
pub fn month(d: Date) -> u32 { d.month() as u32 }
pub fn day(d: Date) -> u32 { d.day() as u32 }

/// 0 for Monday … 6 for Sunday.
pub fn weekday_from_monday(d: Date) -> u32 {
    d.weekday().to_monday_zero_offset() as u32
}

pub fn add_days(d: Date, n: i64) -> Option<Date> {
    d.checked_add(jiff::Span::new().try_days(n).ok()?).ok()
}

/// The same day of the month `n` months on (or back); the 31st becomes the month's last day.
pub fn add_months(d: Date, n: i32) -> Option<Date> {
    d.checked_add(jiff::Span::new().try_months(n).ok()?).ok()
}

/// "2026-11-21", as date inputs use.
pub fn iso(d: Date) -> String {
    d.to_string()
}

pub fn parse_iso(text: &str) -> Option<Date> {
    text.trim().parse().ok()
}

fn on(year: i32, month: u32, day: u32) -> Option<Date> {
    // Clamp to the month's length (29 February in a normal year becomes the 28th).
    (0..4).find_map(|back| ymd(year, month, day.saturating_sub(back)))
}

pub fn next_birthday(p: &Person, from: Date) -> Option<Date> {
    let (m, d) = (p.birthday_month?, p.birthday_day?);
    let this_year = on(year(from), m, d)?;
    if this_year < from { on(year(from) + 1, m, d) } else { Some(this_year) }
}

/// The next day a category is building up to: their birthday, a one-off date, or an anniversary.
pub fn next_occasion(c: &Collection, p: &Person, from: Date) -> Option<Date> {
    match c.occasion.as_str() {
        occasion::BIRTHDAY => next_birthday(p, from),
        occasion::DATE => {
            let d = from_epoch_day(c.occasion_day?)?;
            if c.yearly {
                let this_year = on(year(from), month(d), day(d))?;
                if this_year < from { on(year(from) + 1, month(d), day(d)) } else { Some(this_year) }
            } else if d < from {
                None
            } else {
                Some(d)
            }
        }
        _ => None,
    }
}

pub fn turning_age(p: &Person, from: Date) -> Option<i32> {
    Some(year(next_birthday(p, from)?) - p.birth_year?)
}

/// Whole days from `from` to `date` (negative if `date` is earlier).
pub fn days_until(date: Date, from: Date) -> i64 {
    from.until((Unit::Day, date)).map(|s| s.get_days() as i64).unwrap_or(0)
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
pub const MONTHS_LONG: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];

fn weekday(date: Date) -> &'static str {
    ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][weekday_from_monday(date) as usize]
}

/// "Sat".
pub fn weekday_short(date: Date) -> &'static str {
    &weekday(date)[..3]
}

/// "22 Nov".
pub fn short_date(date: Date) -> String {
    format!("{} {}", day(date), MONTHS[month(date) as usize - 1])
}

/// "Today", "Tomorrow", "Sunday" (within a week) or "22 Nov".
pub fn day_label(date: Date, from: Date) -> String {
    match days_until(date, from) {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        2..=6 => weekday(date).into(),
        _ => short_date(date),
    }
}

/// "Thu" within a week, else "22 Nov".
pub fn short_day_label(date: Date, from: Date) -> String {
    match days_until(date, from) {
        0 => "today".into(),
        1 => "tomorrow".into(),
        2..=6 => weekday(date)[..3].into(),
        _ => short_date(date),
    }
}

/// "in 6 days".
pub fn in_days(date: Date, from: Date) -> String {
    match days_until(date, from) {
        0 => "today".into(),
        1 => "tomorrow".into(),
        n if n < 0 => format!("{} days ago", -n),
        n => format!("in {n} days"),
    }
}

/// "today", "yesterday", "3 days ago", "last week", "in March 2025" (as on Android).
pub fn ago(millis: i64, now: Date) -> String {
    let d = to_date(millis);
    let days = days_until(now, d);
    match days {
        i64::MIN..=0 => "today".into(),
        1 => "yesterday".into(),
        2..=6 => format!("{days} days ago"),
        7..=13 => "last week".into(),
        14..=30 => format!("{} weeks ago", days / 7),
        31..=59 => "last month".into(),
        60..=329 => format!("{} months ago", days / 30),
        _ => format!("in {} {}", MONTHS_LONG[month(d) as usize - 1], year(d)),
    }
}

/// "2d", "3w".
pub fn ago_short(millis: i64, now: Date) -> String {
    let days = days_until(now, to_date(millis)).max(0);
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
    fn calendar_helpers_match_the_old_chrono_behaviour() {
        let d = ymd(2026, 11, 23).unwrap(); // a Monday
        assert_eq!(weekday_from_monday(d), 0);
        assert_eq!(weekday_short(d), "Mon");
        assert_eq!(add_days(d, 8), ymd(2026, 12, 1));
        // Adding a month to the 31st lands on the last day of a shorter month.
        assert_eq!(add_months(ymd(2026, 1, 31).unwrap(), 1), ymd(2026, 2, 28));
        assert_eq!(add_months(ymd(2026, 12, 15).unwrap(), 1), ymd(2027, 1, 15));
        assert_eq!(iso(d), "2026-11-23");
        assert_eq!(parse_iso("2026-11-23"), Some(d));
        assert_eq!(parse_iso("not a date"), None);
        assert_eq!(from_epoch_day(epoch_day(d)), Some(d));
        assert_eq!(epoch_day(ymd(1970, 1, 2).unwrap()), 1);
        assert_eq!(days_until(ymd(2027, 1, 1).unwrap(), d), 39);
        assert_eq!(ymd(2026, 2, 30), None);
        assert_eq!(to_date(1_795_262_400_000).year(), 2026);
    }

    #[test]
    fn leap_day_birthdays_fall_on_the_28th() {
        let mut p = Person::new("Sam");
        p.birthday_month = Some(2);
        p.birthday_day = Some(29);
        let from = ymd(2027, 1, 10).unwrap();
        assert_eq!(next_birthday(&p, from), ymd(2027, 2, 28));
    }

    #[test]
    fn past_one_off_dates_are_gone() {
        let c = Collection {
            id: 1, uid: "c1".into(), person_id: Some(1), person_uid: Some("p1".into()), updated_at: 0, name: "Trip".into(), kind: "CUSTOM".into(), slot: 3, sort_order: 3,
            occasion: occasion::DATE.into(), occasion_day: Some(epoch_day(ymd(2026, 1, 1).unwrap())), yearly: false,
        };
        let p = Person::new("Sam");
        assert_eq!(next_occasion(&c, &p, ymd(2026, 3, 1).unwrap()), None);
        let yearly = Collection { yearly: true, ..c };
        assert_eq!(next_occasion(&yearly, &p, ymd(2026, 3, 1).unwrap()), ymd(2027, 1, 1));
    }
}
