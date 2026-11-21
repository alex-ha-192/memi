//! Sync on the desktop: memi-engine does the work (merging, pushing, joining devices, starting
//! over); this module signs in to Google (sync/auth.rs), keeps the token fresh, and runs the
//! engine's steps on a background thread for the screens. Every method blocks, and returns the
//! problem to show, if any.

pub mod auth;
pub mod secrets;

use crate::platform::DesktopPlatform;
use anyhow::Result;
use memi_engine::any_drive::AnyDrive;
use memi_engine::folder_drive::FolderDrive;
use memi_engine::native_drive::{block_on, NativeDrive};
use memi_engine::webdav::WebDavDrive;
use memi_engine::Provider;
use memi_engine::sync::{self as engine, Handle};
use memi_engine::Memi;
use std::sync::{Arc, Mutex};

pub use memi_engine::{Asking, Join, Problem, SyncSettings as Settings};

/// The secret-store entry for the WebDAV password.
const WEBDAV_PASSWORD: &str = "webdav-password";

pub type Shared = Arc<Mutex<Memi<DesktopPlatform>>>;

pub struct Engine {
    memi: Shared,
    secrets: Arc<secrets::Secrets>,
    auth: auth::Auth,
}

impl Engine {
    pub fn new(memi: Shared, secrets: Arc<secrets::Secrets>) -> Self {
        Engine { memi, secrets, auth: auth::Auth::new() }
    }

    pub fn snapshot(&self) -> Settings { self.memi.with(|m| m.sync.clone()) }
    pub fn joins(&self) -> Vec<Join> { self.memi.with(|m| m.status.joins.clone()) }
    pub fn asking(&self) -> Option<Asking> { self.memi.with(|m| m.status.asking.clone()) }
    pub fn recovery_key(&self) -> Option<String> { self.memi.with(|m| m.recovery_key()) }
    pub fn wants_key_nudge(&self) -> bool { self.memi.with(|m| m.wants_key_nudge(memi_engine::core::model::now_millis())) }
    pub fn key_saved(&self) { self.memi.with(|m| m.key_saved()) }
    pub fn snooze_key_nudge(&self) { self.memi.with(|m| m.snooze_key_nudge(memi_engine::core::model::now_millis())) }

    fn problem(&self) -> Option<Problem> { self.memi.with(|m| m.status.problem.clone()) }

    /// Where sync goes, connected: Google with a fresh token, WebDAV with the saved password, or
    /// the folder. None if you need to sign in again.
    fn drive(&self) -> Result<Option<AnyDrive>> {
        Ok(match self.snapshot().provider {
            Provider::Google => self.auth.silent_token(&self.secrets)?.map(|t| AnyDrive::Google(NativeDrive::new(&t))),
            Provider::WebDav { url, user } => match self.secrets.get(WEBDAV_PASSWORD) {
                Some(pass) => Some(AnyDrive::WebDav(WebDavDrive::new(&url, &user, &pass)?)),
                None => None,
            },
            Provider::Folder { path } => Some(AnyDrive::Folder(FolderDrive::new(path))),
        })
    }

    /// Runs `step`; if Google turns the token down, gets another and tries once more.
    fn with_drive(&self, step: impl Fn(&AnyDrive)) -> Result<Option<Problem>> {
        let Some(drive) = self.drive()? else { return Ok(Some(Problem::NeedsSignIn)) };
        step(&drive);
        if self.problem() == Some(Problem::NeedsSignIn) && matches!(drive, AnyDrive::Google(_)) {
            if let Some(t) = self.auth.silent_token(&self.secrets).ok().flatten() { self.auth.forget(&t); }
            let Some(drive) = self.drive()? else { return Ok(Some(Problem::NeedsSignIn)) };
            self.memi.with(|m| m.set_status(|s| s.problem = None));
            step(&drive);
        }
        Ok(self.problem())
    }

    /// "your Google account", "your WebDAV folder", "the folder": for messages.
    fn place(&self) -> &'static str {
        match self.snapshot().provider { Provider::Google => "your Google account", Provider::WebDav { .. } => "your WebDAV folder", Provider::Folder { .. } => "the folder" }
    }

    // --- Turning it on ---------------------------------------------------------------------------

    /// Opens the browser to sign in, then connects. If this computer has no key yet, it starts
    /// asking your other devices for it (see [Engine::asking]).
    pub fn sign_in_and_connect(&self) -> Result<Option<Problem>> {
        let token = self.auth.sign_in(&self.secrets)?;
        self.memi.with(|m| m.update_sync(|s| s.provider = Provider::Google));
        block_on(engine::connect(&self.memi, &AnyDrive::Google(NativeDrive::new(&token))));
        Ok(self.problem())
    }

    /// Connects to a WebDAV folder. Checks the address and password first, so a mistake is
    /// reported here rather than turning sync on with nothing behind it.
    pub fn connect_webdav(&self, url: &str, user: &str, password: &str) -> Result<Option<Problem>> {
        let url = memi_engine::webdav::normalise_url(url)?;
        anyhow::ensure!(!user.trim().is_empty(), "Enter your user name");
        let drive = WebDavDrive::new(&url, user.trim(), password)?;
        match block_on(memi_engine::sync::Drive::find(&drive, memi_engine::sync::FILE)) {
            Err(e) if e.is::<memi_engine::sync::Unauthorized>() => anyhow::bail!("The server turned down that user name and password. On Nextcloud, use an app password (Settings › Security)."),
            Err(e) => return Err(e),
            Ok(_) => {}
        }
        self.secrets.set(WEBDAV_PASSWORD, password);
        self.memi.with(|m| m.update_sync(|s| s.provider = Provider::WebDav { url: url.clone(), user: user.trim().to_string() }));
        block_on(engine::connect(&self.memi, &AnyDrive::WebDav(drive)));
        Ok(self.problem())
    }

    /// Syncs through a folder that something else keeps the same on each device.
    pub fn connect_folder(&self, path: &std::path::Path) -> Result<Option<Problem>> {
        anyhow::ensure!(path.is_dir(), "That folder isn't there");
        let path = path.to_string_lossy().into_owned();
        self.memi.with(|m| m.update_sync(|s| s.provider = Provider::Folder { path: path.clone() }));
        block_on(engine::connect(&self.memi, &AnyDrive::Folder(FolderDrive::new(path))));
        Ok(self.problem())
    }

    pub fn use_recovery_key(&self, text: &str) -> Result<Option<Problem>> {
        let Some(drive) = self.drive()? else { return Ok(Some(Problem::NeedsSignIn)) };
        if !block_on(engine::use_recovery_key(&self.memi, &drive, text)) && self.problem().is_none() {
            anyhow::bail!("That key doesn't open the backup in {}.", self.place());
        }
        Ok(self.problem())
    }

    // --- Getting the key from another device ----------------------------------------------------

    /// Asks your other devices for the key (a join request in Drive, and a QR code for a phone).
    pub fn ask_for_key(&self) -> Result<Option<Asking>> {
        if let Some(a) = self.asking() { return Ok(Some(a)); }
        self.with_drive(|d| { block_on(engine::ask_for_key(&self.memi, d)); })?;
        Ok(self.asking())
    }

    /// Looks once for another device's answer. True once the key is in and sync is on.
    pub fn check_pairing(&self) -> Result<bool> {
        let Some(drive) = self.drive()? else { return Ok(false) };
        block_on(engine::check_pairing(&self.memi, &drive))
    }

    pub fn cancel_asking(&self) {
        if let Ok(Some(drive)) = self.drive() { block_on(engine::cancel_asking(&self.memi, &drive)); }
        else { self.memi.with(|m| m.set_status(|s| s.asking = None)); }
    }

    // --- Giving the key to another device --------------------------------------------------------

    pub fn approve_join(&self, file_id: &str) -> Result<()> { self.answer(file_id, true) }
    pub fn decline_join(&self, file_id: &str) -> Result<()> { self.answer(file_id, false) }

    fn answer(&self, file_id: &str, allow: bool) -> Result<()> {
        let drive = self.drive()?.ok_or_else(|| anyhow::anyhow!("Sign in to {} again first.", self.place()))?;
        block_on(engine::answer_join(&self.memi, &drive, file_id, allow))
    }

    // --- Starting over and turning off -----------------------------------------------------------

    pub fn start_over(&self) -> Result<Option<Problem>> {
        self.with_drive(|d| { block_on(engine::start_over(&self.memi, d)); })
    }

    pub fn turn_off(&self) {
        self.cancel_asking();
        self.auth.sign_out(&self.secrets);
        self.secrets.delete(WEBDAV_PASSWORD);
        // The key stays, so turning sync back on doesn't need another device.
        self.memi.with(|m| m.turn_off_sync(false));
    }

    // --- Staying in step -------------------------------------------------------------------------

    /// Brings in changes made elsewhere and sends changes made here.
    pub fn sync(&self) -> Result<Option<Problem>> {
        if !self.snapshot().enabled { return Ok(None); }
        self.with_drive(|d| { block_on(engine::sync_now(&self.memi, d)); })
    }
}
