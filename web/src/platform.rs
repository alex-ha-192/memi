//! The browser's side of the engine: where things are kept (IndexedDB), and a reactive handle so
//! the screens update when the Memi changes.
//!
//! IndexedDB holds the same backup document as before ("doc", photos included, unencrypted, since
//! it never leaves the browser), the sync settings ("sync") and the key ("key"). Writes happen in
//! the background; the engine keeps everything in memory.

use crate::js;
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_engine::core::backup;
use memi_engine::core::model::Snapshot;
use memi_engine::{Memi, Platform, SyncSettings};
use std::collections::HashMap;

#[derive(Default)]
pub struct WebPlatform {
    /// Photo bytes by key ("p1", "p2"…). Keys only mean something in this tab.
    photos: HashMap<String, Vec<u8>>,
    next_photo: u64,
    key: Option<[u8; 32]>,
}

impl WebPlatform {
    fn new_photo_key(&mut self) -> String {
        self.next_photo += 1;
        format!("p{}", self.next_photo)
    }
}

impl WebPlatform {
    pub fn photo_bytes(&self, key: &str) -> Option<Vec<u8>> { self.photos.get(key).cloned() }
}

impl Platform for WebPlatform {
    fn save(&mut self, snap: &Snapshot) -> anyhow::Result<()> {
        let bytes = backup::export(snap, |p| p.photo_path.as_ref().and_then(|k| self.photos.get(k)).cloned());
        spawn_local(async move {
            if let Err(e) = js::idb_set_bytes("doc", &bytes).await {
                web_sys::console::error_1(&format!("Couldn't save in this browser: {e}").into());
            }
        });
        Ok(())
    }

    fn save_sync(&mut self, sync: &SyncSettings) {
        let json = serde_json::to_string(sync).unwrap_or_default();
        spawn_local(async move { let _ = js::idb_set_text("sync", &json).await; });
    }

    fn photo(&self, key: &str) -> Option<Vec<u8>> { self.photos.get(key).cloned() }

    fn save_photo(&mut self, bytes: &[u8]) -> Option<String> {
        let k = self.new_photo_key();
        self.photos.insert(k.clone(), bytes.to_vec());
        Some(k)
    }

    fn delete_photo(&mut self, key: &str) { self.photos.remove(key); }

    fn key(&self) -> Option<[u8; 32]> { self.key }

    fn save_key(&mut self, key: &[u8; 32]) {
        self.key = Some(*key);
        let k = *key;
        spawn_local(async move { let _ = js::idb_set_bytes("key", &k).await; });
    }

    fn forget_key(&mut self) {
        self.key = None;
        spawn_local(async { js::idb_delete("key").await; });
    }

    fn device_label(&self) -> String { js::device_label() }
}

/// Loads what this browser has kept and starts the engine with it.
pub async fn load() -> Memi<WebPlatform> {
    let mut platform = WebPlatform::default();
    platform.key = js::idb_get_bytes("key").await.and_then(|b| b.try_into().ok());
    let mut snapshot = Snapshot::default();
    if let Some(bytes) = js::idb_get_bytes("doc").await {
        if let Ok(doc) = backup::import(&bytes) {
            snapshot = doc.snapshot;
            for p in snapshot.people.iter_mut() {
                p.photo_path = doc.photos.get(&p.uid).map(|b| {
                    let k = platform.new_photo_key();
                    platform.photos.insert(k.clone(), b.clone());
                    k
                });
            }
        }
    }
    let sync = match js::idb_get_text("sync").await {
        Some(t) => serde_json::from_str(&t).unwrap_or_default(),
        // Before 0.6.1 the sync settings were part of the app's settings.
        None => js::idb_get_text("settings").await
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| serde_json::from_value::<SyncSettings>(v.get("sync")?.clone()).ok())
            .unwrap_or_default(),
    };
    Memi::new(snapshot, sync, platform)
}

/// The Memi, shared with the screens: reading it in a view subscribes to changes; the engine's
/// writes notify only when something actually changed.
#[derive(Clone, Copy)]
pub struct WebHandle(pub RwSignal<Memi<WebPlatform>, LocalStorage>);

impl memi_engine::sync::Handle for WebHandle {
    type P = WebPlatform;
    fn with<R>(&self, f: impl FnOnce(&mut Memi<WebPlatform>) -> R) -> R {
        let before = self.0.with_untracked(|m| m.version);
        let r = self.0.try_update_untracked(f).expect("the Memi is always there");
        if self.0.with_untracked(|m| m.version) != before { self.0.notify(); }
        r
    }
}
