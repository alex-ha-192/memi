//! What's fresh, what's due, what's coming up next. Same rules as the Android Insights.kt.

use crate::dates;
use crate::model::*;
use chrono::NaiveDate;
use std::collections::HashMap;

const FRESH_DAYS: i64 = 14;
const SOON_DAYS: i64 = 14;
const UPCOMING_DAYS: i64 = 30;

pub struct FreshItem {
    pub note: Note,
    pub person: Person,
    pub collection: Option<Collection>,
    /// Set when something is due soon; otherwise it's simply new.
    pub due: Option<NaiveDate>,
}

/// A note's own deadline, or else the next date its category is building up to.
pub fn effective_due(n: &Note, p: &Person, c: Option<&Collection>, today: NaiveDate) -> Option<NaiveDate> {
    if let Some(d) = n.due_at { return Some(dates::to_date(d)); }
    c.and_then(|c| dates::next_occasion(c, p, today))
}

pub fn still_fresh(notes: &[Note], people: &[Person], cols: &[Collection], today: NaiveDate) -> Vec<FreshItem> {
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
            let fresh = (today - dates::to_date(n.mentioned_at)).num_days() <= FRESH_DAYS;
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
    pub date: NaiveDate,
    pub title: String,
    pub open: usize,
    pub collection: Option<Collection>,
    /// Set when this is a dated note (a plan or a note with a due date).
    pub note: Option<Note>,
    pub is_birthday: bool,
}

/// The next birthday or dated category within a month, for the big tile.
pub fn next_up(people: &[Person], notes: &[Note], cols: &[Collection], today: NaiveDate) -> Option<Upcoming> {
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
pub fn next_events(people: &[Person], notes: &[Note], cols: &[Collection], today: NaiveDate, count: usize) -> Vec<Upcoming> {
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

pub fn open_count(person_id: i64, notes: &[Note]) -> usize {
    notes.iter().filter(|n| n.person_id == person_id && n.status == status::OPEN).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birthday_in_six_days_is_next_up() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
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
            due_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 11, 21).unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp_millis()),
            status: status::OPEN, outcome: outcome::NONE, done_at: None, created_at: 0, updated_at: 0, sort_order: 0,
        };
        let events = next_events(&[p], &[plan], &[gift], today, 3);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "Book the restaurant");
        assert_eq!(events[1].title, "Maya turns 32");
    }
}
