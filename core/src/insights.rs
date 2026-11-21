//! What's fresh, what's due, what's coming up next. Same rules as the Android Insights.kt.

use crate::dates;
use crate::model::*;
use crate::dates::Date;
use std::collections::HashMap;

const FRESH_DAYS: i64 = 14;
const SOON_DAYS: i64 = 14;
const UPCOMING_DAYS: i64 = 30;

pub struct FreshItem {
    pub note: Note,
    pub person: Person,
    pub collection: Option<Collection>,
    /// Set when something is due soon; otherwise it's simply new.
    pub due: Option<Date>,
}

/// A note's own deadline, or else the next date its category is building up to.
pub fn effective_due(n: &Note, p: &Person, c: Option<&Collection>, today: Date) -> Option<Date> {
    if let Some(d) = n.due_at { return Some(dates::to_date(d)); }
    c.and_then(|c| dates::next_occasion(c, p, today))
}

pub fn still_fresh(notes: &[Note], people: &[Person], cols: &[Collection], today: Date) -> Vec<FreshItem> {
    let by_id: HashMap<i64, &Person> = people.iter().map(|p| (p.id, p)).collect();
    let cols: HashMap<i64, &Collection> = cols.iter().map(|c| (c.id, c)).collect();
    let mut out: Vec<FreshItem> = notes
        .iter()
        .filter(|n| n.status == status::OPEN)
        .filter_map(|n| {
            let p = *by_id.get(&n.person_id)?;
            let c = cols.get(&n.collection_id).copied();
            let due = effective_due(n, p, c, today);
            let due_soon = due.map(|d| dates::days_until(d, today) <= SOON_DAYS).unwrap_or(false);
            let fresh = dates::days_until(today, dates::to_date(n.mentioned_at)) <= FRESH_DAYS;
            if !due_soon && !fresh { return None; }
            Some(FreshItem { note: n.clone(), person: p.clone(), collection: c.cloned(), due: if due_soon { due } else { None } })
        })
        .collect();
    out.sort_by(|a, b| {
        (a.due.is_none(), a.due, std::cmp::Reverse(a.note.mentioned_at)).cmp(&(b.due.is_none(), b.due, std::cmp::Reverse(b.note.mentioned_at)))
    });
    out
}

pub struct Upcoming {
    pub person: Person,
    pub date: Date,
    pub title: String,
    pub open: usize,
    pub collection: Option<Collection>,
    /// Set when this is a dated note (a plan or a note with a due date).
    pub note: Option<Note>,
    pub is_birthday: bool,
}

/// The next birthday or dated category within a month, for the big tile.
pub fn next_up(people: &[Person], notes: &[Note], cols: &[Collection], today: Date) -> Option<Upcoming> {
    let mut open: HashMap<i64, usize> = HashMap::new();
    for n in notes.iter().filter(|n| n.status == status::OPEN) {
        *open.entry(n.collection_id).or_default() += 1;
    }
    let mut all = vec![];
    for p in people {
        let mine: Vec<&Collection> = cols.iter().filter(|c| c.person_id == Some(p.id)).collect();
        if let Some(d) = dates::next_birthday(p, today) {
            let bday: Vec<&&Collection> = mine.iter().filter(|c| c.occasion == occasion::BIRTHDAY).collect();
            let title = match dates::turning_age(p, today) {
                Some(age) => format!("{} turns {age}", p.first_name()),
                None => format!("{}'s birthday", p.first_name()),
            };
            all.push(Upcoming {
                person: p.clone(), date: d, title,
                open: bday.iter().map(|c| open.get(&c.id).copied().unwrap_or(0)).sum(),
                collection: bday.first().map(|c| (**c).clone()), note: None, is_birthday: true,
            });
        }
        for c in mine.iter().filter(|c| c.occasion == occasion::DATE) {
            if let Some(d) = dates::next_occasion(c, p, today) {
                all.push(Upcoming {
                    person: p.clone(), date: d, title: format!("{} · {}", c.name, p.first_name()),
                    open: open.get(&c.id).copied().unwrap_or(0), collection: Some((*c).clone()), note: None, is_birthday: false,
                });
            }
        }
    }
    all.into_iter()
        .filter(|u| dates::days_until(u.date, today) <= UPCOMING_DAYS)
        .min_by_key(|u| (u.date, !u.is_birthday))
}

/// The next few things coming up, soonest first, however far away: birthdays, dated categories
/// (an anniversary, a trip), and open plans or notes that have their own date. For the big tile.
pub fn next_events(people: &[Person], notes: &[Note], cols: &[Collection], today: Date, count: usize) -> Vec<Upcoming> {
    let mut open: HashMap<i64, usize> = HashMap::new();
    for n in notes.iter().filter(|n| n.status == status::OPEN) {
        *open.entry(n.collection_id).or_default() += 1;
    }
    let by_id: HashMap<i64, &Person> = people.iter().map(|p| (p.id, p)).collect();
    let col_by_id: HashMap<i64, &Collection> = cols.iter().map(|c| (c.id, c)).collect();
    let mut all = vec![];
    for p in people {
        let mine: Vec<&Collection> = cols.iter().filter(|c| c.person_id == Some(p.id)).collect();
        if let Some(d) = dates::next_birthday(p, today) {
            let bday: Vec<&&Collection> = mine.iter().filter(|c| c.occasion == occasion::BIRTHDAY).collect();
            let title = match dates::turning_age(p, today) {
                Some(age) => format!("{} turns {age}", p.first_name()),
                None => format!("{}'s birthday", p.first_name()),
            };
            all.push(Upcoming {
                person: p.clone(), date: d, title,
                open: bday.iter().map(|c| open.get(&c.id).copied().unwrap_or(0)).sum(),
                collection: bday.first().map(|c| (**c).clone()), note: None, is_birthday: true,
            });
        }
        for c in mine.iter().filter(|c| c.occasion == occasion::DATE) {
            if let Some(d) = dates::next_occasion(c, p, today) {
                all.push(Upcoming {
                    person: p.clone(), date: d, title: format!("{} · {}", c.name, p.first_name()),
                    open: open.get(&c.id).copied().unwrap_or(0), collection: Some((*c).clone()), note: None, is_birthday: false,
                });
            }
        }
    }
    for n in notes.iter().filter(|n| n.status == status::OPEN) {
        let (Some(due), Some(p)) = (n.due_at, by_id.get(&n.person_id)) else { continue };
        let d = dates::to_date(due);
        if d < today { continue; }
        all.push(Upcoming {
            person: (*p).clone(), date: d, title: n.text.clone(), open: 0,
            collection: col_by_id.get(&n.collection_id).map(|c| (*c).clone()), note: Some(n.clone()), is_birthday: false,
        });
    }
    all.sort_by_key(|u| (u.date, !u.is_birthday, u.note.is_some()));
    all.truncate(count);
    all
}

/// Days before a birthday that reminders can be set for: on the day, the day before, a week before.
pub const REMINDER_DAYS: [i64; 3] = [0, 1, 7];

/// A birthday reminder that's due.
pub struct Reminder {
    pub person: Person,
    pub date: Date,
    pub days_before: i64,
    /// Remembered once shown, so each reminder appears once: "<person uid>:<date>:<days before>".
    pub key: String,
    pub title: String,
    pub body: String,
}

/// The birthday reminders due today for the chosen `days` (any of [REMINDER_DAYS]).
pub fn reminders_due(people: &[Person], notes: &[Note], cols: &[Collection], today: Date, days: &[i64]) -> Vec<Reminder> {
    let mut out = vec![];
    for p in people {
        let Some(date) = dates::next_birthday(p, today) else { continue };
        let before = dates::days_until(date, today);
        if !days.contains(&before) { continue; }
        let name = p.first_name();
        let title = match (before, dates::turning_age(p, today)) {
            (0, Some(age)) => format!("{name} turns {age} today"),
            (0, None) => format!("It's {name}'s birthday today"),
            (1, Some(age)) => format!("{name} turns {age} tomorrow"),
            (1, None) => format!("{name}'s birthday is tomorrow"),
            (_, Some(age)) => format!("{name} turns {age} in a week"),
            (_, None) => format!("{name}'s birthday is in a week"),
        };
        let gift: Vec<&Collection> = cols.iter().filter(|c| c.person_id == Some(p.id) && c.occasion == occasion::BIRTHDAY).collect();
        let open = notes.iter().filter(|n| n.status == status::OPEN && gift.iter().any(|c| c.id == n.collection_id)).count();
        let body = match gift.first().map(|c| c.name.to_lowercase()) {
            // Nothing building up to the birthday: just a nudge.
            None => format!("See what you've noted for {name}."),
            Some(what) => match open { 0 => format!("Nothing in {what} yet."), 1 => format!("1 thing in {what}."), n => format!("{n} things in {what}.") },
        };
        out.push(Reminder { key: format!("{}:{date}:{before}", p.uid), person: p.clone(), date, days_before: before, title, body });
    }
    out
}

pub fn open_count(person_id: i64, notes: &[Note]) -> usize {
    notes.iter().filter(|n| n.person_id == person_id && n.status == status::OPEN).count()
}

#[cfg(test)]
mod reminder_tests {
    use super::*;

    #[test]
    fn reminders_on_the_chosen_days() {
        let today = dates::ymd(2026, 11, 20).unwrap();
        let mut maya = Person::new("Maya Okafor");
        maya.uid = "m".into();
        (maya.birthday_month, maya.birthday_day, maya.birth_year) = (Some(11), Some(27), Some(1994));
        let mut sam = Person::new("Sam");
        sam.uid = "s".into();
        (sam.birthday_month, sam.birthday_day) = (Some(11), Some(20));
        let people = vec![maya, sam];
        let r = reminders_due(&people, &[], &[], today, &[0]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "It's Sam's birthday today");
        assert_eq!(r[0].key, "s:2026-11-20:0");
        let r = reminders_due(&people, &[], &[], today, &[7]);
        assert_eq!(r[0].title, "Maya turns 32 in a week");
        assert!(reminders_due(&people, &[], &[], today, &[]).is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birthday_in_six_days_is_next_up() {
        let today = dates::ymd(2026, 11, 20).unwrap();
        let mut p = Person::new("Maya Okafor");
        p.id = 1;
        p.birthday_month = Some(11);
        p.birthday_day = Some(26);
        p.birth_year = Some(1994);
        let gift = Collection { id: 1, uid: "c1".into(), person_id: Some(1), person_uid: Some("p1".into()), updated_at: 0, name: "Gift ideas".into(), kind: kind::GIFT.into(), slot: 0, sort_order: 0, occasion: occasion::BIRTHDAY.into(), occasion_day: None, yearly: false };
        let u = next_up(&[p.clone()], &[], &[gift.clone()], today).expect("next up");
        assert_eq!(u.title, "Maya turns 32");

        // A plan due tomorrow comes before the birthday; the list stops at three.
        let plan = Note {
            id: 1, uid: "n1".into(), person_id: 1, person_uid: "p1".into(), collection_id: 1, collection_uid: "c1".into(),
            text: "Book the restaurant".into(), mentioned_at: 0, first_mentioned_at: 0, mention_count: 1,
            due_at: Some(1_795_262_400_000 /* 21 Nov 2026, 12:00 UTC */),
            status: status::OPEN, outcome: outcome::NONE, done_at: None, created_at: 0, updated_at: 0, sort_order: 0,
        };
        let events = next_events(&[p], &[plan], &[gift], today, 3);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "Book the restaurant");
        assert_eq!(events[1].title, "Maya turns 32");
    }
}
