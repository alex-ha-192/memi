//! The app's state, shared through Leptos context: the Memi (memi-engine), this browser's
//! settings, what's open and the toast. Every change goes through the engine, which saves it and
//! queues it for sync.

use crate::js;
use crate::platform::{WebHandle, WebPlatform};
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_engine::core::model::Note;
use memi_engine::sync::Handle;
use memi_engine::{Data, Memi, SyncSettings, SyncStatus};
use serde::{Deserialize, Serialize};

/// This browser's own settings (sync's are kept by the engine).
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub scheme: String,
    /// "system", "light" or "dark".
    pub appearance: String,
    pub fresh_collapsed: bool,
    /// The first-launch questions are answered (or were skipped: this browser already had people or sync).
    pub welcomed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { scheme: "Bento".into(), appearance: "system".into(), fresh_collapsed: false, welcomed: false }
    }
}

/// The first-launch questions (see ui/welcome.rs).
#[derive(Clone, Copy, PartialEq)]
pub enum Welcome { Question, Choice }

#[derive(Clone, PartialEq)]
pub enum Overlay {
    Category { person: String, collection: String },
    EditNote(String),
    EditPerson(Option<String>),
    EditCategory { person: String, collection: Option<String> },
    Sync,
    Confirm(Confirm),
}

#[derive(Clone, PartialEq)]
pub enum Confirm {
    RemovePerson(String),
    RemoveCategory(String),
    StartSyncOver,
}

#[derive(Clone)]
pub enum Undo {
    RestoreNote(Note),
    RemoveNote(String),
}

#[derive(Clone)]
pub struct Toast {
    pub seq: u64,
    pub message: String,
    pub undo: Option<Undo>,
}

#[derive(Clone, Copy)]
pub struct App {
    pub memi: RwSignal<Memi<WebPlatform>, LocalStorage>,
    pub settings: RwSignal<Settings>,
    pub loaded: RwSignal<bool>,
    /// The person shown, by uid.
    pub selected: RwSignal<Option<String>>,
    /// On a narrow screen: the person's page is showing instead of the list.
    pub page_open: RwSignal<bool>,
    pub overlay: RwSignal<Option<Overlay>>,
    pub theme_open: RwSignal<bool>,
    pub search: RwSignal<Option<String>>,
    pub toast: RwSignal<Option<Toast>>,
    pub system_dark: RwSignal<bool>,
    pub width: RwSignal<f64>,
    /// Photo URLs by photo key (blob: URLs made from the bytes).
    pub photo_urls: RwSignal<std::collections::HashMap<String, String>>,
    pub push_seq: RwSignal<u64>,
    /// The ⋯ menu that's open, if any.
    pub menu: RwSignal<Option<Menu>>,
    /// While dragging to reorder: the uid being dragged and the one it's over.
    pub drag: RwSignal<Option<(String, Option<String>)>>,
    /// The first-launch questions, while they're showing.
    pub welcome: RwSignal<Option<Welcome>>,
}

#[derive(Clone, PartialEq)]
pub struct Menu {
    pub x: f64,
    pub y: f64,
    pub kind: MenuKind,
}

#[derive(Clone, PartialEq)]
pub enum MenuKind {
    Person(String),
    Category(String),
    Note(String),
}

pub fn app() -> App { expect_context::<App>() }

impl App {
    pub fn new() -> Self {
        App {
            memi: RwSignal::new_local(Memi::new(Default::default(), SyncSettings::default(), WebPlatform::default())),
            settings: RwSignal::new(Settings::default()),
            loaded: RwSignal::new(false),
            selected: RwSignal::new(None),
            page_open: RwSignal::new(false),
            overlay: RwSignal::new(None),
            theme_open: RwSignal::new(false),
            search: RwSignal::new(None),
            toast: RwSignal::new(None),
            system_dark: RwSignal::new(js::system_dark()),
            width: RwSignal::new(window_width()),
            photo_urls: RwSignal::new(Default::default()),
            push_seq: RwSignal::new(0),
            menu: RwSignal::new(None),
            drag: RwSignal::new(None),
            welcome: RwSignal::new(None),
        }
    }

    /// Loads what's saved in this browser.
    pub async fn load(self) {
        js::persist_storage();
        if let Some(text) = js::idb_get_text("settings").await {
            if let Ok(s) = serde_json::from_str::<Settings>(&text) { self.settings.set(s); }
        }
        self.memi.set(crate::platform::load().await);
        // Only a browser with nothing in it and sync off is asked; every earlier install is marked as answered.
        if !self.settings.with_untracked(|s| s.welcomed) {
            let fresh = self.memi.with_untracked(|m| m.snapshot().people.is_empty() && !m.sync.enabled);
            if fresh { self.welcome.set(Some(Welcome::Question)); } else { self.update_settings(|s| s.welcomed = true); }
        }
        self.loaded.set(true);
    }

    pub fn handle(self) -> WebHandle { WebHandle(self.memi) }

    /// Reads the data, subscribing the view to changes.
    pub fn read<R>(self, f: impl FnOnce(&Data) -> R) -> R { self.memi.with(|m| f(m.data())) }
    /// Reads the data without subscribing.
    pub fn peek<R>(self, f: impl FnOnce(&Data) -> R) -> R { self.memi.with_untracked(|m| f(m.data())) }

    /// Makes a change through the engine (saved, and queued for sync). Returns what `f` returns.
    pub fn change<R>(self, f: impl FnOnce(&mut Data) -> R) -> R {
        let r = self.handle().with(|m| m.change(f));
        crate::sync::changed(self);
        r
    }

    /// For changes that involve photos (adding, editing or removing a person).
    pub fn with_memi<R>(self, f: impl FnOnce(&mut Memi<WebPlatform>) -> R) -> R {
        let r = self.handle().with(f);
        crate::sync::changed(self);
        r
    }

    pub fn status(self) -> SyncStatus { self.memi.with(|m| m.status.clone()) }
    pub fn sync_settings(self) -> SyncSettings { self.memi.with(|m| m.sync.clone()) }

    pub fn update_settings(self, f: impl FnOnce(&mut Settings)) {
        self.settings.update(f);
        let json = self.settings.with_untracked(|s| serde_json::to_string(s).unwrap_or_default());
        spawn_local(async move { let _ = js::idb_set_text("settings", &json).await; });
    }

    pub fn show_toast(self, message: impl Into<String>, undo: Option<Undo>) {
        let seq = self.toast.with_untracked(|t| t.as_ref().map(|t| t.seq).unwrap_or(0)) + 1;
        self.toast.set(Some(Toast { seq, message: message.into(), undo }));
        spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(5_000).await;
            if self.toast.with_untracked(|t| t.as_ref().map(|t| t.seq) == Some(seq)) { self.toast.set(None); }
        });
    }

    pub fn undo(self) {
        let Some(t) = self.toast.get_untracked() else { return };
        self.toast.set(None);
        match t.undo {
            Some(Undo::RestoreNote(n)) => self.change(|d| d.restore_note(n)),
            Some(Undo::RemoveNote(uid)) => self.change(|d| d.remove_note(&uid)),
            None => {}
        }
    }

    pub fn dark(self) -> bool {
        match self.settings.with(|s| s.appearance.clone()).as_str() {
            "light" => false,
            "dark" => true,
            _ => self.system_dark.get(),
        }
    }

    /// A URL for someone's photo, or None to show their placeholder.
    pub fn photo_url(self, uid: &str) -> Option<String> {
        let key = self.read(|d| d.person(uid).and_then(|p| p.photo_path.clone()))?;
        if let Some(url) = self.photo_urls.with_untracked(|m| m.get(&key).cloned()) { return Some(url); }
        let url = self.memi.with_untracked(|m| m.platform_ref().photo_bytes(&key)).and_then(|b| js::blob_url(&b))?;
        self.photo_urls.update_untracked(|m| { m.insert(key, url.clone()); });
        Some(url)
    }

    pub fn open_person(self, uid: String) {
        self.selected.set(Some(uid));
        self.page_open.set(true);
        self.search.set(None);
        scroll_top();
    }

    pub fn close_overlay(self) { self.overlay.set(None); }
}

pub fn window_width() -> f64 {
    web_sys::window().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).unwrap_or(1200.)
}

pub fn scroll_top() {
    if let Some(w) = web_sys::window() { w.scroll_to_with_x_and_y(0., 0.); }
}
