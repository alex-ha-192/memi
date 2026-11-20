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
    is_birthday: bool,
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
                collection: bday.first().map(|c| (**c).clone()), is_birthday: true,
            });
        }
        for c in mine.iter().filter(|c| c.occasion == occasion::DATE) {
            if let Some(d) = dates::next_occasion(c, p, today) {
                all.push(Upcoming {
                    person: p.clone(), date: d, title: format!("{} · {}", c.name, p.first_name()),
                    open: open.get(&c.id).copied().unwrap_or(0), collection: Some((*c).clone()), is_birthday: false,
                });
            }
        }
    }
    all.into_iter()
        .filter(|u| dates::days_until(u.date, today) <= UPCOMING_DAYS)
        .min_by_key(|u| (u.date, !u.is_birthday))
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
        let gift = Collection { id: 1, person_id: Some(1), name: "Gift ideas".into(), kind: kind::GIFT.into(), slot: 0, sort_order: 0, occasion: occasion::BIRTHDAY.into(), occasion_day: None, yearly: false };
        let u = next_up(&[p], &[], &[gift], today).expect("next up");
        assert_eq!(u.title, "Maya turns 32");
    }
}
