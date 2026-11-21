//! One person: their photo and details, their categories as tiles, and what's done.

use super::home::{drag_over, note_row, set_drag, take_drag};
use super::widgets::{open_menu, Icon, Photo};
use crate::state::{app, MenuKind, Overlay};
use leptos::prelude::*;
use memi_engine::core::dates;
use memi_engine::core::model::*;

#[component]
pub fn PersonPage() -> impl IntoView {
    let a = app();
    move || {
        let selected = a.selected.get();
        let person = a.read(|d| selected.as_deref().and_then(|u| d.person(u).cloned()).or_else(|| d.snap.people.first().cloned()));
        match person {
            Some(p) => view! { <Person p=p /> }.into_any(),
            None => view! {
                <div class="empty-page">
                    <img src="/memi.png" alt="" width="120" height="120" />
                    <h2>"Memi remembers for you"</h2>
                    <p class="muted">"Add someone, then keep their gift ideas, plans and the little things they mention."</p>
                    <button class="button ink" on:click=move |_| a.overlay.set(Some(Overlay::EditPerson(None)))>"Add a person"</button>
                </div>
            }.into_any(),
        }
    }
}

#[component]
fn Person(p: Person) -> impl IntoView {
    let a = app();
    let today = dates::today();
    let uid = p.uid.clone();
    let (cats, open, done) = a.peek(|d| {
        let cats = d.categories_of(&uid);
        let open: Vec<Note> = d.snap.notes.iter().filter(|n| n.person_uid == uid && n.status == status::OPEN).cloned().collect();
        let mut done: Vec<Note> = d.snap.notes.iter().filter(|n| n.person_uid == uid && n.status == status::DONE).cloned().collect();
        done.sort_by_key(|n| -n.done_at.unwrap_or(n.mentioned_at));
        (cats, open, done)
    });
    let birthday = dates::next_birthday(&p, today).map(|d| {
        let when = format!("{} {}", dates::day(d), &dates::MONTHS_LONG[(p.birthday_month.unwrap_or(1) - 1) as usize][..3]);
        match dates::turning_age(&p, today) {
            Some(age) => format!("Birthday · {when} · turns {age}"),
            None => format!("Birthday · {when}"),
        }
    });
    let birthday_in = dates::next_birthday(&p, today).map(|d| dates::in_days(d, today));
    let (u_menu, u_edit, u_new) = (uid.clone(), uid.clone(), uid.clone());
    let narrow = move || a.width.get() < super::TWO_PANE_MIN;

    view! {
        <div class="person">
            <Show when=narrow>
                <button class="back" on:click=move |_| a.page_open.set(false)><Icon name="back" />"Your people"</button>
            </Show>
            <div class="person-head">
                <div class="portrait">
                    <Photo person=p.clone() size=220. radius=30. tall=true />
                    <button class="round on-photo" title="Edit" on:click=move |_| a.overlay.set(Some(Overlay::EditPerson(Some(u_edit.clone()))))><Icon name="more" /></button>
                </div>
                <div class="grow col gap">
                    <h1 class="name">{p.name.clone()}{p.pinned.then(|| view! { <span class="pin"><Icon name="pin" size=16. /></span> })}</h1>
                    {(!p.relationship.is_empty()).then(|| view! { <span class="muted">{p.relationship.clone()}</span> })}
                    <div class="card dates" on:click=move |e| open_menu(&e, MenuKind::Person(u_menu.clone()))>
                        <span class="eyebrow muted">"DATES"</span>
                        <b>{birthday.unwrap_or_else(|| "No birthday yet".into())}</b>
                        <span class="caption muted">{birthday_in.unwrap_or_else(|| "Add it with Edit".into())}</span>
                    </div>
                    <span class="caption muted">{format!("{} open · {} done", open.len(), done.len())}</span>
                </div>
            </div>

            <div class="section-head"><h3>"Categories"</h3><span class="caption muted">"tap to add · ⋯ for more"</span></div>
            <div class="tiles">
                {cats.into_iter().map(|c| category_tile(&p, c, &open, today)).collect_view()}
                <button class="tile new" on:click=move |_| a.overlay.set(Some(Overlay::EditCategory { person: u_new.clone(), collection: None }))>"+ New category"</button>
            </div>

            {(!done.is_empty()).then(|| view! {
                <div class="section-head"><h3>"Done"</h3><span class="label muted">{done.len()}</span></div>
                {done.into_iter().take(30).map(|n| {
                    let (t, cls) = match n.outcome { outcome::HIT => ("A hit", "plan"), outcome::MISS => ("A miss", "track"), _ => ("Done", "track") };
                    let sub = n.done_at.map(|d| format!("Done {}", dates::ago(d, today))).unwrap_or_default();
                    note_row(n, None, sub, Some((t.to_string(), cls)), false)
                }).collect_view()}
            })}
        </div>
    }
}

fn category_tile(p: &Person, c: Collection, open: &[Note], today: dates::Date) -> impl IntoView {
    let a = app();
    let mine: Vec<&Note> = open.iter().filter(|n| n.collection_uid == c.uid).collect();
    let slot = c.slot.rem_euclid(5);
    let occasion = dates::next_occasion(&c, p, today).map(|d| {
        if dates::days_until(d, today) > 60 { format!("for {}", dates::short_date(d)) }
        else { let day = dates::day_label(d, today); format!("for {}", if day == "Today" || day == "Tomorrow" { day.to_lowercase() } else { day }) }
    });
    let first = mine.first().map(|n| n.text.clone());
    let more = mine.len();
    let (pu, cu, cu2, cu3, cu4) = (p.uid.clone(), c.uid.clone(), c.uid.clone(), c.uid.clone(), c.uid.clone());
    let over = move || a.drag.with(|d| d.as_ref().and_then(|(_, o)| o.clone()).as_deref() == Some(cu4.as_str()));
    view! {
        <div class=format!("tile cat slot{slot}") class:over=over draggable="true"
            on:click=move |_| a.overlay.set(Some(Overlay::Category { person: pu.clone(), collection: cu.clone() }))
            on:contextmenu=move |e| open_menu(&e, MenuKind::Category(cu2.clone()))
            on:dragstart={let u = c.uid.clone(); move |e| set_drag(&e, &u)}
            on:dragover={let u = c.uid.clone(); move |e| drag_over(&e, &u, true)}
            on:drop={let u = c.uid.clone(); move |e| { e.prevent_default(); if let Some(from) = take_drag() { if from != u { a.change(|d| d.place_collection(&from, &u)); } } }}
            on:dragend=move |_| a.drag.set(None)>
            <div class="row between">
                <div class="col min0">
                    <b class="ellipsis">{c.name.clone()}</b>
                    {occasion.map(|o| view! { <span class=format!("caption tileink{slot}")>{o}</span> })}
                </div>
                <button class="more" title="More" on:click=move |e| open_menu(&e, MenuKind::Category(cu3.clone()))><Icon name="more" size=18. /></button>
            </div>
            {match first {
                Some(t) => view! {
                    <div class="col">
                        <span class="label ellipsis">{t}</span>
                        <span class=format!("caption tileink{slot}")>{if more > 1 { format!("+ {} more", more - 1) } else { "1 open".into() }}</span>
                    </div>
                }.into_any(),
                None => view! { <span class=format!("label tileink{slot}")>"+ Click to add"</span> }.into_any(),
            }}
        </div>
    }
}
