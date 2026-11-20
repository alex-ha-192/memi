//! The app's state, shared through Leptos context: the data, settings, what's open, sync status
//! and the toast. Every change to the data is saved to IndexedDB and queued for sync.

use crate::data::Data;
use crate::js;
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_core::model::Note;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub scheme: String,
    /// "system", "light" or "dark".
    pub appearance: String,
    pub fresh_collapsed: bool,
    pub sync: SyncSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { scheme: "Bento".into(), appearance: "system".into(), fresh_collapsed: false, sync: SyncSettings::default() }
    }
}

#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct SyncSettings {
    pub enabled: bool,
    pub account: Option<String>,
    /// Drive's modifiedTime of the copy last seen, to tell when another device changed it.
    pub remote_version: Option<String>,
    /// Changed here since the last upload.
    pub dirty: bool,
    pub last_sync: Option<i64>,
}

#[derive(Clone, PartialEq)]
pub enum Problem {
    /// Google wants you to sign in again (tokens last an hour in the browser).
    NeedsSignIn,
    /// There's a backup in the account but this browser doesn't have its key.
    NeedsKey,
    Failed(String),
}

#[derive(Clone, PartialEq)]
pub struct Join {
    pub file_id: String,
    pub device: String,
    pub confirm: String,
}

/// While this browser asks another device for the key.
#[derive(Clone, PartialEq)]
pub struct Asking {
    pub id: String,
    pub confirm: String,
    pub qr_svg: String,
}

#[derive(Clone, Default, PartialEq)]
pub struct SyncUi {
    pub busy: bool,
    pub problem: Option<Problem>,
    pub joins: Vec<Join>,
    pub asking: Option<Asking>,
}

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
    pub data: RwSignal<Data>,
    pub settings: RwSignal<Settings>,
    pub loaded: RwSignal<bool>,
    /// The person shown, by uid.
    pub selected: RwSignal<Option<String>>,
    /// On a narrow screen: the person's page is showing instead of the list.
    pub page_open: RwSignal<bool>,
    pub overlay: RwSignal<Option<Overlay>>,
    pub theme_open: RwSignal<bool>,
    pub search: RwSignal<Option<String>>,
    pub sync: RwSignal<SyncUi>,
    pub toast: RwSignal<Option<Toast>>,
    pub system_dark: RwSignal<bool>,
    pub width: RwSignal<f64>,
    /// Photo URLs by person uid (blob: URLs made from the bytes).
    pub photo_urls: RwSignal<std::collections::HashMap<String, (usize, String)>>,
    pub push_seq: RwSignal<u64>,
    /// The ⋯ menu that's open, if any.
    pub menu: RwSignal<Option<Menu>>,
    /// While dragging to reorder: the uid being dragged and the one it's over.
    pub drag: RwSignal<Option<(String, Option<String>)>>,
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
            data: RwSignal::new(Data::default()),
            settings: RwSignal::new(Settings::default()),
            loaded: RwSignal::new(false),
            selected: RwSignal::new(None),
            page_open: RwSignal::new(false),
            overlay: RwSignal::new(None),
            theme_open: RwSignal::new(false),
            search: RwSignal::new(None),
            sync: RwSignal::new(SyncUi::default()),
            toast: RwSignal::new(None),
            system_dark: RwSignal::new(js::system_dark()),
            width: RwSignal::new(window_width()),
            photo_urls: RwSignal::new(Default::default()),
            push_seq: RwSignal::new(0),
            menu: RwSignal::new(None),
            drag: RwSignal::new(None),
        }
    }

    /// Loads what's saved in this browser.
    pub async fn load(self) {
        js::persist_storage();
        if let Some(text) = js::idb_get_text("settings").await {
            if let Ok(s) = serde_json::from_str::<Settings>(&text) { self.settings.set(s); }
        }
        if let Some(bytes) = js::idb_get_bytes("doc").await {
            match memi_core::backup::import(&bytes) {
                Ok(doc) => self.data.set(Data::from_doc(doc)),
                Err(e) => self.show_toast(format!("Couldn't open what's saved here: {e}"), None),
            }
        }
        self.loaded.set(true);
    }

    /// Changes the data, saves it and queues an upload. Returns what `f` returns.
    pub fn change<R>(self, f: impl FnOnce(&mut Data) -> R) -> R {
        let r = self.data.try_update(f).expect("data");
        self.save_data();
        crate::sync::changed(self);
        r
    }

    pub fn save_data(self) {
        let bytes = self.data.with_untracked(|d| d.export());
        spawn_local(async move {
            if let Err(e) = js::idb_set_bytes("doc", &bytes).await {
                self.show_toast(format!("Couldn't save in this browser: {e}"), None);
            }
        });
    }

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
        let len = self.data.with(|d| d.photos.get(uid).map(|b| b.len()))?;
        if let Some((l, url)) = self.photo_urls.with_untracked(|m| m.get(uid).cloned()) {
            if l == len { return Some(url); }
            js::revoke_url(&url);
        }
        let url = self.data.with_untracked(|d| d.photos.get(uid).and_then(|b| js::blob_url(b)))?;
        self.photo_urls.update_untracked(|m| { m.insert(uid.to_string(), (len, url.clone())); });
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
