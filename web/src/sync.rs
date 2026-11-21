//! Sync in the browser: memi-engine does the work; this file signs in to Google, keeps the token,
//! and decides when to sync (every 10 seconds while the tab is visible, 2 seconds after a change,
//! every 3 seconds while asking another device for the key).
//!
//! Google Identity Services gives an access token for about an hour; after that, sync pauses until
//! you tap Reconnect (browsers block sign-in windows that open on their own).

use crate::drive::Drive;
use crate::webdav::{DavDrive, WebDrive};
use crate::js;
use crate::state::App;
use leptos::prelude::*;
use leptos::task::spawn_local;
use memi_engine::core::pairing;
use memi_engine::sync::{self as engine, Handle};
use memi_engine::{Problem, Provider};
use std::cell::RefCell;

const LIVE_SECONDS: u32 = 10;
const PUSH_DELAY_MS: u32 = 2_000;

/// The OAuth client for memi.yourmemi.app, set when building (MEMI_WEB_CLIENT_ID).
pub const CLIENT_ID: Option<&str> = option_env!("MEMI_WEB_CLIENT_ID");

thread_local! {
    static TOKEN: RefCell<Option<(String, f64)>> = const { RefCell::new(None) };
    /// The WebDAV password, from the browser's storage.
    static DAV_PASSWORD: RefCell<Option<String>> = const { RefCell::new(None) };
}

const DAV_PASSWORD_KEY: &str = "webdav-password";
/// The Google token and when it runs out ("token|expiry ms"), so a reload within the hour keeps syncing.
const GOOGLE_TOKEN_KEY: &str = "google-token";

/// Keeps a new Google token, in memory and in the browser, and clears the sign-in problem.
fn keep_token(app: App, t: &js::Token) {
    let until = js::now_ms() + t.expires_in * 1000.;
    TOKEN.with(|x| *x.borrow_mut() = Some((t.value.clone(), until)));
    let saved = format!("{}|{until}", t.value);
    spawn_local(async move { let _ = js::idb_set_text(GOOGLE_TOKEN_KEY, &saved).await; });
    if app.memi.with_untracked(|m| m.status.problem == Some(Problem::NeedsSignIn)) { set_problem(app, None); }
}

fn forget_token() {
    TOKEN.with(|t| *t.borrow_mut() = None);
    spawn_local(async { js::idb_delete(GOOGLE_TOKEN_KEY).await });
}

/// Where sync goes, connected: Google with an unexpired token, or WebDAV with the saved password.
fn drive_for(app: App) -> Option<WebDrive> {
    match app.memi.with_untracked(|m| m.sync.provider.clone()) {
        Provider::WebDav { url, user } => DAV_PASSWORD.with(|p| p.borrow().clone())
            .and_then(|pass| DavDrive::new(&url, &user, &pass).ok()).map(WebDrive::Dav),
        _ => TOKEN.with(|t| t.borrow().as_ref().filter(|(_, exp)| js::now_ms() < *exp - 60_000.).map(|(v, _)| WebDrive::Google(Drive::new(v)))),
    }
}

fn set_problem(app: App, p: Option<Problem>) { app.handle().with(|m| m.set_status(|s| s.problem = p)); }

/// Signed out or expired: show Reconnect.
fn needs_sign_in(app: App) -> Option<WebDrive> {
    let d = drive_for(app);
    if d.is_none() { set_problem(app, Some(Problem::NeedsSignIn)); }
    d
}

/// After an engine call: if Google rejected the token, forget it so Reconnect asks for a new one.
fn check_token(app: App) {
    if app.memi.with_untracked(|m| m.status.problem == Some(Problem::NeedsSignIn) && m.sync.provider == Provider::Google) { forget_token(); }
}

/// Called once at start.
pub fn start(app: App) {
    spawn_local(async move {
        if let Some(p) = js::idb_get_text(DAV_PASSWORD_KEY).await { DAV_PASSWORD.with(|x| *x.borrow_mut() = Some(p)); }
        // A token from earlier in the hour (a reload, or the tab reopened) carries on.
        if let Some((value, until)) = js::idb_get_text(GOOGLE_TOKEN_KEY).await.and_then(|s| {
            let (v, u) = s.rsplit_once('|')?;
            Some((v.to_string(), u.parse::<f64>().ok()?))
        }) {
            if js::now_ms() < until - 60_000. { TOKEN.with(|t| *t.borrow_mut() = Some((value, until))); }
        }
        if app.memi.with_untracked(|m| m.sync.enabled && m.sync.provider == Provider::Google) {
            js::load_gis().await;
            if drive_for(app).is_some() { sync_now(app).await; }
        }
        loop {
            gloo_timers::future::TimeoutFuture::new(LIVE_SECONDS * 1_000).await;
            let (on, idle) = app.memi.with_untracked(|m| (m.sync.enabled, !m.status.busy && m.status.problem.is_none()));
            if !on { continue; }
            if drive_for(app).is_none() {
                if app.memi.with_untracked(|m| m.status.problem.is_none()) { set_problem(app, Some(Problem::NeedsSignIn)); }
                continue;
            }
            if idle && js::is_visible() { sync_now(app).await; }
        }
    });
}

/// Something changed here: upload shortly, batching quick edits together.
pub fn changed(app: App) {
    if !app.memi.with_untracked(|m| m.sync.enabled) { return; }
    let seq = app.push_seq.get_untracked() + 1;
    app.push_seq.set(seq);
    spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(PUSH_DELAY_MS).await;
        if app.push_seq.get_untracked() == seq && drive_for(app).is_some() { sync_now(app).await; }
    });
}

pub async fn sync_now(app: App) {
    let Some(d) = needs_sign_in(app) else { return };
    engine::sync_now(&app.handle(), &d).await;
    check_token(app);
}

/// Signs in to Google (must run from a click, so the sign-in window isn't blocked) and connects.
pub async fn connect(app: App, choose_account: bool) {
    let Some(client) = CLIENT_ID else {
        set_problem(app, Some(Problem::Failed("Sync isn't set up in this build of Memi.".into())));
        return;
    };
    let hint = app.memi.with_untracked(|m| m.sync.account.clone()).unwrap_or_default();
    let prompt = if choose_account { "select_account" } else { "" };
    let t = match js::gis_request(client, &hint, prompt).await {
        Ok(t) => t,
        Err(e) => { set_problem(app, Some(Problem::Failed(format!("Google sign-in didn't finish ({e})")))); return; }
    };
    keep_token(app, &t);
    set_problem(app, None);
    app.handle().with(|m| m.update_sync(|s| s.provider = Provider::Google));
    let d = WebDrive::Google(Drive::new(&t.value));
    if app.memi.with_untracked(|m| m.sync.enabled) {
        // Reconnecting: just carry on syncing.
        engine::sync_now(&app.handle(), &d).await;
    } else {
        engine::connect(&app.handle(), &d).await;
        if app.memi.with_untracked(|m| m.status.asking.is_some()) { wait_for_key(app); }
    }
    check_token(app);
}

/// Connects to a WebDAV folder, after checking the address and password. Err is shown in the form.
pub async fn connect_webdav(app: App, url: &str, user: &str, password: &str) -> anyhow::Result<()> {
    let user = user.trim();
    anyhow::ensure!(!user.is_empty(), "Enter your user name");
    let d = DavDrive::new(url, user, password)?;
    match memi_engine::sync::Drive::find(&d, engine::FILE).await {
        Err(e) if e.is::<engine::Unauthorized>() => anyhow::bail!("The server turned down that user name and password. On Nextcloud, use an app password (Settings › Security)."),
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    js::idb_set_text(DAV_PASSWORD_KEY, password).await?;
    DAV_PASSWORD.with(|x| *x.borrow_mut() = Some(password.to_string()));
    let url = memi_engine::dav::normalise_url(url)?;
    app.handle().with(|m| { m.update_sync(|s| s.provider = Provider::WebDav { url, user: user.to_string() }); m.set_status(|s| s.problem = None) });
    let d = WebDrive::Dav(d);
    if app.memi.with_untracked(|m| m.sync.enabled) {
        engine::sync_now(&app.handle(), &d).await;
    } else {
        engine::connect(&app.handle(), &d).await;
        if app.memi.with_untracked(|m| m.status.asking.is_some()) { wait_for_key(app); }
    }
    Ok(())
}

/// While asking another device for the key: look for its answer every few seconds.
fn wait_for_key(app: App) {
    spawn_local(async move {
        let Some(id) = app.memi.with_untracked(|m| m.status.asking.as_ref().map(|a| a.id.clone())) else { return };
        let until = js::now_ms() + pairing::JOIN_MINUTES as f64 * 60_000.;
        while js::now_ms() < until {
            gloo_timers::future::TimeoutFuture::new(3_000).await;
            if app.memi.with_untracked(|m| m.status.asking.as_ref().map(|a| a.id.clone())) != Some(id.clone()) { return; }
            let Some(d) = drive_for(app) else { return };
            if let Ok(true) = engine::check_pairing(&app.handle(), &d).await { return; }
        }
        if let Some(d) = drive_for(app) { engine::cancel_asking(&app.handle(), &d).await; }
    });
}

/// The recovery key typed in. False if it doesn't open the backup.
pub async fn use_recovery_key(app: App, text: &str) -> bool {
    let Some(d) = needs_sign_in(app) else { return false };
    engine::use_recovery_key(&app.handle(), &d, text).await
}

/// Allow or Not mine, for another device asking for the key.
pub async fn answer_join(app: App, file_id: String, allow: bool) {
    let Some(d) = needs_sign_in(app) else { return };
    match engine::answer_join(&app.handle(), &d, &file_id, allow).await {
        Ok(()) if allow => app.show_toast("Sent. The other device will finish setting up in a few seconds.", None),
        Ok(()) => {}
        Err(e) => app.show_toast(format!("Couldn't send the key: {e}"), None),
    }
}

pub async fn start_over(app: App) {
    let Some(d) = needs_sign_in(app) else { return };
    engine::start_over(&app.handle(), &d).await;
}

pub async fn turn_off(app: App) {
    if let Some(d) = drive_for(app) { engine::cancel_asking(&app.handle(), &d).await; }
    if let Some(t) = TOKEN.with(|t| t.borrow_mut().take()) { js::gis_revoke(&t.0); }
    js::idb_delete(GOOGLE_TOKEN_KEY).await;
    DAV_PASSWORD.with(|x| *x.borrow_mut() = None);
    js::idb_delete(DAV_PASSWORD_KEY).await;
    app.handle().with(|m| m.turn_off_sync(false));
}

pub async fn recovery_key(app: App) -> Option<String> { app.memi.with_untracked(|m| m.recovery_key()) }
