//! The page: the list (Home) on the left and the person on the right, or one at a time on a narrow
//! screen, plus the theme panel, menus, sheets and the toast.

mod home;
mod person;
mod sheets;
pub mod theme;
mod widgets;

use crate::state::{app, App, Confirm, MenuKind, Overlay};
use crate::{js, sync};
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_core::model::{outcome, status};
use wasm_bindgen::prelude::*;
use widgets::Icon;

/// Below this width, one pane at a time.
pub const TWO_PANE_MIN: f64 = 820.;

#[component]
pub fn Root() -> impl IntoView {
    let a = App::new();
    provide_context(a);
    spawn_local(async move {
        a.load().await;
        sync::start(a);
        // Pick up anything new from your other devices straight away (needs a token, so only if
        // this tab already has one; otherwise Sync shows Reconnect).
    });

    // Theme: CSS variables on <html>.
    Effect::new(move |_| {
        let css = theme::css_vars(&a.settings.with(|s| s.scheme.clone()), a.dark());
        if let Some(el) = document().document_element() { let _ = el.set_attribute("style", &css); }
    });
    let dark_cb = Closure::<dyn FnMut(bool)>::new(move |d| a.system_dark.set(d));
    js::on_system_dark_change(&dark_cb);
    dark_cb.forget();
    let vis_cb = Closure::<dyn FnMut(bool)>::new(move |visible| {
        if visible && a.settings.with_untracked(|s| s.sync.enabled) { spawn_local(sync::sync_now(a)); }
    });
    js::on_visibility_change(&vis_cb);
    vis_cb.forget();
    let _ = window_event_listener(leptos::ev::resize, move |_| a.width.set(crate::state::window_width()));
    let _ = window_event_listener(leptos::ev::keydown, move |e| {
        if e.key() == "Escape" {
            if a.menu.get_untracked().is_some() { a.menu.set(None); }
            else if a.overlay.get_untracked().is_some() { a.overlay.set(None); }
            else if a.theme_open.get_untracked() { a.theme_open.set(false); }
            else if a.search.get_untracked().is_some() { a.search.set(None); }
            else if a.page_open.get_untracked() && a.width.get_untracked() < TWO_PANE_MIN { a.page_open.set(false); }
        }
    });

    let narrow = move || a.width.get() < TWO_PANE_MIN;
    view! {
        <Show when=move || a.loaded.get() fallback=|| view! { <div class="splash"><img src="/memi.png" alt="" /></div> }>
            <div class="app" class:narrow=narrow>
                <aside class="list" class:hidden=move || narrow() && a.page_open.get()>
                    <home::Home />
                </aside>
                <main class="page" class:hidden=move || narrow() && !a.page_open.get()>
                    <person::PersonPage />
                </main>
                <ThemePanel />
                <MenuView />
                <sheets::OverlayHost />
                <ToastView />
            </div>
        </Show>
    }
}

#[component]
fn ThemePanel() -> impl IntoView {
    let a = app();
    let mode = move |id: &'static str, label: &'static str, icon: &'static str| view! {
        <button class="seg" class:on=move || a.settings.with(|s| s.appearance == id)
            on:click=move |_| a.update_settings(|s| s.appearance = id.into())>
            <Icon name=icon size=16. />{label}
        </button>
    };
    let sync_detail = move || {
        let (enabled, account) = a.settings.with(|s| (s.sync.enabled, s.sync.account.clone()));
        match (enabled, a.sync.with(|s| s.problem.is_some())) {
            (true, true) => "Needs a look".to_string(),
            (true, false) => account.unwrap_or_else(|| "On".into()),
            _ => "Off".into(),
        }
    };
    view! {
        <Show when=move || a.theme_open.get()>
            <div class="scrim clear" on:click=move |_| a.theme_open.set(false)></div>
            <div class="theme-panel">
                <div class="row baseline">
                    <h2>"Theme"</h2>
                    <span class="caption muted">{move || {
                        let s = a.settings.get();
                        format!("{} · {}", s.scheme, match s.appearance.as_str() { "light" => "light", "dark" => "dark", _ => "following your system" })
                    }}</span>
                </div>
                <div class="schemes">
                    {memi_core::palette::SCHEMES.iter().map(|s| s.name).map(|name| {
                        let p = move || theme::preview(name, a.dark());
                        view! {
                            <button class="scheme" class:on=move || a.settings.with(|s| s.scheme == name)
                                on:click=move |_| a.update_settings(|s| s.scheme = name.into())>
                                <span class="swatch" style=move || format!("background:{}", p().0)>
                                    <span class="big" style=move || format!("background:{}", p().1)></span>
                                    <span class="small"><i style=move || format!("background:{}", p().2)></i><i style=move || format!("background:{}", p().3)></i></span>
                                </span>
                                <span class="caption">{name}</span>
                            </button>
                        }
                    }).collect_view()}
                </div>
                <div class="segments">
                    {mode("light", "Light", "sun")}
                    {mode("dark", "Dark", "moon")}
                    {mode("system", "System", "monitor")}
                </div>
                <button class="card-row" on:click=move |_| { a.theme_open.set(false); a.overlay.set(Some(Overlay::Sync)); }>
                    <Icon name="cloud" size=19. />
                    <b>"Sync"</b>
                    <span class="muted grow ellipsis">{sync_detail}</span>
                    <Icon name="right" size=17. />
                </button>
            </div>
        </Show>
    }
}

#[component]
fn MenuView() -> impl IntoView {
    let a = app();
    move || {
        let Some(m) = a.menu.get() else { return ().into_any() };
        let close = move || a.menu.set(None);
        let w = crate::state::window_width();
        let h = web_sys::window().and_then(|w| w.inner_height().ok()).and_then(|v| v.as_f64()).unwrap_or(800.);
        let x = m.x.min(w - 244.).max(8.);
        // Open upwards when there isn't room below.
        let y = if m.y + 330. > h { (m.y - 330.).max(8.) } else { m.y };
        let item = move |label: &'static str, danger: bool, f: Box<dyn Fn()>| view! {
            <button class="menu-item" class:danger=danger on:click=move |_| { close(); f(); }>{label}</button>
        };
        let items = match m.kind {
            MenuKind::Note(uid) => {
                let Some(n) = a.data.with_untracked(|d| d.note(&uid).cloned()) else { return ().into_any() };
                let others: Vec<(String, String)> = a.data.with_untracked(|d| d.categories_of(&n.person_uid).into_iter()
                    .filter(|c| c.uid != n.collection_uid).map(|c| (c.uid, c.name)).collect());
                let (u1, u2, u3, u4, u5, u6) = (uid.clone(), uid.clone(), uid.clone(), uid.clone(), uid.clone(), uid.clone());
                let n2 = n.clone();
                view! {
                    {if n.status == status::OPEN {
                        view! {
                            {item("✓  A hit", false, Box::new(move || done(&u1, outcome::HIT)))}
                            {item("✕  A miss", false, Box::new(move || done(&u2, outcome::MISS)))}
                            {item("Done", false, Box::new(move || done(&u3, outcome::NONE)))}
                        }.into_any()
                    } else {
                        item("Reopen", false, Box::new(move || a.change(|d| d.reopen(&u1)))).into_any()
                    }}
                    {item("Edit", false, Box::new(move || a.overlay.set(Some(Overlay::EditNote(u4.clone())))))}
                    {others.into_iter().map(|(cu, name)| {
                        let (u, nm) = (u5.clone(), name.clone());
                        view! { <button class="menu-item" on:click=move |_| {
                            close();
                            let before = a.data.with_untracked(|d| d.note(&u).cloned());
                            a.change(|d| d.move_note(&u, &cu));
                            a.show_toast(format!("Moved to {nm}"), before.map(crate::state::Undo::RestoreNote));
                        }>{format!("Move to {name}")}</button> }
                    }).collect_view()}
                    {item("Delete", true, Box::new(move || {
                        a.change(|d| d.remove_note(&u6));
                        a.show_toast("Deleted", Some(crate::state::Undo::RestoreNote(n2.clone())));
                    }))}
                }.into_any()
            }
            MenuKind::Person(uid) => {
                let Some(p) = a.data.with_untracked(|d| d.person(&uid).cloned()) else { return ().into_any() };
                let (u1, u2, u3) = (uid.clone(), uid.clone(), uid.clone());
                let pinned = p.pinned;
                view! {
                    {item("Edit", false, Box::new(move || a.overlay.set(Some(Overlay::EditPerson(Some(u1.clone()))))))}
                    {item(if pinned { "Unpin" } else { "Pin to top" }, false, Box::new(move || a.change(|d| d.set_pinned(&u2, !pinned))))}
                    {item("Remove", true, Box::new(move || a.overlay.set(Some(Overlay::Confirm(Confirm::RemovePerson(u3.clone()))))))}
                }.into_any()
            }
            MenuKind::Category(uid) => {
                let Some(c) = a.data.with_untracked(|d| d.collection(&uid).cloned()) else { return ().into_any() };
                let person = c.person_uid.clone().unwrap_or_default();
                let (u1, u2) = (uid.clone(), uid.clone());
                view! {
                    {item("Edit", false, Box::new(move || a.overlay.set(Some(Overlay::EditCategory { person: person.clone(), collection: Some(u1.clone()) }))))}
                    {item("Remove category", true, Box::new(move || a.overlay.set(Some(Overlay::Confirm(Confirm::RemoveCategory(u2.clone()))))))}
                }.into_any()
            }
        };
        view! {
            <div class="scrim clear" on:click=move |_| a.menu.set(None) on:contextmenu=move |e| { e.prevent_default(); a.menu.set(None); }></div>
            <div class="menu" style=format!("left:{x}px;top:{y}px")>{items}</div>
        }.into_any()
    }
}

fn done(uid: &str, result: i32) {
    let a = app();
    let before = a.data.with_untracked(|d| d.note(uid).cloned());
    a.change(|d| d.mark_done(uid, result));
    let msg = match result { outcome::HIT => "Marked as a hit", outcome::MISS => "Marked as a miss", _ => "Marked done" };
    a.show_toast(msg, before.map(crate::state::Undo::RestoreNote));
}

#[component]
fn ToastView() -> impl IntoView {
    let a = app();
    move || a.toast.get().map(|t| view! {
        <div class="toast-wrap">
            <div class="toast">
                <span>{t.message.clone()}</span>
                {t.undo.is_some().then(|| view! { <button on:click=move |_| a.undo()>"Undo"</button> })}
            </div>
        </div>
    })
}
