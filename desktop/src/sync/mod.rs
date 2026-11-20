//! Keeps your Memi in step across your devices through an encrypted copy in your own Google Drive,
//! the same way the Android and web apps do (same file, same format).
//!
//! Each sync downloads the copy if someone else changed it, merges it with what's here item by item
//! (memi_core::merge: the newest change to each thing wins, deletions included), and uploads the
//! result if it differs. The window syncs every few seconds while it's in front, and shortly after
//! every change. All the methods here block (network and disk), so the UI runs them on a background
//! thread.

pub mod auth;
pub mod drive;
pub mod secrets;
pub use memi_core::{crypto, pairing};

use crate::model::*;
use crate::store::Store;
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use drive::{Drive, RemoteFile, Unauthorized};
use memi_core::{backup, merge};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

const FILE: &str = "memi-backup.bin";
const KEY: &str = "sync-key";

#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// Google wants you to sign in again.
    NeedsSignIn,
    /// There's a backup, but this computer doesn't have its key yet (another device, or the recovery key).
    NeedsKey,
    /// Kept for old answers; merging means there's nothing to choose between any more.
    Conflict,
    Failed(String),
}

/// Another device asking for the key (see memi_core::pairing).
#[derive(Clone, Debug, PartialEq)]
pub struct Join {
    pub file_id: String,
    pub device: String,
    pub confirm: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub account: Option<String>,
    pub dirty: bool,
    pub last_sync: Option<i64>,
    pub remote_version: Option<String>,
}

pub struct Engine {
    store: Arc<Mutex<Store>>,
    photo_dir: PathBuf,
    settings_path: PathBuf,
    pub settings: Mutex<Settings>,
    secrets: secrets::Secrets,
    auth: auth::Auth,
    /// Kept between signing in and answering a key question.
    pending_token: Mutex<Option<String>>,
    /// Devices waiting for this one to hand them the key.
    pub joins: Mutex<Vec<Join>>,
}

/// How this computer describes itself to your other devices when it asks for the key.
pub fn device_label() -> &'static str {
    if cfg!(target_os = "windows") { "Windows computer" } else if cfg!(target_os = "macos") { "Mac" } else { "Linux computer" }
}

impl Engine {
    pub fn new(store: Arc<Mutex<Store>>, data_dir: &Path, photo_dir: PathBuf) -> Self {
        let settings_path = data_dir.join("sync.json");
        let settings = std::fs::read(&settings_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Engine {
            store, photo_dir, settings_path, settings: Mutex::new(settings),
            secrets: secrets::Secrets::new(data_dir), auth: auth::Auth::new(), pending_token: Mutex::new(None),
            joins: Mutex::new(vec![]),
        }
    }

    pub fn snapshot(&self) -> Settings { self.settings.lock().unwrap().clone() }

    fn update(&self, f: impl FnOnce(&mut Settings)) {
        let mut s = self.settings.lock().unwrap();
        f(&mut s);
        if let Ok(json) = serde_json::to_vec_pretty(&*s) {
            let tmp = self.settings_path.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() { let _ = std::fs::rename(tmp, &self.settings_path); }
        }
    }

    fn local_key(&self) -> Option<Zeroizing<[u8; 32]>> {
        let b = Zeroizing::new(STANDARD.decode(self.secrets.get(KEY)?).ok()?);
        (b.len() == 32).then(|| { let mut k = Zeroizing::new([0u8; 32]); k.copy_from_slice(&b); k })
    }

    fn save_key(&self, key: &[u8; 32]) {
        self.secrets.set(KEY, &STANDARD.encode(key));
    }

    pub fn recovery_key(&self) -> Option<String> {
        self.local_key().map(|k| crypto::key_to_text(&k))
    }

    pub fn mark_dirty(&self) {
        if self.settings.lock().unwrap().enabled { self.update(|s| s.dirty = true); }
    }

    // --- Turning it on -----------------------------------------------------------------------

    /// Opens the browser to sign in, then connects. Returns a question to ask, if any.
    pub fn sign_in_and_connect(&self) -> Result<Option<Problem>> {
        let token = self.auth.sign_in(&self.secrets)?;
        self.connect(&token)
    }

    fn connect(&self, token: &str) -> Result<Option<Problem>> {
        *self.pending_token.lock().unwrap() = Some(token.to_string());
        let drive = Drive::new(token);
        let email = drive.email();
        self.update(|s| s.account = email);
        let remote = drive.find(FILE)?;
        match (remote, self.local_key()) {
            (None, key) => {
                let key = key.unwrap_or_else(crypto::new_key);
                self.save_key(&key);
                self.push(&drive, &key, None)?;
                self.enable();
                Ok(None)
            }
            (Some(_), None) => Ok(Some(Problem::NeedsKey)),
            // Whatever is here and whatever is in the account are merged: nothing to choose.
            (Some(remote), Some(key)) => {
                self.merge_with(&drive, &key, &remote)?;
                self.enable();
                Ok(None)
            }
        }
    }

    /// Checks a key (from the recovery key or another device) really opens the backup, then keeps it.
    pub fn adopt_key(&self, key: &[u8; 32]) -> Result<Option<Problem>> {
        let token = self.token()?;
        let drive = Drive::new(&token);
        let remote = drive.find(FILE)?.ok_or_else(|| anyhow::anyhow!("There's no backup in this account yet."))?;
        crypto::open(key, &drive.download(&remote.id)?)?;
        self.save_key(key);
        self.connect(&token)
    }

    pub fn use_recovery_key(&self, text: &str) -> Result<Option<Problem>> {
        let key = crypto::key_from_text(text).ok_or_else(|| anyhow::anyhow!("That doesn't look like a recovery key."))?;
        self.adopt_key(&key)
    }

    /// Answers the old "which copy?" question by merging both.
    pub fn resolve_conflict(&self, _keep_account: bool) -> Result<()> {
        let token = self.token()?;
        let drive = Drive::new(&token);
        let key = self.local_key().ok_or_else(|| anyhow::anyhow!("No key"))?;
        match drive.find(FILE)? {
            Some(r) => self.merge_with(&drive, &key, &r)?,
            None => self.push(&drive, &key, None)?,
        }
        self.enable();
        Ok(())
    }

    // --- Getting the key from another device -------------------------------------------------

    /// Leaves a join request in Drive, so a device that has the key can hand it over without
    /// scanning anything. The QR code shows the same offer.
    pub fn post_join_request(&self, offer: &pairing::Offer) -> Result<()> {
        let drive = Drive::new(&self.token()?);
        drive.create(&offer.join_file_name(), &offer.join_request(device_label(), now_millis()))?;
        Ok(())
    }

    /// Looks once for another device's answer to `offer`. Ok(true) once the key is in and sync is on.
    pub fn check_pairing(&self, offer: &pairing::Offer) -> Result<bool> {
        let token = self.token()?;
        let drive = Drive::new(&token);
        let Some(file) = drive.find(&offer.file_name())? else { return Ok(false) };
        let blob = drive.download(&file.id);
        let _ = drive.delete(&file.id);
        self.cancel_join_request(offer);
        let key = offer.open(&blob?)?;
        Ok(self.adopt_key(&key)?.is_none())
    }

    pub fn cancel_join_request(&self, offer: &pairing::Offer) {
        if let Ok(token) = self.token() {
            let drive = Drive::new(&token);
            if let Ok(Some(f)) = drive.find(&offer.join_file_name()) { let _ = drive.delete(&f.id); }
        }
    }

    // --- Giving the key to another device ----------------------------------------------------

    /// Finds devices asking for the key; old requests are cleared away.
    fn look_for_joins(&self, drive: &Drive) -> Result<()> {
        let mut found = vec![];
        for f in drive.list(pairing::JOIN_PREFIX)? {
            let Some(req) = drive.download(&f.id).ok().and_then(|b| pairing::parse_join_request(&b)) else { let _ = drive.delete(&f.id); continue };
            if now_millis() - req.at > pairing::JOIN_MINUTES * 60_000 { let _ = drive.delete(&f.id); continue; }
            found.push(Join { file_id: f.id, device: req.device, confirm: req.scanned.confirm_code() });
        }
        *self.joins.lock().unwrap() = found;
        Ok(())
    }

    /// Hands the key to the device that left join request `file_id`.
    pub fn approve_join(&self, file_id: &str) -> Result<()> {
        let key = self.local_key().ok_or_else(|| anyhow::anyhow!("Turn on sync here first."))?;
        let drive = Drive::new(&self.token()?);
        let req = pairing::parse_join_request(&drive.download(file_id)?).ok_or_else(|| anyhow::anyhow!("That request has expired."))?;
        drive.create(&req.scanned.file_name(), &pairing::seal_for(&req.scanned, &key)?)?;
        let _ = drive.delete(file_id);
        self.joins.lock().unwrap().retain(|j| j.file_id != file_id);
        Ok(())
    }

    pub fn decline_join(&self, file_id: &str) -> Result<()> {
        let drive = Drive::new(&self.token()?);
        let _ = drive.delete(file_id);
        self.joins.lock().unwrap().retain(|j| j.file_id != file_id);
        Ok(())
    }

    /// For when nobody has the recovery key any more: deletes the backup in the Google account
    /// (it can't be opened without its key), makes a new key, and uploads what's on this computer
    /// as the new backup. Other devices then ask for the new key.
    pub fn start_over(&self) -> Result<Option<Problem>> {
        let token = self.token()?;
        let drive = Drive::new(&token);
        // Drive can hold more than one file with the same name; remove them all.
        for _ in 0..20 {
            match drive.find(FILE)? {
                Some(old) => drive.delete(&old.id)?,
                None => break,
            }
        }
        let key = crypto::new_key();
        self.save_key(&key);
        self.push(&drive, &key, None)?;
        self.enable();
        Ok(None)
    }

    pub fn turn_off(&self) {
        self.auth.sign_out(&self.secrets);
        *self.pending_token.lock().unwrap() = None;
        self.joins.lock().unwrap().clear();
        // The key stays, so turning sync back on doesn't need another device.
        self.update(|s| *s = Settings::default());
    }

    // --- Staying in step ---------------------------------------------------------------------

    /// Brings in changes made elsewhere and sends changes made here.
    pub fn sync(&self) -> Result<Option<Problem>> {
        if !self.settings.lock().unwrap().enabled { return Ok(None); }
        let Some(token) = self.auth.silent_token(&self.secrets)? else { return Ok(Some(Problem::NeedsSignIn)) };
        let Some(key) = self.local_key() else { return Ok(Some(Problem::NeedsKey)) };
        let result = match self.sync_with(&Drive::new(&token), &key) {
            Err(e) if e.downcast_ref::<Unauthorized>().is_some() => {
                self.auth.forget(&token);
                let Some(again) = self.auth.silent_token(&self.secrets)? else { return Ok(Some(Problem::NeedsSignIn)) };
                let drive = Drive::new(&again);
                self.sync_with(&drive, &key).map(|_| drive)
            }
            other => other.map(|_| Drive::new(&token)),
        };
        match result {
            Err(e) if e.downcast_ref::<crypto::WrongKey>().is_some() => Ok(Some(Problem::NeedsKey)),
            Err(e) => Err(e),
            Ok(drive) => {
                let _ = self.look_for_joins(&drive);
                Ok(None)
            }
        }
    }

    fn sync_with(&self, drive: &Drive, key: &[u8; 32]) -> Result<()> {
        let remote = drive.find(FILE)?;
        let s = self.snapshot();
        match remote {
            None => self.push(drive, key, None),
            Some(r) if r.modified_time != s.remote_version => self.merge_with(drive, key, &r),
            Some(r) if s.dirty => self.push(drive, key, Some(r)),
            Some(r) => { self.mark_synced(r.modified_time); Ok(()) }
        }
    }

    fn token(&self) -> Result<String> {
        if let Some(t) = self.pending_token.lock().unwrap().clone() { return Ok(t); }
        self.auth.silent_token(&self.secrets)?.ok_or_else(|| anyhow::anyhow!("Sign in to Google again first."))
    }

    /// Downloads the copy in Drive, merges it with this computer's, and saves the result on
    /// whichever side needs it. A copy made with another key fails with WrongKey and nothing changes.
    fn merge_with(&self, drive: &Drive, key: &[u8; 32], remote: &RemoteFile) -> Result<()> {
        let plain = Zeroizing::new(crypto::open(key, &drive.download(&remote.id)?)?);
        let doc = backup::import(&plain)?;
        let local = self.store.lock().unwrap().snapshot()?;
        let merged = merge::merge(&local, &doc.snapshot, now_millis());
        if merged.local_changed {
            let photo_dir = self.photo_dir.clone();
            let unused = self.store.lock().unwrap().apply(&merged.snapshot, &merged.people_from_remote, |p| {
                doc.photos.get(&p.uid).and_then(|b| crate::photos::save_bytes(b, &photo_dir).ok()).map(|p| p.to_string_lossy().into_owned())
            })?;
            for p in unused { crate::photos::delete(Some(&p), &self.photo_dir); }
        }
        if merged.remote_changed || doc.legacy {
            self.push(drive, key, Some(remote.clone()))
        } else {
            self.update(|s| s.dirty = false);
            self.mark_synced(remote.modified_time.clone());
            Ok(())
        }
    }

    fn push(&self, drive: &Drive, key: &[u8; 32], known: Option<RemoteFile>) -> Result<()> {
        let json = {
            let store = self.store.lock().unwrap();
            Zeroizing::new(backup::export(&store.snapshot()?, |p| p.photo_path.as_deref().and_then(|path| std::fs::read(path).ok())))
        };
        let blob = crypto::seal(key, &json)?;
        let existing = match known { Some(k) => Some(k), None => drive.find(FILE)? };
        let saved = match existing { Some(f) => drive.update(&f.id, &blob)?, None => drive.create(FILE, &blob)? };
        self.update(|s| s.dirty = false);
        self.mark_synced(saved.modified_time);
        Ok(())
    }

    fn mark_synced(&self, version: Option<String>) {
        let now = now_millis();
        self.update(|s| { s.remote_version = version; s.last_sync = Some(now); });
    }

    fn enable(&self) {
        *self.pending_token.lock().unwrap() = None;
        self.update(|s| s.enabled = true);
    }
}
