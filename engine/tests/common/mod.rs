//! What the engine's tests share: a device that keeps everything in memory, and a cloud folder in
//! memory that can be made to fail the ways real ones do (offline, signed out, refusing writes).
#![allow(dead_code)]

use anyhow::Result;
use memi_engine::core::model::*;
use memi_engine::sync::{Drive, RemoteFile, Unauthorized};
use memi_engine::{Memi, Platform, SyncSettings};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// --- The cloud ---------------------------------------------------------------------------------------

#[derive(Default)]
pub struct Files {
    pub files: Vec<(RemoteFile, Vec<u8>)>,
    clock: u64,
}

/// A sync folder in memory. Clones share the same files, like two devices signed in to one account.
#[derive(Clone, Default)]
pub struct Cloud {
    pub store: Rc<RefCell<Files>>,
    /// Every call fails as if there were no connection.
    pub offline: Rc<Cell<bool>>,
    /// Every call fails as if the sign-in had expired.
    pub signed_out: Rc<Cell<bool>>,
    /// Uploads (create and update) fail; reads still work.
    pub read_only: Rc<Cell<bool>>,
    pub uploads: Rc<Cell<u32>>,
    pub downloads: Rc<Cell<u32>>,
}

impl Cloud {
    fn check(&self) -> Result<()> {
        if self.signed_out.get() { return Err(Unauthorized.into()); }
        if self.offline.get() { anyhow::bail!("No connection"); }
        Ok(())
    }
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.store.borrow().files.iter().map(|(f, _)| f.name.clone()).collect();
        v.sort();
        v
    }
    pub fn bytes(&self, name: &str) -> Option<Vec<u8>> {
        self.store.borrow().files.iter().find(|(f, _)| f.name == name).map(|(_, b)| b.clone())
    }
    /// Replaces a file's contents behind the apps' backs (as if another app wrote it).
    pub fn overwrite(&self, name: &str, bytes: &[u8]) {
        let mut d = self.store.borrow_mut();
        d.clock += 1;
        let t = d.clock;
        if let Some((f, b)) = d.files.iter_mut().find(|(f, _)| f.name == name) {
            f.modified_time = Some(format!("t{t}"));
            *b = bytes.to_vec();
        }
    }
}

impl Drive for Cloud {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>> {
        self.check()?;
        Ok(self.store.borrow().files.iter().rev().find(|(f, _)| f.name == name).map(|(f, _)| f.clone()))
    }
    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        self.check()?;
        Ok(self.store.borrow().files.iter().rev().filter(|(f, _)| f.name.starts_with(prefix)).map(|(f, _)| f.clone()).collect())
    }
    async fn download(&self, id: &str) -> Result<Vec<u8>> {
        self.check()?;
        self.downloads.set(self.downloads.get() + 1);
        self.store.borrow().files.iter().find(|(f, _)| f.id == id).map(|(_, b)| b.clone()).ok_or_else(|| anyhow::anyhow!("no such file"))
    }
    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> {
        self.check()?;
        if self.read_only.get() { anyhow::bail!("Storage is full"); }
        self.uploads.set(self.uploads.get() + 1);
        let mut d = self.store.borrow_mut();
        d.clock += 1;
        let f = RemoteFile { id: format!("f{}", d.clock), name: name.into(), modified_time: Some(format!("t{}", d.clock)) };
        d.files.push((f.clone(), bytes.to_vec()));
        Ok(f)
    }
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> {
        self.check()?;
        if self.read_only.get() { anyhow::bail!("Storage is full"); }
        self.uploads.set(self.uploads.get() + 1);
        let mut d = self.store.borrow_mut();
        d.clock += 1;
        let t = d.clock;
        let (f, b) = d.files.iter_mut().find(|(f, _)| f.id == id).ok_or_else(|| anyhow::anyhow!("no such file"))?;
        f.modified_time = Some(format!("t{t}"));
        *b = bytes.to_vec();
        Ok(f.clone())
    }
    async fn delete(&self, id: &str) -> Result<()> {
        self.check()?;
        self.store.borrow_mut().files.retain(|(f, _)| f.id != id);
        Ok(())
    }
    async fn email(&self) -> Option<String> { Some("maya@example.com".into()) }
}

// --- A device ----------------------------------------------------------------------------------------

#[derive(Default)]
pub struct Phone {
    /// What was last saved, and how many times.
    pub saved: Option<Snapshot>,
    pub saves: u32,
    pub fail_saves: bool,
    pub sync_saved: Option<SyncSettings>,
    pub photos: HashMap<String, Vec<u8>>,
    pub key: Option<[u8; 32]>,
    pub name: &'static str,
}

impl Platform for Phone {
    fn save(&mut self, snap: &Snapshot) -> Result<()> {
        if self.fail_saves { anyhow::bail!("disk full"); }
        self.saved = Some(snap.clone());
        self.saves += 1;
        Ok(())
    }
    fn save_sync(&mut self, s: &SyncSettings) { self.sync_saved = Some(s.clone()); }
    fn photo(&self, key: &str) -> Option<Vec<u8>> { self.photos.get(key).cloned() }
    fn save_photo(&mut self, bytes: &[u8]) -> Option<String> {
        let k = format!("photo-{}", new_uid());
        self.photos.insert(k.clone(), bytes.to_vec());
        Some(k)
    }
    fn delete_photo(&mut self, key: &str) { self.photos.remove(key); }
    fn key(&self) -> Option<[u8; 32]> { self.key }
    fn save_key(&mut self, key: &[u8; 32]) { self.key = Some(*key); }
    fn forget_key(&mut self) { self.key = None; }
    fn device_label(&self) -> String { self.name.into() }
}

pub type H = Arc<Mutex<Memi<Phone>>>;

pub fn phone(name: &'static str) -> H {
    Arc::new(Mutex::new(Memi::new(Snapshot::default(), SyncSettings::default(), Phone { name, ..Default::default() })))
}

/// Runs `f` on the device's Memi.
pub fn on<R>(h: &H, f: impl FnOnce(&mut Memi<Phone>) -> R) -> R { f(&mut h.lock().unwrap()) }

pub fn notes(h: &H) -> Vec<String> {
    let mut v: Vec<String> = on(h, |m| m.snapshot().notes.iter().map(|n| n.text.clone()).collect());
    v.sort();
    v
}

pub fn people(h: &H) -> Vec<String> { on(h, |m| m.snapshot().people.iter().map(|p| p.name.clone()).collect()) }

/// Adds someone and returns their uid and the uid of their first category (Gift ideas).
pub fn someone(h: &H, name: &str) -> (String, String) {
    on(h, |m| {
        let uid = m.add_person(Person::new(name), None);
        let gifts = m.data().categories_of(&uid)[0].uid.clone();
        (uid, gifts)
    })
}

pub fn block<F: std::future::Future>(f: F) -> F::Output { futures::executor::block_on(f) }

/// Turns sync on for `a` (first device) and brings `b` in with the recovery key.
pub fn two_in_step(a: &H, b: &H, cloud: &Cloud) {
    use memi_engine::sync;
    assert!(block(sync::connect(a, cloud)));
    assert!(block(sync::connect(b, cloud)));
    let text = on(a, |m| m.recovery_key()).unwrap();
    assert!(block(sync::use_recovery_key(b, cloud, &text)));
}

/// Waits a millisecond, so the next edit has a later edit time than the last.
pub fn tick() { std::thread::sleep(std::time::Duration::from_millis(3)); }
