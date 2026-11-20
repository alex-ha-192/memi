//! The Memi object: the data plus everything that touches the outside world on this device (saving,
//! photos and the sync key), through the [Platform] each app provides. Every screen on every
//! platform makes its changes through here, so they're stamped, saved and queued for sync the same
//! way everywhere.

use crate::data::Data;
use anyhow::Result;
use memi_core::model::*;
use memi_core::{backup, crypto, merge, pairing};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// What each app provides: where things are kept on this device.
pub trait Platform {
    /// Saves the whole Memi. Called after every change; the store writes only what differs.
    fn save(&mut self, snap: &Snapshot) -> Result<()>;
    /// Saves the sync settings (small; written often).
    fn save_sync(&mut self, sync: &SyncSettings);

    /// A photo's bytes, by the key [Platform::save_photo] returned (a file path on the native apps).
    fn photo(&self, key: &str) -> Option<Vec<u8>>;
    /// Keeps a photo (JPEG bytes, already cropped) and returns its key.
    fn save_photo(&mut self, bytes: &[u8]) -> Option<String>;
    fn delete_photo(&mut self, key: &str);

    /// The sync key on this device.
    fn key(&self) -> Option<[u8; 32]>;
    fn save_key(&mut self, key: &[u8; 32]);
    fn forget_key(&mut self);

    /// What this device is called when it asks your other devices for the key ("Pixel 8").
    fn device_label(&self) -> String;
}

/// Sync settings kept on the device, the same on every platform.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// Google wants you to sign in again.
    NeedsSignIn,
    /// There's a backup in the account but this device doesn't have its key.
    NeedsKey,
    Failed(String),
}

/// Another device asking for the key.
#[derive(Clone, Debug, PartialEq)]
pub struct Join {
    pub file_id: String,
    pub device: String,
    pub confirm: String,
}

/// While this device asks your other devices for the key: the code to show as a QR code, and the
/// six digits to compare.
#[derive(Clone, Debug, PartialEq)]
pub struct Asking {
    pub id: String,
    pub code: String,
    pub confirm: String,
}

/// How sync is doing, for the screens. Not saved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncStatus {
    pub busy: bool,
    pub problem: Option<Problem>,
    pub joins: Vec<Join>,
    pub asking: Option<Asking>,
}

/// A photo change when saving a person.
#[derive(Clone, Debug, PartialEq)]
pub enum PhotoChange {
    Keep,
    Remove,
    /// New JPEG bytes, already cropped.
    Set(Vec<u8>),
}

pub struct Memi<P: Platform> {
    data: Data,
    pub sync: SyncSettings,
    pub status: SyncStatus,
    /// Goes up with every change, so screens know when to refresh.
    pub version: u64,
    /// The last thing that went wrong saving on this device, if anything.
    pub save_error: Option<String>,
    pub(crate) offer: Option<Arc<pairing::Offer>>,
    platform: P,
}

impl<P: Platform> Memi<P> {
    pub fn new(snapshot: Snapshot, sync: SyncSettings, platform: P) -> Self {
        Memi { data: Data::new(snapshot), sync, status: SyncStatus::default(), version: 0, save_error: None, offer: None, platform }
    }

    pub fn data(&self) -> &Data { &self.data }
    pub fn snapshot(&self) -> &Snapshot { &self.data.snap }
    pub fn platform(&mut self) -> &mut P { &mut self.platform }
    pub fn platform_ref(&self) -> &P { &self.platform }

    /// Makes a change, saves it, and queues it for sync. Every edit goes through here.
    pub fn change<R>(&mut self, f: impl FnOnce(&mut Data) -> R) -> R {
        let r = f(&mut self.data);
        self.saved();
        if self.sync.enabled && !self.sync.dirty { self.update_sync(|s| s.dirty = true); }
        r
    }

    fn saved(&mut self) {
        self.version += 1;
        self.save_error = self.platform.save(&self.data.snap).err().map(|e| format!("Couldn't save on this device: {e}"));
    }

    pub fn update_sync(&mut self, f: impl FnOnce(&mut SyncSettings)) {
        f(&mut self.sync);
        self.platform.save_sync(&self.sync);
        self.version += 1;
    }

    pub fn set_status(&mut self, f: impl FnOnce(&mut SyncStatus)) {
        f(&mut self.status);
        self.version += 1;
    }

    // --- People, with their photos ---------------------------------------------------------------------

    pub fn add_person(&mut self, mut p: Person, photo: Option<Vec<u8>>) -> String {
        p.photo_path = photo.and_then(|b| self.platform.save_photo(&b));
        self.change(|d| d.add_person(p))
    }

    pub fn update_person(&mut self, mut p: Person, photo: PhotoChange) {
        let old = self.data.person(&p.uid).and_then(|x| x.photo_path.clone());
        p.photo_path = match photo {
            PhotoChange::Keep => old.clone(),
            PhotoChange::Remove => None,
            PhotoChange::Set(b) => self.platform.save_photo(&b),
        };
        if let Some(o) = old.filter(|o| Some(o) != p.photo_path.as_ref()) { self.platform.delete_photo(&o); }
        self.change(|d| d.update_person(p));
    }

    pub fn remove_person(&mut self, uid: &str) {
        if let Some(path) = self.data.person(uid).and_then(|p| p.photo_path.clone()) { self.platform.delete_photo(&path); }
        self.change(|d| d.remove_person(uid));
    }

    pub fn photo(&self, p: &Person) -> Option<Vec<u8>> { p.photo_path.as_deref().and_then(|k| self.platform.photo(k)) }

    // --- Sync's view of the data ----------------------------------------------------------------------

    pub fn key(&self) -> Option<[u8; 32]> { self.platform.key() }
    pub fn recovery_key(&self) -> Option<String> { self.key().map(|k| crypto::key_to_text(&k)) }
    pub(crate) fn save_key(&mut self, key: &[u8; 32]) { self.platform.save_key(key) }
    pub fn device_label(&self) -> String { self.platform.device_label() }

    /// Everything, photos included, encrypted for Drive.
    pub fn sealed(&self, key: &[u8; 32]) -> Result<Vec<u8>> {
        let json = backup::export(&self.data.snap, |p| self.photo(p));
        crypto::seal(key, &json)
    }

    /// Merges a copy from Drive into this one. Returns whether Drive's copy needs updating.
    pub fn merge_in(&mut self, doc: backup::Doc) -> bool {
        let m = merge::merge(&self.data.snap, &doc.snapshot, now_millis());
        if m.local_changed {
            let mut merged = m.snapshot;
            let old: std::collections::HashMap<String, Option<String>> =
                self.data.snap.people.iter().map(|p| (p.uid.clone(), p.photo_path.clone())).collect();
            for p in merged.people.iter_mut() {
                let mine = old.get(&p.uid).cloned().flatten();
                if m.people_from_remote.contains(&p.uid) {
                    // Their photo comes from Drive too.
                    p.photo_path = doc.photos.get(&p.uid).and_then(|b| self.platform.save_photo(b));
                    if let Some(o) = mine { self.platform.delete_photo(&o); }
                } else {
                    p.photo_path = mine;
                }
            }
            // People who are gone: their photos go too.
            for (uid, path) in &old {
                if let Some(path) = path { if !merged.people.iter().any(|p| &p.uid == uid) { self.platform.delete_photo(path); } }
            }
            self.data.replace(merged);
            self.saved();
        }
        m.remote_changed || doc.legacy
    }

    pub fn turn_off_sync(&mut self, forget_key: bool) {
        self.offer = None;
        if forget_key { self.platform.forget_key(); }
        self.update_sync(|s| *s = SyncSettings::default());
        self.set_status(|s| *s = SyncStatus::default());
    }
}
