//! Sync, the same on every platform: one encrypted file (`memi-backup.bin`) in the Google Drive app
//! folder, merged item by item. Each platform supplies a [Drive] (with a token it got its own way)
//! and decides when to call these: every few seconds while the app is in front, and shortly after a
//! change.
//!
//! Every function here holds the Memi only briefly (through a [Handle]), never while waiting on the
//! network, so the screens stay responsive while sync runs.

use crate::memi::{Asking, Join, Memi, Platform, Problem};
use anyhow::{anyhow, Result};
use memi_core::model::now_millis;
use memi_core::{backup, crypto, pairing};
use std::sync::{Arc, Mutex};

pub const FILE: &str = "memi-backup.bin";

#[derive(Clone, Debug)]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub modified_time: Option<String>,
}

/// Google said the sign-in has expired or was removed. Drives return this for HTTP 401.
#[derive(Debug)]
pub struct Unauthorized;
impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Google sign-in expired") }
}
impl std::error::Error for Unauthorized {}

/// The Google Drive calls sync needs, all in the hidden app folder.
#[allow(async_fn_in_trait)]
pub trait Drive {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>>;
    /// Every file whose name starts with `prefix`, newest first.
    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>>;
    async fn download(&self, id: &str) -> Result<Vec<u8>>;
    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile>;
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile>;
    async fn delete(&self, id: &str) -> Result<()>;
    /// The connected account's email, to show in Sync.
    async fn email(&self) -> Option<String>;
}

/// Shared access to the Memi: a mutex on the native apps, a signal in the browser.
pub trait Handle: Clone {
    type P: Platform;
    fn with<R>(&self, f: impl FnOnce(&mut Memi<Self::P>) -> R) -> R;
}

impl<P: Platform> Handle for Arc<Mutex<Memi<P>>> {
    type P = P;
    fn with<R>(&self, f: impl FnOnce(&mut Memi<P>) -> R) -> R { f(&mut self.lock().unwrap_or_else(|e| e.into_inner())) }
}

/// What went wrong, as the screens show it.
pub fn problem_for(e: &anyhow::Error) -> Problem {
    if e.is::<Unauthorized>() { Problem::NeedsSignIn }
    else if e.is::<crypto::WrongKey>() { Problem::NeedsKey }
    else { Problem::Failed(e.to_string()) }
}

/// Runs `work` with the busy flag set, and records any problem. Returns false if it failed or
/// something else was already running.
pub async fn guarded<H: Handle>(h: &H, work: impl std::future::Future<Output = Result<()>>) -> bool {
    let started = h.with(|m| if m.status.busy { false } else { m.set_status(|s| s.busy = true); true });
    if !started { return false; }
    let result = work.await;
    h.with(|m| m.set_status(|s| {
        s.busy = false;
        match &result {
            Ok(()) => {}
            Err(e) => s.problem = Some(problem_for(e)),
        }
    }));
    result.is_ok()
}

fn key_of<H: Handle>(h: &H) -> Result<[u8; 32]> {
    h.with(|m| m.key()).ok_or_else(|| crypto::WrongKey.into())
}

// --- Staying in step --------------------------------------------------------------------------------

/// One round: brings in changes from Drive, sends changes from here, and looks for devices asking
/// for the key.
pub async fn sync_now<H: Handle, D: Drive>(h: &H, drive: &D) -> bool {
    if !h.with(|m| m.sync.enabled) { return false; }
    guarded(h, async {
        let key = key_of(h)?;
        let remote = drive.find(FILE).await?;
        let (version, dirty) = h.with(|m| (m.sync.remote_version.clone(), m.sync.dirty));
        match remote {
            None => push(h, drive, &key, None).await?,
            Some(r) if r.modified_time != version => merge_with(h, drive, &key, &r).await?,
            Some(r) if dirty => push(h, drive, &key, Some(r)).await?,
            Some(r) => mark_synced(h, r.modified_time),
        }
        h.with(|m| m.set_status(|s| s.problem = None));
        let _ = look_for_joins(h, drive).await;
        Ok(())
    }).await
}

/// Downloads the copy in Drive, merges it with this one, and saves the result wherever it's needed.
/// A copy made with another key fails with WrongKey and nothing changes.
async fn merge_with<H: Handle, D: Drive>(h: &H, drive: &D, key: &[u8; 32], remote: &RemoteFile) -> Result<()> {
    let plain = crypto::open(key, &drive.download(&remote.id).await?)?;
    let doc = backup::import(&plain)?;
    let push_needed = h.with(|m| m.merge_in(doc));
    if push_needed {
        push(h, drive, key, Some(remote.clone())).await
    } else {
        h.with(|m| m.update_sync(|s| s.dirty = false));
        mark_synced(h, remote.modified_time.clone());
        Ok(())
    }
}

async fn push<H: Handle, D: Drive>(h: &H, drive: &D, key: &[u8; 32], known: Option<RemoteFile>) -> Result<()> {
    // Clear "dirty" before reading, so a change made during the upload is sent next time.
    let blob = h.with(|m| { m.update_sync(|s| s.dirty = false); m.sealed(key) })?;
    let existing = match known { Some(k) => Some(k), None => drive.find(FILE).await? };
    let saved = match existing {
        Some(f) => drive.update(&f.id, &blob).await,
        None => drive.create(FILE, &blob).await,
    };
    match saved {
        Ok(f) => { mark_synced(h, f.modified_time); Ok(()) }
        Err(e) => { h.with(|m| m.update_sync(|s| s.dirty = true)); Err(e) }
    }
}

fn mark_synced<H: Handle>(h: &H, version: Option<String>) {
    h.with(|m| m.update_sync(|s| { s.remote_version = version; s.last_sync = Some(now_millis()); }));
}

// --- Turning it on ----------------------------------------------------------------------------------

/// Connects with a freshly signed-in Drive. Afterwards sync is on, or the device is asking for the
/// key ([Problem::NeedsKey], with [crate::SyncStatus::asking] set).
pub async fn connect<H: Handle, D: Drive>(h: &H, drive: &D) -> bool {
    h.with(|m| m.set_status(|s| s.problem = None));
    guarded(h, async {
        if let Some(email) = drive.email().await { h.with(|m| m.update_sync(|s| s.account = Some(email))); }
        let remote = drive.find(FILE).await?;
        let key = h.with(|m| m.key());
        match (remote, key) {
            (None, key) => {
                let key = key.unwrap_or_else(|| *crypto::new_key());
                h.with(|m| { m.save_key(&key); m.update_sync(|s| s.enabled = true) });
                push(h, drive, &key, None).await?;
            }
            (Some(_), None) => ask(h, drive).await?,
            (Some(r), Some(key)) => {
                merge_with(h, drive, &key, &r).await?;
                h.with(|m| m.update_sync(|s| s.enabled = true));
            }
        }
        let _ = look_for_joins(h, drive).await;
        Ok(())
    }).await
}

/// Checks `key` opens the backup in Drive; if so keeps it, merges and turns sync on.
async fn adopt_key<H: Handle, D: Drive>(h: &H, drive: &D, key: [u8; 32]) -> Result<bool> {
    let Some(remote) = drive.find(FILE).await? else { return Ok(false) };
    if crypto::open(&key, &drive.download(&remote.id).await?).is_err() { return Ok(false); }
    cancel_asking(h, drive).await;
    h.with(|m| m.save_key(&key));
    merge_with(h, drive, &key, &remote).await?;
    h.with(|m| { m.update_sync(|s| s.enabled = true); m.set_status(|s| s.problem = None) });
    Ok(true)
}

/// The recovery key typed in. False if it isn't one, or doesn't open the backup.
pub async fn use_recovery_key<H: Handle, D: Drive>(h: &H, drive: &D, text: &str) -> bool {
    let Some(key) = crypto::key_from_text(text) else { return false };
    let key = *key;
    let mut ok = false;
    guarded(h, async { ok = adopt_key(h, drive, key).await?; Ok(()) }).await;
    ok
}

// --- Getting the key from another device ("start anywhere") -----------------------------------------

/// Leaves a join request in Drive. The screens show [Asking::confirm] (and [Asking::code] as a QR
/// code), and call [check_pairing] every few seconds until it's answered.
async fn ask<H: Handle, D: Drive>(h: &H, drive: &D) -> Result<()> {
    cancel_asking(h, drive).await;
    let offer = Arc::new(pairing::Offer::new());
    let request = offer.join_request(&h.with(|m| m.device_label()), now_millis());
    drive.create(&offer.join_file_name(), &request).await?;
    let asking = Asking { id: offer.id.clone(), code: offer.code(), confirm: offer.confirm_code() };
    h.with(|m| { m.offer = Some(offer); m.set_status(|s| { s.problem = Some(Problem::NeedsKey); s.asking = Some(asking) }) });
    Ok(())
}

/// Starts asking again (after the earlier request expired, say).
pub async fn ask_for_key<H: Handle, D: Drive>(h: &H, drive: &D) -> bool {
    guarded(h, ask(h, drive)).await
}

/// Looks once for another device's answer. True once the key is in and sync is on. Stops asking
/// after [pairing::JOIN_MINUTES].
pub async fn check_pairing<H: Handle, D: Drive>(h: &H, drive: &D) -> Result<bool> {
    let Some(offer) = h.with(|m| m.offer.clone()) else { return Ok(false) };
    let Some(file) = drive.find(&offer.file_name()).await? else { return Ok(false) };
    let blob = drive.download(&file.id).await?;
    let _ = drive.delete(&file.id).await;
    let key = *offer.open(&blob)?;
    let mut ok = false;
    guarded(h, async { ok = adopt_key(h, drive, key).await?; Ok(()) }).await;
    Ok(ok)
}

/// Stops asking for the key and removes the request from Drive.
pub async fn cancel_asking<H: Handle, D: Drive>(h: &H, drive: &D) {
    let Some(offer) = h.with(|m| { let o = m.offer.take(); m.set_status(|s| s.asking = None); o }) else { return };
    if let Ok(Some(f)) = drive.find(&offer.join_file_name()).await { let _ = drive.delete(&f.id).await; }
}

// --- Giving the key to another device ----------------------------------------------------------------

/// Finds devices asking for the key; old or broken requests are cleared away.
pub async fn look_for_joins<H: Handle, D: Drive>(h: &H, drive: &D) -> Result<()> {
    let mine = h.with(|m| m.offer.as_ref().map(|o| o.join_file_name()));
    let mut found = vec![];
    for f in drive.list(pairing::JOIN_PREFIX).await? {
        if Some(&f.name) == mine.as_ref() { continue; }
        let req = drive.download(&f.id).await.ok().and_then(|b| pairing::parse_join_request(&b));
        match req {
            Some(r) if now_millis() - r.at <= pairing::JOIN_MINUTES * 60_000 => {
                found.push(Join { file_id: f.id, device: r.device, confirm: r.scanned.confirm_code() });
            }
            _ => { let _ = drive.delete(&f.id).await; }
        }
    }
    h.with(|m| if m.status.joins != found { m.set_status(|s| s.joins = found) });
    Ok(())
}

/// Allow (sends this device's key so only the asking device can open it) or Not mine.
pub async fn answer_join<H: Handle, D: Drive>(h: &H, drive: &D, file_id: &str, allow: bool) -> Result<()> {
    h.with(|m| m.set_status(|s| s.joins.retain(|j| j.file_id != file_id)));
    if !allow { return drive.delete(file_id).await; }
    let key = h.with(|m| m.key()).ok_or_else(|| anyhow!("Turn on sync here first."))?;
    let req = pairing::parse_join_request(&drive.download(file_id).await?).ok_or_else(|| anyhow!("That request has expired."))?;
    drive.create(&req.scanned.file_name(), &pairing::seal_for(&req.scanned, &key)?).await?;
    let _ = drive.delete(file_id).await;
    Ok(())
}

/// A code shown by another device (scanned from its QR code), answered with this device's key.
pub async fn send_key_to<H: Handle, D: Drive>(h: &H, drive: &D, code: &str) -> Result<()> {
    let scanned = pairing::parse(code).ok_or_else(|| anyhow!("That isn't a Memi code. On the other device, open Sync to show one."))?;
    let key = h.with(|m| m.key()).ok_or_else(|| anyhow!("Turn on sync here first."))?;
    drive.create(&scanned.file_name(), &pairing::seal_for(&scanned, &key)?).await?;
    Ok(())
}

// --- Starting over -------------------------------------------------------------------------------------

/// For when nobody has the recovery key any more: replaces the backup in Drive with what's here,
/// under a new key. Other devices then ask for the new key.
pub async fn start_over<H: Handle, D: Drive>(h: &H, drive: &D) -> bool {
    guarded(h, async {
        for _ in 0..20 {
            match drive.find(FILE).await? { Some(old) => drive.delete(&old.id).await?, None => break }
        }
        cancel_asking(h, drive).await;
        let key = *crypto::new_key();
        h.with(|m| { m.save_key(&key); m.update_sync(|s| s.enabled = true); m.set_status(|s| s.problem = None) });
        push(h, drive, &key, None).await
    }).await
}
