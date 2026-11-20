//! Keeps an encrypted copy of everything in the Google account you pick, the same way the
//! Android app does (same file, same format), so a phone and a computer stay in step.
//!
//! It's one encrypted file, so if two devices change things at the same moment, the last one
//! to upload wins. All the methods here block (network and disk), so the UI runs them on a
//! background thread.

pub mod auth;
pub mod crypto;
pub mod drive;
pub mod pairing;
pub mod secrets;

use crate::backup;
use crate::store::Store;
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use drive::{Drive, RemoteFile, Unauthorized};
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
    /// There's a backup, but this computer doesn't have its key yet (QR code or recovery key).
    NeedsKey,
    /// Both this computer and the account have people; you choose which to keep.
    Conflict,
    Failed(String),
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
    /// Kept between signing in and answering a key or conflict question.
    pending_token: Mutex<Option<String>>,
}

impl Engine {
    pub fn new(store: Arc<Mutex<Store>>, data_dir: &Path, photo_dir: PathBuf) -> Self {
        let settings_path = data_dir.join("sync.json");
        let settings = std::fs::read(&settings_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Engine {
            store, photo_dir, settings_path, settings: Mutex::new(settings),
            secrets: secrets::Secrets::new(data_dir), auth: auth::Auth::new(), pending_token: Mutex::new(None),
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
        let key = self.local_key();
        let empty_here = self.store.lock().unwrap().people()?.is_empty();
        match (remote, key) {
            (None, key) => {
                let key = key.unwrap_or_else(crypto::new_key);
                self.save_key(&key);
                self.push(&drive, &key, None)?;
                self.enable();
                Ok(None)
            }
            (Some(_), None) => Ok(Some(Problem::NeedsKey)),
            (Some(remote), Some(key)) if empty_here => {
                self.restore(&drive, &key, &remote)?;
                self.enable();
                Ok(None)
            }
            (Some(_), Some(_)) => Ok(Some(Problem::Conflict)),
        }
    }

    /// Checks a key (from the recovery key or the phone) really opens the backup, then keeps it.
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

    /// Answers [Problem::Conflict]: keep what's in the Google account, or what's on this computer.
    pub fn resolve_conflict(&self, keep_account: bool) -> Result<()> {
        let token = self.token()?;
        let drive = Drive::new(&token);
        let key = self.local_key().ok_or_else(|| anyhow::anyhow!("No key"))?;
        let remote = drive.find(FILE)?;
        match (keep_account, remote) {
            (true, Some(r)) => self.restore(&drive, &key, &r)?,
            (_, r) => self.push(&drive, &key, r)?,
        }
        self.enable();
        Ok(())
    }

    // --- QR pairing: the phone hands this computer the key ----------------------------------

    /// Looks once for the phone's answer to `offer`. Ok(true) once the key is in and sync is on.
    pub fn check_pairing(&self, offer: &pairing::Offer) -> Result<bool> {
        let token = self.token()?;
        let drive = Drive::new(&token);
        let Some(file) = drive.find(&offer.file_name())? else { return Ok(false) };
        let blob = drive.download(&file.id);
        let _ = drive.delete(&file.id);
        let key = offer.open(&blob?)?;
        Ok(self.adopt_key(&key)?.is_none())
    }

    pub fn turn_off(&self) {
        self.auth.sign_out(&self.secrets);
        *self.pending_token.lock().unwrap() = None;
        // The key stays, so turning sync back on doesn't need the phone again.
        self.update(|s| *s = Settings::default());
    }

    // --- Staying in step ---------------------------------------------------------------------

    /// Sends local changes, or brings in changes made elsewhere.
    pub fn sync(&self) -> Result<Option<Problem>> {
        if !self.settings.lock().unwrap().enabled { return Ok(None); }
        let Some(token) = self.auth.silent_token(&self.secrets)? else { return Ok(Some(Problem::NeedsSignIn)) };
        let Some(key) = self.local_key() else { return Ok(Some(Problem::NeedsKey)) };
        match self.sync_with(&Drive::new(&token), &key) {
            Err(e) if e.downcast_ref::<Unauthorized>().is_some() => {
                self.auth.forget(&token);
                let Some(again) = self.auth.silent_token(&self.secrets)? else { return Ok(Some(Problem::NeedsSignIn)) };
                self.sync_with(&Drive::new(&again), &key)?;
                Ok(None)
            }
            Err(e) if e.downcast_ref::<crypto::WrongKey>().is_some() => Ok(Some(Problem::NeedsKey)),
            other => other.map(|_| None),
        }
    }

    fn sync_with(&self, drive: &Drive, key: &[u8; 32]) -> Result<()> {
        let remote = drive.find(FILE)?;
        let s = self.snapshot();
        match remote {
            None => self.push(drive, key, None),
            Some(_) if s.dirty => self.push(drive, key, remote),
            Some(r) if r.modified_time != s.remote_version => self.restore(drive, key, &r),
            Some(r) => { self.mark_synced(r.modified_time); Ok(()) }
        }
    }

    fn token(&self) -> Result<String> {
        if let Some(t) = self.pending_token.lock().unwrap().clone() { return Ok(t); }
        self.auth.silent_token(&self.secrets)?.ok_or_else(|| anyhow::anyhow!("Sign in to Google again first."))
    }

    fn push(&self, drive: &Drive, key: &[u8; 32], known: Option<RemoteFile>) -> Result<()> {
        let json = {
            let store = self.store.lock().unwrap();
            Zeroizing::new(backup::export(&store.people()?, &store.collections()?, &store.notes()?))
        };
        let blob = crypto::seal(key, &json)?;
        let existing = match known { Some(k) => Some(k), None => drive.find(FILE)? };
        let saved = match existing { Some(f) => drive.update(&f.id, &blob)?, None => drive.create(FILE, &blob)? };
        self.update(|s| s.dirty = false);
        self.mark_synced(saved.modified_time);
        Ok(())
    }

    fn restore(&self, drive: &Drive, key: &[u8; 32], remote: &RemoteFile) -> Result<()> {
        let json = Zeroizing::new(crypto::open(key, &drive.download(&remote.id)?)?);
        let contents = backup::import(&json, &self.photo_dir)?;
        {
            let mut store = self.store.lock().unwrap();
            let old: Vec<String> = store.people()?.into_iter().filter_map(|p| p.photo_path).collect();
            store.replace_all(&contents.people, &contents.collections, &contents.notes)?;
            let kept: Vec<&String> = contents.people.iter().filter_map(|p| p.photo_path.as_ref()).collect();
            for p in old.iter().filter(|p| !kept.contains(p)) { crate::photos::delete(Some(p), &self.photo_dir); }
        }
        self.update(|s| s.dirty = false);
        self.mark_synced(remote.modified_time.clone());
        Ok(())
    }

    fn mark_synced(&self, version: Option<String>) {
        let now = crate::model::now_millis();
        self.update(|s| { s.remote_version = version; s.last_sync = Some(now); });
    }

    fn enable(&self) {
        *self.pending_token.lock().unwrap() = None;
        self.update(|s| s.enabled = true);
    }
}
