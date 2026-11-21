//! Sync on the desktop: memi-engine does the work (merging, pushing, joining devices, starting
//! over); this module signs in to Google (sync/auth.rs), keeps the token fresh, and runs the
//! engine's steps on a background thread for the screens. Every method blocks, and returns the
//! problem to show, if any.

pub mod auth;
pub mod secrets;

use crate::platform::DesktopPlatform;
use anyhow::Result;
use memi_engine::native_drive::{block_on, NativeDrive};
use memi_engine::sync::{self as engine, Handle};
use memi_engine::Memi;
use std::sync::{Arc, Mutex};

pub use memi_engine::{Asking, Join, Problem, SyncSettings as Settings};

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

    /// A Drive with a fresh token, or None if you need to sign in again.
    fn drive(&self) -> Result<Option<NativeDrive>> {
        Ok(self.auth.silent_token(&self.secrets)?.map(|t| NativeDrive::new(&t)))
    }

    /// Runs `step` with a fresh token; if Google turns the token down, gets another and tries once more.
    fn with_drive(&self, step: impl Fn(&NativeDrive)) -> Result<Option<Problem>> {
        let Some(drive) = self.drive()? else { return Ok(Some(Problem::NeedsSignIn)) };
        step(&drive);
        if self.problem() == Some(Problem::NeedsSignIn) {
            if let Some(t) = self.auth.silent_token(&self.secrets).ok().flatten() { self.auth.forget(&t); }
            let Some(drive) = self.drive()? else { return Ok(Some(Problem::NeedsSignIn)) };
            self.memi.with(|m| m.set_status(|s| s.problem = None));
            step(&drive);
        }
        Ok(self.problem())
    }

    // --- Turning it on ---------------------------------------------------------------------------

    /// Opens the browser to sign in, then connects. If this computer has no key yet, it starts
    /// asking your other devices for it (see [Engine::asking]).
    pub fn sign_in_and_connect(&self) -> Result<Option<Problem>> {
        let token = self.auth.sign_in(&self.secrets)?;
        block_on(engine::connect(&self.memi, &NativeDrive::new(&token)));
        Ok(self.problem())
    }

    pub fn use_recovery_key(&self, text: &str) -> Result<Option<Problem>> {
        let Some(drive) = self.drive()? else { return Ok(Some(Problem::NeedsSignIn)) };
        if !block_on(engine::use_recovery_key(&self.memi, &drive, text)) && self.problem().is_none() {
            anyhow::bail!("That key doesn't open the backup in your Google account.");
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
        let drive = self.drive()?.ok_or_else(|| anyhow::anyhow!("Sign in to Google again first."))?;
        block_on(engine::answer_join(&self.memi, &drive, file_id, allow))
    }

    // --- Starting over and turning off -----------------------------------------------------------

    pub fn start_over(&self) -> Result<Option<Problem>> {
        self.with_drive(|d| { block_on(engine::start_over(&self.memi, d)); })
    }

    pub fn turn_off(&self) {
        self.cancel_asking();
        self.auth.sign_out(&self.secrets);
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
