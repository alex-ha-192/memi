//! Sheets over the page: a category's notes, editing a note, a person or a category, confirmations,
//! and Sync.

use super::home::note_row;
use super::widgets::{Icon, Photo};
use crate::state::{app, Confirm, Overlay, Undo};
use memi_engine::Problem;
use crate::{js, sync};
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_engine::core::dates;
use memi_engine::core::model::*;
use wasm_bindgen::JsCast;

#[component]
pub fn OverlayHost() -> impl IntoView {
    let a = app();
    move || {
        let Some(o) = a.overlay.get() else { return ().into_any() };
        let body = match o {
            Overlay::Category { person, collection } => view! { <CategorySheet person=person collection=collection /> }.into_any(),
            Overlay::EditNote(uid) => view! { <NoteEditor uid=uid /> }.into_any(),
            Overlay::EditPerson(uid) => view! { <PersonEditor uid=uid /> }.into_any(),
            Overlay::EditCategory { person, collection } => view! { <CategoryEditor person=person uid=collection /> }.into_any(),
            Overlay::Confirm(c) => view! { <ConfirmSheet what=c /> }.into_any(),
            Overlay::Sync => view! { <SyncSheet /> }.into_any(),
        };
        view! {
            <div class="scrim" on:click=move |_| a.close_overlay()></div>
            <div class="sheet" role="dialog">{body}</div>
        }.into_any()
    }
}

fn title(text: impl Into<String>) -> impl IntoView {
    let a = app();
    let t: String = text.into();
    view! {
        <div class="sheet-title">
            <h2 class="grow">{t}</h2>
            <button class="round" title="Close" on:click=move |_| a.close_overlay()><Icon name="x" /></button>
        </div>
    }
}

fn input_value(e: &leptos::ev::Event) -> String { event_target_value(e) }

// --- A category -------------------------------------------------------------------------------------

#[component]
fn CategorySheet(person: String, collection: String) -> impl IntoView {
    let a = app();
    let text = RwSignal::new(String::new());
    let input = NodeRef::<leptos::html::Input>::new();
    let (pu, cu) = (StoredValue::new(person.clone()), StoredValue::new(collection.clone()));
    let add = move || {
        let t = text.get_untracked();
        if t.trim().is_empty() { return; }
        let (pu, cu) = (pu.get_value(), cu.get_value());
        let name = a.peek(|d| d.collection(&cu).map(|c| c.name.clone())).unwrap_or_default();
        let uid = a.change(|d| d.add_note(&pu, &cu, &t, None));
        text.set(String::new());
        a.show_toast(format!("Added to {name}"), Some(Undo::RemoveNote(uid)));
    };
    Effect::new(move |_| { if let Some(i) = input.get() { let _ = i.focus(); } });
    let cu = collection.clone();
    let header = move || {
        let (c, p) = a.read(|d| (d.collection(&cu).cloned(), d.person(&person).cloned()));
        let (Some(c), Some(p)) = (c, p) else { return view! { {title("Removed")} }.into_any() };
        let today = dates::today();
        let occ = dates::next_occasion(&c, &p, today).map(|d| format!(" · for {} · {}", dates::day_label(d, today), dates::in_days(d, today))).unwrap_or_default();
        view! { {title(c.name.clone())} <p class="caption muted">{format!("{}{occ}", p.name)}</p> }.into_any()
    };
    let cu = collection.clone();
    let slot = a.peek(|d| d.collection(&cu).map(|c| c.slot.rem_euclid(5)).unwrap_or(0));
    let list = move || {
        let notes = a.read(|d| d.open_in(&cu));
        if notes.is_empty() { return view! { <p class="muted">"Nothing here yet. Type above and press Enter."</p> }.into_any(); }
        let today = dates::today();
        view! {
            {notes.into_iter().map(|n| {
                let sub = match n.due_at {
                    Some(d) => format!("Added {} · due {}", dates::ago(n.mentioned_at, today), dates::short_day_label(dates::to_date(d), today)),
                    None => format!("Added {}", dates::ago(n.mentioned_at, today)),
                };
                note_row(n, None, sub, None, true)
            }).collect_view()}
            <p class="caption muted">"Click to edit, drag to reorder. Use ⋯ or right-click to mark done, a hit or a miss."</p>
        }.into_any()
    };
    view! {
        {header}
        <div class=format!("add-row slot{slot}")>
            <input node_ref=input placeholder="Add something…" prop:value=move || text.get()
                on:input=move |e| text.set(input_value(&e))
                on:keydown=move |e| if e.key() == "Enter" { add() } />
            <button class="add" class:ready=move || !text.get().trim().is_empty() title="Add" on:click=move |_| add()><Icon name="plus" /></button>
        </div>
        <div class="col gap">{list}</div>
    }
}

// --- Editing a note ---------------------------------------------------------------------------------

#[component]
fn NoteEditor(uid: String) -> impl IntoView {
    let a = app();
    let Some(n) = a.peek(|d| d.note(&uid).cloned()) else { return ().into_any() };
    let text = RwSignal::new(n.text.clone());
    let due = RwSignal::new(n.due_at.map(|d| dates::iso(dates::to_date(d))).unwrap_or_default());
    let n_save = n.clone();
    let save = move |_| {
        let t = text.get_untracked();
        if t.trim().is_empty() { return; }
        let due_at = dates::parse_iso(&due.get_untracked()).map(dates::end_of_day);
        a.change(|d| d.update_note(Note { text: t.trim().to_string(), due_at, ..n_save.clone() }));
        a.close_overlay();
    };
    let n_del = n.clone();
    view! {
        {title("Edit")}
        <textarea rows="4" prop:value=move || text.get() on:input=move |e| text.set(input_value(&e))></textarea>
        <label class="field"><span class="label muted">"Due (optional)"</span>
            <input type="date" prop:value=move || due.get() on:input=move |e| due.set(input_value(&e)) />
        </label>
        <div class="row gap">
            <button class="button danger" on:click=move |_| {
                a.change(|d| d.remove_note(&n_del.uid));
                a.show_toast("Deleted", Some(Undo::RestoreNote(n_del.clone())));
                a.close_overlay();
            }>"Delete"</button>
            <span class="grow"></span>
            <button class="button" on:click=move |_| a.close_overlay()>"Cancel"</button>
            <button class="button ink" on:click=save>"Save"</button>
        </div>
    }.into_any()
}

// --- Adding or editing a person ------------------------------------------------------------------------

#[component]
fn PersonEditor(uid: Option<String>) -> impl IntoView {
    let a = app();
    let existing = uid.as_ref().and_then(|u| a.peek(|d| d.person(u).cloned()));
    let base = existing.clone().unwrap_or_else(|| Person::new(""));
    let name = RwSignal::new(base.name.clone());
    let rel = RwSignal::new(base.relationship.clone());
    let month = RwSignal::new(base.birthday_month);
    let day = RwSignal::new(base.birthday_day.map(|d| d.to_string()).unwrap_or_default());
    let year = RwSignal::new(base.birth_year.map(|d| d.to_string()).unwrap_or_default());
    // None: unchanged; Some(None): removed; Some(Some): new photo.
    let photo = RwSignal::new(None::<Option<Vec<u8>>>);
    let preview = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let had_photo = uid.as_ref().map(|u| a.peek(|d| d.person(u).is_some_and(|p| p.photo_path.is_some()))).unwrap_or(false);
    let file = NodeRef::<leptos::html::Input>::new();
    let editing = existing.is_some();

    let pick = move |_| {
        let Some(input) = file.get() else { return };
        let Some(f) = input.files().and_then(|l| l.get(0)) else { return };
        busy.set(true);
        spawn_local(async move {
            match js::crop_photo(&f).await {
                Ok(bytes) => {
                    if let Some(old) = preview.get_untracked() { js::revoke_url(&old); }
                    preview.set(js::blob_url(&bytes));
                    photo.set(Some(Some(bytes)));
                }
                Err(e) => app().show_toast(format!("Couldn't open that picture: {e}"), None),
            }
            busy.set(false);
        });
    };
    let base_save = base.clone();
    let save = move |_| {
        let n = name.get_untracked();
        if n.trim().is_empty() { return; }
        let d = day.get_untracked().trim().parse::<u32>().ok().filter(|d| (1..=31).contains(d));
        let m = month.get_untracked();
        let p = Person {
            name: n.trim().into(), relationship: rel.get_untracked().trim().into(),
            birthday_month: if d.is_some() { m } else { None }, birthday_day: if m.is_some() { d } else { None },
            birth_year: year.get_untracked().trim().parse::<i32>().ok().filter(|y| (1900..=2100).contains(y)),
            ..base_save.clone()
        };
        let ph = photo.get_untracked();
        if editing {
            let change = match ph {
                None => memi_engine::PhotoChange::Keep,
                Some(None) => memi_engine::PhotoChange::Remove,
                Some(Some(b)) => memi_engine::PhotoChange::Set(b),
            };
            a.with_memi(|m| m.update_person(p, change));
        } else {
            let new_uid = a.with_memi(|m| m.add_person(p, ph.flatten()));
            a.open_person(new_uid);
        }
        a.close_overlay();
    };
    let shown_photo = {
        let base = base.clone();
        move || match (preview.get(), photo.get()) {
            (Some(url), _) => view! { <img class="photo" src=url alt="" style="width:96px;height:120px;border-radius:20px" /> }.into_any(),
            (None, Some(None)) => view! { <Photo person=Person { uid: String::new(), ..base.clone() } size=96. radius=20. tall=true /> }.into_any(),
            _ => view! { <Photo person=base.clone() size=96. radius=20. tall=true /> }.into_any(),
        }
    };
    view! {
        {title(if editing { format!("Edit {}", base.first_name()) } else { "New person".into() })}
        <div class="row gap top">
            <div class="col gap center">
                {shown_photo}
                <label class="chip">{move || if busy.get() { "Opening…" } else { "Choose photo" }}
                    <input type="file" accept="image/*" node_ref=file on:change=pick hidden />
                </label>
                {move || ((had_photo && photo.get().is_none()) || matches!(photo.get(), Some(Some(_)))).then(|| view! {
                    <button class="chip" on:click=move |_| { photo.set(Some(None)); preview.set(None); }>"Remove"</button>
                })}
            </div>
            <div class="grow col gap">
                <label class="field"><span class="label muted">"Name"</span>
                    <input placeholder="Maya Okafor" prop:value=move || name.get() on:input=move |e| name.set(input_value(&e)) /></label>
                <label class="field"><span class="label muted">"Who they are to you (optional)"</span>
                    <input placeholder="Sister, friend from uni…" prop:value=move || rel.get() on:input=move |e| rel.set(input_value(&e)) /></label>
            </div>
        </div>
        <span class="label muted">"Birthday (optional)"</span>
        <div class="months">
            {dates::MONTHS_LONG.iter().enumerate().map(|(i, m)| {
                let mi = i as u32 + 1;
                view! { <button class="chip" class:on=move || month.get() == Some(mi)
                    on:click=move |_| month.update(|x| *x = if *x == Some(mi) { None } else { Some(mi) })>{&m[..3]}</button> }
            }).collect_view()}
        </div>
        <div class="row gap">
            <label class="field"><span class="label muted">"Day"</span>
                <input inputmode="numeric" placeholder="26" prop:value=move || day.get() on:input=move |e| day.set(input_value(&e)) /></label>
            <label class="field"><span class="label muted">"Year (optional)"</span>
                <input inputmode="numeric" placeholder="1994" prop:value=move || year.get() on:input=move |e| year.set(input_value(&e)) /></label>
        </div>
        <div class="row gap">
            <span class="grow"></span>
            <button class="button" on:click=move |_| a.close_overlay()>"Cancel"</button>
            <button class="button ink" prop:disabled=move || name.get().trim().is_empty() on:click=save>{if editing { "Save" } else { "Add" }}</button>
        </div>
    }
}

// --- Adding or editing a category ----------------------------------------------------------------------

#[component]
fn CategoryEditor(person: String, uid: Option<String>) -> impl IntoView {
    let a = app();
    let existing = uid.as_ref().and_then(|u| a.peek(|d| d.collection(u).cloned()));
    let name = RwSignal::new(existing.as_ref().map(|c| c.name.clone()).unwrap_or_default());
    let occ = RwSignal::new(existing.as_ref().map(|c| c.occasion.clone()).unwrap_or_else(|| occasion::NONE.into()));
    let date = RwSignal::new(existing.as_ref().and_then(|c| c.occasion_day).and_then(dates::from_epoch_day).map(dates::iso).unwrap_or_default());
    let yearly = RwSignal::new(existing.as_ref().map(|c| c.yearly).unwrap_or(false));
    let editing = existing.is_some();
    let save = move |_| {
        let n = name.get_untracked();
        if n.trim().is_empty() { return; }
        let o = occ.get_untracked();
        let day = dates::parse_iso(&date.get_untracked()).map(dates::epoch_day);
        let (o, day) = if o == occasion::DATE && day.is_none() { (occasion::NONE.to_string(), None) } else { (o, day) };
        match &existing {
            Some(c) => a.change(|d| d.update_collection(Collection { name: n, occasion: o.clone(), occasion_day: if o == occasion::DATE { day } else { None }, yearly: yearly.get_untracked(), ..c.clone() })),
            None => { a.change(|d| d.add_collection(&person, &n, &o, if o == occasion::DATE { day } else { None }, yearly.get_untracked())); }
        }
        a.close_overlay();
    };
    let choice = move |value: &'static str, label: &'static str| view! {
        <button class="seg" class:on=move || occ.get() == value on:click=move |_| occ.set(value.into())>{label}</button>
    };
    view! {
        {title(if editing { "Edit category" } else { "New category" })}
        <label class="field"><span class="label muted">"Name"</span>
            <input placeholder="Trip to Lisbon" prop:value=move || name.get() on:input=move |e| name.set(input_value(&e)) /></label>
        <span class="label muted">"Building up to"</span>
        <div class="segments">
            {choice(occasion::NONE, "Nothing")}
            {choice(occasion::BIRTHDAY, "Their birthday")}
            {choice(occasion::DATE, "A date")}
        </div>
        <Show when=move || occ.get() == occasion::DATE>
            <div class="row gap">
                <input type="date" prop:value=move || date.get() on:input=move |e| date.set(input_value(&e)) />
                <label class="check"><input type="checkbox" prop:checked=move || yearly.get() on:change=move |e| {
                    yearly.set(e.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.checked()).unwrap_or(false))
                } />"Every year"</label>
            </div>
        </Show>
        <div class="row gap">
            <span class="grow"></span>
            <button class="button" on:click=move |_| a.close_overlay()>"Cancel"</button>
            <button class="button ink" on:click=save>{if editing { "Save" } else { "Add" }}</button>
        </div>
    }
}

// --- Confirmations -------------------------------------------------------------------------------------

#[component]
fn ConfirmSheet(what: Confirm) -> impl IntoView {
    let a = app();
    let (head, body, button) = match &what {
        Confirm::RemovePerson(uid) => {
            let name = a.peek(|d| d.person(uid).map(|p| p.first_name().to_string())).unwrap_or_default();
            (format!("Remove {name}?"), format!("{name} and everything noted about them will be removed from Memi on all your devices."), "Remove")
        }
        Confirm::RemoveCategory(uid) => {
            let name = a.peek(|d| d.collection(uid).map(|c| c.name.clone())).unwrap_or_default();
            (format!("Remove {name}?"), "Anything in it moves to their Notes, so nothing is lost.".into(), "Remove")
        }
        Confirm::StartSyncOver => (
            "Start sync over?".into(),
            "Without its recovery key, nobody can open the copy in your Google account, so Memi will delete it and make a new one from what's in this browser. You'll get a new recovery key. Your other devices will ask for it before they sync again.".into(),
            "Start over",
        ),
    };
    let go = move |_| {
        match &what {
            Confirm::RemovePerson(uid) => { a.with_memi(|m| m.remove_person(uid)); a.selected.set(None); a.page_open.set(false); }
            Confirm::RemoveCategory(uid) => a.change(|d| d.remove_collection(uid)),
            Confirm::StartSyncOver => spawn_local(sync::start_over(a)),
        }
        a.close_overlay();
    };
    view! {
        {title(head)}
        <p>{body}</p>
        <div class="row gap"><span class="grow"></span>
            <button class="button" on:click=move |_| a.close_overlay()>"Cancel"</button>
            <button class="button danger-fill" on:click=go>{button}</button>
        </div>
    }
}

// --- Sync ----------------------------------------------------------------------------------------------

#[component]
fn SyncSheet() -> impl IntoView {
    let a = app();
    // Google's sign-in script loads now, so the button below opens its window straight away.
    spawn_local(async { js::load_gis().await; });
    let key_text = RwSignal::new(String::new());
    let wrong = RwSignal::new(false);
    let shown_key = RwSignal::new(None::<String>);
    let since = |ms: i64| {
        let mins = (memi_engine::core::model::now_millis() - ms) / 60_000;
        match mins { m if m < 1 => "just now".to_string(), m if m < 60 => format!("{m} min ago"), m if m < 1440 => format!("{} h ago", m / 60), m => format!("{} d ago", m / 1440) }
    };
    let content = move || {
        let s = a.sync_settings();
        let ui = a.status();
        if sync::CLIENT_ID.is_none() {
            return view! { <p>"Sync isn't set up in this build of Memi."</p> }.into_any();
        }
        if ui.problem == Some(Problem::NeedsKey) {
            return view! {
                <p>"Memi is already syncing on another device. To join it, this browser needs its key."</p>
                {ui.asking.clone().map(|ask| view! {
                    <div class="card col gap">
                        <b>"On any device that's already syncing"</b>
                        <span class="caption muted">"Open Memi there and choose Allow when it asks about this browser. It shows this code:"</span>
                        <span class="code">{ask.confirm.clone()}</span>
                        <span class="caption muted">"Or, on your phone: Theme → Sync → Add a computer, then scan this."</span>
                        <div class="qr" inner_html=qr_svg(&ask.code)></div>
                    </div>
                })}
                <p class="caption muted">"Or type the recovery key from Theme → Sync → Show recovery key on that device."</p>
                <input placeholder="xxxxxxxx-xxxxxxxx-…" autocomplete="off" spellcheck="false" prop:value=move || key_text.get()
                    on:input=move |e| { key_text.set(input_value(&e)); wrong.set(false); } />
                <Show when=move || wrong.get()><span class="caption danger">"That key doesn't open this backup."</span></Show>
                <div class="row gap">
                    <button class="button" on:click=move |_| spawn_local(sync::turn_off(a))>"Cancel"</button>
                    <button class="button ink grow" on:click=move |_| { let t = key_text.get_untracked(); spawn_local(async move { if !sync::use_recovery_key(a, &t).await { wrong.set(true); } }); }>"Unlock backup"</button>
                </div>
                <button class="link danger" on:click=move |_| a.overlay.set(Some(Overlay::Confirm(Confirm::StartSyncOver)))>"Lost your recovery key? Start over"</button>
            }.into_any();
        }
        if !s.enabled {
            return view! {
                <p>"Keep this Memi in step with your phone and computer through your own Google account."</p>
                <p class="muted">"It's encrypted in this browser before it leaves, so Google can't read it, and it sits in a hidden folder only Memi can open. Nobody else, including us, ever gets a copy."</p>
                <button class="button ink" prop:disabled=ui.busy on:click=move |_| spawn_local(sync::connect(a, true))>
                    {if ui.busy { "Setting up…" } else { "Choose a Google account" }}</button>
                {match ui.problem { Some(Problem::Failed(m)) => Some(view! { <span class="caption danger">{m}</span> }), _ => None }}
            }.into_any();
        }
        let detail = if ui.busy { "Syncing…".to_string() } else { s.last_sync.map(|t| format!("Up to date · synced {}", since(t))).unwrap_or_else(|| "Waiting to sync".into()) };
        view! {
            <div class="card col">
                <b>{s.account.clone().map(|a| format!("Syncing with {a}")).unwrap_or_else(|| "Sync is on".into())}</b>
                <span class="label muted">{detail}</span>
            </div>
            {match ui.problem.clone() {
                Some(Problem::NeedsSignIn) => view! {
                    <p>"Google needs you to sign in again before Memi can sync. In a browser that's every hour or so."</p>
                    <button class="button ink" on:click=move |_| spawn_local(sync::connect(a, false))>"Reconnect"</button>
                }.into_any(),
                Some(Problem::Failed(m)) => view! { <p class="muted">{format!("The last sync didn't work ({m}). Memi will try again by itself.")}</p> }.into_any(),
                _ => ().into_any(),
            }}
            <div class="row gap">
                <button class="button grow" on:click=move |_| spawn_local(sync::sync_now(a))>"Sync now"</button>
                <button class="button grow danger" on:click=move |_| spawn_local(sync::turn_off(a))>"Turn off"</button>
            </div>
            {move || match shown_key.get() {
                None => view! { <button class="button" on:click=move |_| spawn_local(async move { shown_key.set(Some(sync::recovery_key(a).await.unwrap_or_else(|| "No key in this browser".into()))) })>"Show recovery key"</button> }.into_any(),
                Some(k) => { let k2 = k.clone(); view! {
                    <div class="key">{k}</div>
                    <button class="button" on:click=move |_| js::copy_text(&k2)>"Copy"</button>
                }.into_any() }
            }}
            <p class="caption muted">"Keep the recovery key somewhere safe. With it, any device can open your Memi."</p>
        }.into_any()
    };
    view! {
        {title("Sync")}
        {content}
        <a class="link" href="https://yourmemi.app/privacy/" target="_blank" rel="noopener">"How sync handles your data"</a>
    }
}

/// The pairing code as a QR code, for a phone to scan.
fn qr_svg(code: &str) -> String {
    qrcode::QrCode::new(code.as_bytes())
        .map(|q| q.render::<qrcode::render::svg::Color>().min_dimensions(200, 200).quiet_zone(true).build())
        .unwrap_or_default()
}
