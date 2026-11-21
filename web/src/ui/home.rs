//! The list: header, Coming up, Still fresh and everyone; or search results.

use super::widgets::{open_menu, tag, Icon, Photo};
use crate::state::{app, MenuKind, Overlay};
use memi_engine::Problem;
use crate::sync;
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_engine::core::model::*;
use memi_engine::core::{dates, insights};

const NEXT_EVENTS: usize = 3;
const FRESH_SHOWN: usize = 6;

#[component]
pub fn Home() -> impl IntoView {
    let a = app();
    view! {
        <Header />
        <SyncBanner />
        {move || if a.search.get().is_some() { view! { <SearchResults /> }.into_any() } else { view! { <Lists /> }.into_any() }}
    }
}

#[component]
fn Header() -> impl IntoView {
    let a = app();
    let input = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| { if a.search.get().is_some() { if let Some(i) = input.get() { let _ = i.focus(); } } });
    view! {
        <header class="list-header">
            {move || if a.search.get().is_some() {
                view! {
                    <input class="search" node_ref=input placeholder="Search people and notes"
                        prop:value=move || a.search.get().unwrap_or_default()
                        on:input=move |e| a.search.set(Some(event_target_value(&e))) />
                    <button class="round" title="Close search" on:click=move |_| a.search.set(None)><Icon name="x" /></button>
                }.into_any()
            } else {
                view! {
                    <div class="grow">
                        <div class="date-line">
                            <span class="caption muted">{move || {
                                let t = dates::today();
                                format!("{} {}", dates::weekday_short(t), dates::short_date(t))
                            }}</span>
                            <SyncChip />
                        </div>
                        <h1>"Your people"</h1>
                    </div>
                    <button class="round" class:on=move || a.theme_open.get() title="Settings" aria-label="Settings" on:click=move |_| a.theme_open.update(|o| *o = !*o)><Icon name="settings" /></button>
                    <button class="round" title="Search" on:click=move |_| a.search.set(Some(String::new()))><Icon name="search" /></button>
                    <button class="round" title="Add a person" on:click=move |_| a.overlay.set(Some(Overlay::EditPerson(None)))><Icon name="plus" /></button>
                }.into_any()
            }}
        </header>
    }
}

/// Sync in a word or two, beside the date: Synced, Syncing…, Not synced or Sign in. Opens Sync.
/// Routine checks in the background don't show as Syncing.
#[component]
fn SyncChip() -> impl IntoView {
    let a = app();
    let state = move || {
        let st = a.status();
        let on = a.sync_settings().enabled;
        if matches!(st.problem, Some(Problem::NeedsSignIn)) { ("signin", "cloud-alert", "Sign in") }
        else if st.busy && (!on || st.problem.is_some()) { ("syncing", "refresh", "Syncing…") }
        else if on && st.problem.is_none() { ("synced", "cloud-check", "Synced") }
        else { ("off", "cloud-off", "Not synced") }
    };
    view! {
        <button class=move || format!("sync-chip {}", state().0)
            aria-label=move || format!("Sync: {}. Open Sync", state().2)
            on:click=move |_| { a.theme_open.set(false); a.overlay.set(Some(Overlay::Sync)); }>
            {move || view! { <Icon name=state().1 size=15. /> }}
            <span>{move || state().2}</span>
        </button>
    }
}

/// When sync is on but paused (Google's sign-in lasts an hour in a browser).
#[component]
fn SyncBanner() -> impl IntoView {
    let a = app();
    move || {
        let on = a.memi.with(|m| m.sync.enabled);
        match a.memi.with(|m| m.status.problem.clone()) {
            Some(Problem::NeedsSignIn) if on => view! {
                <div class="banner">
                    <span class="grow">"Sync is paused until you reconnect to Google."</span>
                    <button class="chip ink" on:click=move |_| spawn_local(sync::connect(a, false))>"Reconnect"</button>
                </div>
            }.into_any(),
            Some(Problem::NeedsKey) if !on => ().into_any(),
            _ => ().into_any(),
        }
    }
}

#[component]
fn Lists() -> impl IntoView {
    let a = app();
    let joins = move || a.memi.with(|m| m.status.joins.clone()).into_iter().map(|j| {
        let (f1, f2) = (j.file_id.clone(), j.file_id.clone());
        view! {
            <div class="join card">
                <b>{format!("{} wants to join your Memi", j.device)}</b>
                <span class="caption muted">"Allow it only if it shows this code:"</span>
                <span class="code">{j.confirm.clone()}</span>
                <div class="row gap">
                    <button class="button ink grow" on:click=move |_| spawn_local(sync::answer_join(a, f1.clone(), true))>"Allow"</button>
                    <button class="button grow" on:click=move |_| spawn_local(sync::answer_join(a, f2.clone(), false))>"Not mine"</button>
                </div>
            </div>
        }
    }).collect_view();
    // Sync is on but they haven't said they've kept the recovery key anywhere.
    let key_nudge = move || a.memi.with(|m| m.wants_key_nudge(memi_engine::core::model::now_millis())).then(|| view! {
        <div class="join card">
            <b>"Save your recovery key"</b>
            <span class="caption muted">"If you lose this browser's data and your other devices, it's the only way to open your synced copy. Keep it somewhere safe, like a password manager."</span>
            <div class="row gap">
                <button class="button ink grow" on:click=move |_| { a.reveal_key.set(true); a.theme_open.set(false); a.overlay.set(Some(Overlay::Sync)); }>"Show key"</button>
                <button class="button grow" on:click=move |_| memi_engine::sync::Handle::with(&a.handle(), |m| m.snooze_key_nudge(memi_engine::core::model::now_millis()))>"Not now"</button>
            </div>
        </div>
    });

    let hero = move || {
        let (events, empty, open_total, people) = a.read(|d| {
            let s = &d.snap;
            (insights::next_events(&s.people, &s.notes, &s.collections, dates::today(), NEXT_EVENTS), s.people.is_empty(),
             s.notes.iter().filter(|n| n.status == status::OPEN).count(), s.people.len())
        });
        if empty {
            view! {
                <button class="tile gift welcome" on:click=move |_| a.overlay.set(Some(Overlay::EditPerson(None)))>
                    <div class="grow">
                        <h2>"Hi, I'm Memi"</h2>
                        <p class="tileink0">"Add someone you care about, then keep their gift ideas, plans and little things to remember in one place."</p>
                        <span class="tag white">"+ Add a person"</span>
                    </div>
                    <img src="/memi.png" alt="" width="96" height="96" />
                </button>
            }.into_any()
        } else if events.is_empty() {
            view! {
                <div class="tile plan quiet">
                    <div class="grow">
                        <h2>"Nothing coming up"</h2>
                        <p class="tileink1">{format!("{open_total} open across {people} {}.", if people == 1 { "person" } else { "people" })}</p>
                    </div>
                    <img src="/memi.png" alt="" width="84" height="84" />
                </div>
            }.into_any()
        } else {
            let today = dates::today();
            view! {
                <div class="tile gift events">
                    <div class="row between"><span class="eyebrow tileink0">"COMING UP"</span><img src="/memi.png" alt="" width="34" height="34" /></div>
                    {events.into_iter().map(|u| {
                        let detail = match (&u.note, &u.collection) {
                            (Some(_), Some(c)) => format!("{} · {}", u.person.first_name(), c.name),
                            (Some(_), None) => u.person.first_name().to_string(),
                            // A birthday with no category building up to it: say who they are.
                            (None, None) => if u.person.relationship.is_empty() { "Birthday".to_string() } else { u.person.relationship.clone() },
                            (None, Some(c)) => {
                                let what = c.name.to_lowercase();
                                match u.open { 0 => format!("Nothing in {what} yet"), 1 => format!("1 thing in {what}"), n => format!("{n} things in {what}") }
                            }
                        };
                        let (pu, cu) = (u.person.uid.clone(), u.collection.as_ref().map(|c| c.uid.clone()));
                        view! {
                            <button class="event" on:click=move |_| {
                                a.open_person(pu.clone());
                                if let Some(c) = &cu { a.overlay.set(Some(Overlay::Category { person: pu.clone(), collection: c.clone() })); }
                            }>
                                <span class="date-chip"><b>{dates::short_day_label(u.date, today)}</b><span class="caption muted">{dates::in_days(u.date, today)}</span></span>
                                <span class="grow min0 col">
                                    <b class="ellipsis">{u.title.clone()}</b>
                                    <span class="label tileink0 ellipsis">{detail}</span>
                                </span>
                                <Photo person=u.person.clone() size=40. radius=12. />
                            </button>
                        }
                    }).collect_view()}
                </div>
            }.into_any()
        }
    };

    let fresh = move || {
        let collapsed = a.settings.with(|s| s.fresh_collapsed);
        let items = a.read(|d| insights::still_fresh(&d.snap.notes, &d.snap.people, &d.snap.collections, dates::today()));
        if items.is_empty() { return ().into_any(); }
        let count = items.len();
        let today = dates::today();
        view! {
            <div class="section-head">
                <button class="fold" on:click=move |_| a.update_settings(|s| s.fresh_collapsed = !s.fresh_collapsed)>
                    <h3>"Still fresh"</h3><Icon name=if collapsed { "right" } else { "down" } size=18. />
                </button>
                <span class="label muted">{if collapsed { format!("{count} hidden") } else { "new or due soon".into() }}</span>
            </div>
            {(!collapsed).then(|| items.into_iter().take(FRESH_SHOWN).map(|f| {
                let (t, cls) = match f.due {
                    Some(d) => (format!("Due {}", dates::short_day_label(d, today)), "gift"),
                    None => (format!("New · {}", dates::ago_short(f.note.mentioned_at, today)), "plan"),
                };
                let sub = format!("{} · {}", f.person.first_name(), f.collection.as_ref().map(|c| c.name.clone()).unwrap_or_default());
                note_row(f.note.clone(), Some(f.person.clone()), sub, Some((t, cls)), false)
            }).collect_view())}
        }.into_any()
    };

    let everyone = move || {
        let (people, notes) = a.read(|d| (d.snap.people.clone(), d.snap.notes.clone()));
        if people.is_empty() { return ().into_any(); }
        let n = people.len();
        view! {
            <div class="section-head"><h3>"Everyone"</h3><span class="label muted">{n}</span></div>
            {people.into_iter().map(|p| person_row(p.clone(), insights::open_count(p.id, &notes), true)).collect_view()}
        }.into_any()
    };

    view! {
        <div class="joins">{joins}{key_nudge}</div>
        {hero}
        {fresh}
        {everyone}
    }
}

/// One person in the list; draggable to reorder when `drag` is set.
pub fn person_row(p: Person, open: usize, drag: bool) -> impl IntoView {
    let a = app();
    let (u1, u2, u3, u4, u5) = (p.uid.clone(), p.uid.clone(), p.uid.clone(), p.uid.clone(), p.uid.clone());
    let selected = move || a.selected.get().as_deref() == Some(u1.as_str()) && a.width.get() >= super::TWO_PANE_MIN;
    let over = move || a.drag.with(|d| d.as_ref().and_then(|(_, o)| o.clone()).as_deref() == Some(u2.as_str()));
    // Look A4: tinted in their tone, relationship and open count on the second line.
    let t = p.tone_index.rem_euclid(8);
    let tint = format!("--pc:var(--pcard{t});--pi:var(--pink{t})");
    let subtitle = [Some(p.relationship.clone()).filter(|r| !r.is_empty()), (open > 0).then(|| format!("{open} open"))]
        .into_iter().flatten().collect::<Vec<_>>().join(" · ");
    view! {
        <div class="person-row" style=tint class:selected=selected class:over=over draggable=if drag { "true" } else { "false" }
            on:click=move |_| a.open_person(u3.clone())
            on:contextmenu=move |e| open_menu(&e, MenuKind::Person(u4.clone()))
            on:dragstart=move |e| { if drag { set_drag(&e, &u5); } }
            on:dragover={let u = p.uid.clone(); move |e| drag_over(&e, &u, drag)}
            on:dragleave=move |_| a.drag.update(|d| if let Some((_, o)) = d { *o = None })
            on:drop={let u = p.uid.clone(); move |e| { e.prevent_default(); if let Some(from) = take_drag() { if from != u { a.change(|d| d.place_person(&from, &u)); } } }}
            on:dragend=move |_| a.drag.set(None)>
            <Photo person=p.clone() size=46. radius=14. />
            <div class="grow min0 col">
                <span class="item ellipsis">{p.name.clone()}{p.pinned.then(|| view! { <span class="pin"><Icon name="pin" size=13. /></span> })}</span>
                {(!subtitle.is_empty()).then(|| view! { <span class="caption sub ellipsis">{subtitle}</span> })}
            </div>
            <button class="more" title="More" on:click={let u = p.uid.clone(); move |e| open_menu(&e, MenuKind::Person(u.clone()))}><Icon name="more" size=18. /></button>
        </div>
    }
}

/// A note as a card. Clicking edits it; ⋯ or right-click for done, hit, miss, move, delete.
pub fn note_row(n: Note, person: Option<Person>, subtitle: String, tag_: Option<(String, &'static str)>, drag: bool) -> impl IntoView {
    let a = app();
    let (u1, u2, u3, u4) = (n.uid.clone(), n.uid.clone(), n.uid.clone(), n.uid.clone());
    let over = move || a.drag.with(|d| d.as_ref().and_then(|(_, o)| o.clone()).as_deref() == Some(u4.as_str()));
    view! {
        <div class="note card" class:over=over class:done=n.status == status::DONE draggable=if drag { "true" } else { "false" }
            on:click=move |_| a.overlay.set(Some(Overlay::EditNote(u1.clone())))
            on:contextmenu=move |e| open_menu(&e, MenuKind::Note(u2.clone()))
            on:dragstart={let u = n.uid.clone(); move |e| { if drag { set_drag(&e, &u); } }}
            on:dragover={let u = n.uid.clone(); move |e| drag_over(&e, &u, drag)}
            on:drop={let u = n.uid.clone(); move |e| { e.prevent_default(); if let Some(from) = take_drag() { if from != u { a.change(|d| d.place_note(&from, &u)); } } }}
            on:dragend=move |_| a.drag.set(None)>
            {person.map(|p| view! { <Photo person=p size=44. radius=15. /> })}
            <div class="grow min0 col">
                <span class="item">{n.text.clone()}</span>
                <span class="caption muted ellipsis">{subtitle}</span>
            </div>
            {tag_.map(|(t, c)| tag(t, c))}
            <button class="more" title="More" on:click=move |e| open_menu(&e, MenuKind::Note(u3.clone()))><Icon name="more" size=18. /></button>
        </div>
    }
}

// --- Drag and drop (HTML5): the dragged uid is kept in app state ----------------------------------

pub fn set_drag(e: &web_sys::DragEvent, uid: &str) {
    if let Some(dt) = e.data_transfer() { let _ = dt.set_data("text/plain", uid); dt.set_effect_allowed("move"); }
    app().drag.set(Some((uid.to_string(), None)));
}

pub fn drag_over(e: &web_sys::DragEvent, uid: &str, enabled: bool) {
    if !enabled { return; }
    let a = app();
    let same = a.drag.with_untracked(|d| d.as_ref().map(|(from, _)| from == uid));
    if same.is_some() { e.prevent_default(); }
    if same == Some(false) && a.drag.with_untracked(|d| d.as_ref().and_then(|(_, o)| o.as_deref()) != Some(uid)) {
        a.drag.update(|d| if let Some((_, o)) = d { *o = Some(uid.to_string()) });
    }
}

pub fn take_drag() -> Option<String> {
    let a = app();
    let from = a.drag.get_untracked().map(|(f, _)| f);
    a.drag.set(None);
    from
}

#[component]
fn SearchResults() -> impl IntoView {
    let a = app();
    move || {
        let q = a.search.get().unwrap_or_default().trim().to_lowercase();
        if q.is_empty() { return view! { <p class="caption muted pad">"Type a name, or something you noted down."</p> }.into_any(); }
        let (people, notes) = a.read(|d| {
            let people: Vec<Person> = d.snap.people.iter().filter(|p| p.name.to_lowercase().contains(&q) || p.relationship.to_lowercase().contains(&q)).cloned().collect();
            let notes: Vec<(Note, Person, String)> = d.snap.notes.iter().filter(|n| n.text.to_lowercase().contains(&q)).take(30)
                .filter_map(|n| Some((n.clone(), d.person_by_id(n.person_id)?.clone(), d.collection_by_id(n.collection_id).map(|c| c.name.clone()).unwrap_or_default())))
                .collect();
            (people, notes)
        });
        if people.is_empty() && notes.is_empty() {
            return view! { <p class="body muted pad">{format!("Nothing matches “{q}”.")}</p> }.into_any();
        }
        view! {
            {people.into_iter().map(|p| person_row(p, 0, false)).collect_view()}
            {notes.into_iter().map(|(n, p, c)| { let sub = format!("{} · {c}", p.first_name()); note_row(n, Some(p), sub, None, false) }).collect_view()}
        }.into_any()
    }
}
