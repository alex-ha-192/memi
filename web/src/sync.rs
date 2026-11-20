//! Sync in the browser: the same encrypted file in your Google Drive app folder as the apps, merged
//! item by item. Checks for changes every 10 seconds while the tab is visible and uploads 2 seconds
//! after a change.
//!
//! Signing in uses Google Identity Services, which gives an access token for an hour at a time;
//! after that, sync pauses until you tap Reconnect (browsers block sign-in windows that open on
//! their own).

use crate::data::Data;
use crate::drive::{Drive, RemoteFile, Unauthorized};
use crate::js;
use crate::state::{App, Asking, Join, Problem};
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_core::{backup, crypto, merge, pairing};
use std::cell::RefCell;

type Key = [u8; 32];

const FILE: &str = "memi-backup.bin";
const LIVE_SECONDS: u32 = 10;
const PUSH_DELAY_MS: u32 = 2_000;

/// The OAuth client for memi.yourmemi.app, set when building (MEMI_WEB_CLIENT_ID).
pub const CLIENT_ID: Option<&str> = option_env!("MEMI_WEB_CLIENT_ID");

thread_local! {
    static TOKEN: RefCell<Option<(String, f64)>> = const { RefCell::new(None) };
    static KEY: RefCell<Option<Key>> = const { RefCell::new(None) };
    static OFFER: RefCell<Option<std::rc::Rc<pairing::Offer>>> = const { RefCell::new(None) };
}

fn token() -> Option<String> {
    TOKEN.with(|t| t.borrow().as_ref().filter(|(_, exp)| js::now_ms() < *exp - 60_000.).map(|(v, _)| v.clone()))
}

async fn load_key() -> Option<Key> {
    if let Some(k) = KEY.with(|k| *k.borrow()) { return Some(k); }
    let bytes = js::idb_get_bytes("key").await?;
    let key: Key = bytes.try_into().ok()?;
    KEY.with(|k| *k.borrow_mut() = Some(key));
    Some(key)
}

async fn save_key(key: &Key) {
    KEY.with(|k| *k.borrow_mut() = Some(*key));
    let _ = js::idb_set_bytes("key", key).await;
}

pub async fn recovery_key() -> Option<String> { load_key().await.map(|k| crypto::key_to_text(&k)) }

fn set_problem(app: App, p: Option<Problem>) { app.sync.update(|s| s.problem = p); }

// --- Starting up and staying in step ---------------------------------------------------------------

/// Called once at start: checks every few seconds while the tab is visible.
pub fn start(app: App) {
    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(LIVE_SECONDS * 1_000).await;
            let on = app.settings.with_untracked(|s| s.sync.enabled);
            let idle = app.sync.with_untracked(|s| !s.busy && s.problem.is_none());
            if on && idle && js::is_visible() && token().is_some() { sync_now(app).await; }
            if on && token().is_none() && app.sync.with_untracked(|s| s.problem.is_none()) {
                set_problem(app, Some(Problem::NeedsSignIn));
            }
        }
    });
}

/// Something changed here: upload shortly, batching quick edits together.
pub fn changed(app: App) {
    if !app.settings.with_untracked(|s| s.sync.enabled) { return; }
    app.update_settings(|s| s.sync.dirty = true);
    let seq = app.push_seq.get_untracked() + 1;
    app.push_seq.set(seq);
    spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(PUSH_DELAY_MS).await;
        if app.push_seq.get_untracked() == seq && token().is_some() { sync_now(app).await; }
    });
}

pub async fn sync_now(app: App) {
    let Some(token) = token() else { set_problem(app, Some(Problem::NeedsSignIn)); return };
    let Some(key) = load_key().await else { set_problem(app, Some(Problem::NeedsKey)); return };
    guarded(app, async move { sync_with(app, &Drive::new(&token), &key).await }).await;
}

async fn guarded(app: App, work: impl std::future::Future<Output = anyhow::Result<()>>) {
    if app.sync.with_untracked(|s| s.busy) { return; }
    app.sync.update(|s| s.busy = true);
    let result = work.await;
    app.sync.update(|s| s.busy = false);
    match result {
        Ok(()) => {}
        Err(e) if e.is::<Unauthorized>() => {
            TOKEN.with(|t| *t.borrow_mut() = None);
            set_problem(app, Some(Problem::NeedsSignIn));
        }
        Err(e) if e.is::<crypto::WrongKey>() => {
            set_problem(app, Some(Problem::NeedsKey));
        }
        Err(e) => set_problem(app, Some(Problem::Failed(e.to_string()))),
    }
}

async fn sync_with(app: App, drive: &Drive, key: &Key) -> anyhow::Result<()> {
    let remote = drive.find(FILE).await?;
    let s = app.settings.with_untracked(|s| s.sync.clone());
    match remote {
        None => push(app, drive, key, None).await?,
        Some(r) if r.modified_time != s.remote_version => merge_with(app, drive, key, &r).await?,
        Some(r) if s.dirty => push(app, drive, key, Some(r)).await?,
        Some(r) => mark_synced(app, r.modified_time),
    }
    let _ = look_for_joins(app, drive).await;
    Ok(())
}

/// Downloads the copy in Drive, merges it with this browser's, and saves the result on whichever
/// side needs it. A copy made with another key fails and nothing changes.
async fn merge_with(app: App, drive: &Drive, key: &Key, remote: &RemoteFile) -> anyhow::Result<()> {
    let plain = crypto::open(key, &drive.download(&remote.id).await?)?;
    let doc = backup::import(&plain)?;
    let local = app.data.with_untracked(|d| d.snap.clone());
    let m = merge::merge(&local, &doc.snapshot, memi_core::model::now_millis());
    if m.local_changed {
        app.data.update(|d| d.apply_merged(m.snapshot, &m.people_from_remote, &doc.photos));
        app.save_data();
    }
    if m.remote_changed || doc.legacy {
        push(app, drive, key, Some(remote.clone())).await
    } else {
        app.update_settings(|s| s.sync.dirty = false);
        mark_synced(app, remote.modified_time.clone());
        Ok(())
    }
}

async fn push(app: App, drive: &Drive, key: &Key, known: Option<RemoteFile>) -> anyhow::Result<()> {
    let blob = crypto::seal(key, &app.data.with_untracked(Data::export))?;
    let existing = match known { Some(k) => Some(k), None => drive.find(FILE).await? };
    let saved = match existing {
        Some(f) => drive.update(&f.id, &blob).await?,
        None => drive.create(FILE, &blob).await?,
    };
    app.update_settings(|s| s.sync.dirty = false);
    mark_synced(app, saved.modified_time);
    Ok(())
}

fn mark_synced(app: App, version: Option<String>) {
    app.update_settings(|s| {
        s.sync.remote_version = version;
        s.sync.last_sync = Some(memi_core::model::now_millis());
    });
    app.sync.update(|s| s.problem = None);
}

// --- Turning it on --------------------------------------------------------------------------------

/// Signs in to Google (must run from a click, so the sign-in window isn't blocked) and connects.
pub async fn connect(app: App, choose_account: bool) {
    let Some(client) = CLIENT_ID else {
        set_problem(app, Some(Problem::Failed("Sync isn't set up in this build of Memi.".into())));
        return;
    };
    let hint = app.settings.with_untracked(|s| s.sync.account.clone()).unwrap_or_default();
    let prompt = if choose_account { "select_account" } else { "" };
    let t = match js::gis_request(client, &hint, prompt).await {
        Ok(t) => t,
        Err(e) => { set_problem(app, Some(Problem::Failed(format!("Google sign-in didn't finish ({e})")))); return; }
    };
    TOKEN.with(|x| *x.borrow_mut() = Some((t.value.clone(), js::now_ms() + t.expires_in * 1000.)));
    set_problem(app, None);
    let token = t.value;
    guarded(app, async move {
        let drive = Drive::new(&token);
        if let Some(email) = drive.email().await { app.update_settings(|s| s.sync.account = Some(email)); }
        let remote = drive.find(FILE).await?;
        let key = load_key().await;
        match (remote, key) {
            (None, key) => {
                let key = key.unwrap_or_else(|| *crypto::new_key());
                save_key(&key).await;
                enable(app);
                push(app, &drive, &key, None).await?;
            }
            (Some(_), None) => ask_for_key(app, &drive).await?,
            (Some(r), Some(key)) => {
                merge_with(app, &drive, &key, &r).await?;
                enable(app);
            }
        }
        look_for_joins(app, &drive).await
    }).await;
}

fn enable(app: App) {
    app.update_settings(|s| s.sync.enabled = true);
    set_problem(app, None);
}

/// Checks `key` really opens the backup in your account, then keeps it and finishes connecting.
async fn adopt_key(app: App, key: Key) -> anyhow::Result<bool> {
    let Some(token) = token() else { return Ok(false) };
    let drive = Drive::new(&token);
    let Some(remote) = drive.find(FILE).await? else { return Ok(false) };
    if crypto::open(&key, &drive.download(&remote.id).await?).is_err() { return Ok(false); }
    stop_asking(app);
    save_key(&key).await;
    merge_with(app, &drive, &key, &remote).await?;
    enable(app);
    Ok(true)
}

/// The recovery key typed in. False if it doesn't open the backup.
pub async fn use_recovery_key(app: App, text: &str) -> bool {
    let Some(key) = crypto::key_from_text(text) else { return false };
    let mut ok = false;
    let k = *key;
    guarded(app, async { ok = adopt_key(app, k).await?; Ok(()) }).await;
    ok
}

// --- Getting the key from another device ("start anywhere") ----------------------------------------

/// Leaves a join request in Drive and shows a QR code; whichever device answers first sends the key.
async fn ask_for_key(app: App, drive: &Drive) -> anyhow::Result<()> {
    stop_asking(app);
    let offer = std::rc::Rc::new(pairing::Offer::new());
    let _ = drive.create(&offer.join_file_name(), &offer.join_request(&js::device_label(), memi_core::model::now_millis())).await;
    let qr_svg = qrcode::QrCode::new(offer.code().as_bytes())
        .map(|q| q.render::<qrcode::render::svg::Color>().min_dimensions(200, 200).quiet_zone(true).build())
        .unwrap_or_default();
    app.sync.update(|s| {
        s.problem = Some(Problem::NeedsKey);
        s.asking = Some(Asking { id: offer.id.clone(), confirm: offer.confirm_code(), qr_svg });
    });
    OFFER.with(|o| *o.borrow_mut() = Some(offer.clone()));
    spawn_local(async move {
        let until = js::now_ms() + pairing::JOIN_MINUTES as f64 * 60_000.;
        while js::now_ms() < until {
            gloo_timers::future::TimeoutFuture::new(3_000).await;
            let current = OFFER.with(|o| o.borrow().as_ref().map(|x| x.id.clone()));
            if current.as_deref() != Some(offer.id.as_str()) { return; }
            if let Ok(true) = check_pairing(app, &offer).await { return; }
        }
        if OFFER.with(|o| o.borrow().as_ref().map(|x| x.id == offer.id).unwrap_or(false)) {
            app.sync.update(|s| s.asking = None);
        }
    });
    Ok(())
}

async fn check_pairing(app: App, offer: &pairing::Offer) -> anyhow::Result<bool> {
    let Some(token) = token() else { return Ok(false) };
    let drive = Drive::new(&token);
    let Some(file) = drive.find(&offer.file_name()).await? else { return Ok(false) };
    let blob = drive.download(&file.id).await?;
    let _ = drive.delete(&file.id).await;
    let key = *offer.open(&blob)?;
    let mut ok = false;
    guarded(app, async { ok = adopt_key(app, key).await?; Ok(()) }).await;
    Ok(ok)
}

/// Stops asking for the key and removes the request from Drive.
pub fn stop_asking(app: App) {
    let Some(offer) = OFFER.with(|o| o.borrow_mut().take()) else { return };
    app.sync.update(|s| s.asking = None);
    if let Some(t) = token() {
        spawn_local(async move {
            let drive = Drive::new(&t);
            if let Ok(Some(f)) = drive.find(&offer.join_file_name()).await { let _ = drive.delete(&f.id).await; }
        });
    }
}

// --- Giving the key to another device --------------------------------------------------------------

async fn look_for_joins(app: App, drive: &Drive) -> anyhow::Result<()> {
    let mine = OFFER.with(|o| o.borrow().as_ref().map(|x| x.join_file_name()));
    let mut found = vec![];
    for f in drive.list(pairing::JOIN_PREFIX).await? {
        if Some(&f.name) == mine.as_ref() { continue; }
        let req = drive.download(&f.id).await.ok().and_then(|b| pairing::parse_join_request(&b));
        match req {
            Some(r) if memi_core::model::now_millis() - r.at <= pairing::JOIN_MINUTES * 60_000 => {
                found.push(Join { file_id: f.id, device: r.device, confirm: r.scanned.confirm_code() });
            }
            _ => { let _ = drive.delete(&f.id).await; }
        }
    }
    app.sync.update(|s| s.joins = found);
    Ok(())
}

/// Allow: sends this browser's key to the device that asked, so only it can open it.
pub async fn answer_join(app: App, file_id: String, allow: bool) {
    app.sync.update(|s| s.joins.retain(|j| j.file_id != file_id));
    let Some(token) = token() else { set_problem(app, Some(Problem::NeedsSignIn)); return };
    let drive = Drive::new(&token);
    if !allow { let _ = drive.delete(&file_id).await; return; }
    let result: anyhow::Result<()> = async {
        let key = load_key().await.ok_or_else(|| anyhow::anyhow!("Turn on sync here first."))?;
        let req = pairing::parse_join_request(&drive.download(&file_id).await?).ok_or_else(|| anyhow::anyhow!("That request has expired."))?;
        drive.create(&req.scanned.file_name(), &pairing::seal_for(&req.scanned, &key)?).await?;
        let _ = drive.delete(&file_id).await;
        Ok(())
    }.await;
    match result {
        Ok(()) => app.show_toast("Sent. The other device will finish setting up in a few seconds.", None),
        Err(e) => app.show_toast(format!("Couldn't send the key: {e}"), None),
    }
}

// --- Starting over and turning off -----------------------------------------------------------------

/// For when nobody has the recovery key any more: replaces the backup in Google Drive with what's
/// in this browser, under a new key. Other devices then ask for the new key.
pub async fn start_over(app: App) {
    let Some(token) = token() else { set_problem(app, Some(Problem::NeedsSignIn)); return };
    guarded(app, async move {
        let drive = Drive::new(&token);
        for _ in 0..20 {
            match drive.find(FILE).await? { Some(old) => drive.delete(&old.id).await?, None => break }
        }
        stop_asking(app);
        let key = *crypto::new_key();
        save_key(&key).await;
        enable(app);
        push(app, &drive, &key, None).await
    }).await;
}

pub async fn turn_off(app: App) {
    stop_asking(app);
    if let Some(t) = token() { js::gis_revoke(&t); }
    TOKEN.with(|t| *t.borrow_mut() = None);
    app.update_settings(|s| s.sync = Default::default());
    app.sync.set(Default::default());
}
