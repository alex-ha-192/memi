//! The desktop's side of memi-engine: where things are kept on this computer. The same places as
//! before the engine, so existing installs carry straight over:
//!
//! - everything in `memi.db` (SQLite, through the engine's shared store),
//! - photos as JPEG files in the photos folder,
//! - sync settings in `sync.json`,
//! - the sync key in Windows Credential Manager or the Linux Secret Service (see sync/secrets.rs).

use crate::photos;
use crate::sync::secrets::Secrets;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use memi_engine::core::model::Snapshot;
use memi_engine::sqlite::SqliteStore;
use memi_engine::{Platform, SyncSettings};
use std::path::PathBuf;
use std::sync::Arc;
use zeroize::Zeroizing;

const KEY: &str = "sync-key";

pub struct DesktopPlatform {
    store: SqliteStore,
    photo_dir: PathBuf,
    sync_path: PathBuf,
    secrets: Arc<Secrets>,
}

impl DesktopPlatform {
    pub fn new(store: SqliteStore, photo_dir: PathBuf, data_dir: &std::path::Path, secrets: Arc<Secrets>) -> Self {
        DesktopPlatform { store, photo_dir, sync_path: data_dir.join("sync.json"), secrets }
    }

    /// The sync settings saved on this computer.
    pub fn load_sync(data_dir: &std::path::Path) -> SyncSettings {
        std::fs::read(data_dir.join("sync.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
}

impl Platform for DesktopPlatform {
    fn save(&mut self, snap: &Snapshot) -> anyhow::Result<()> { self.store.write(snap) }

    fn save_sync(&mut self, sync: &SyncSettings) {
        if let Ok(json) = serde_json::to_vec_pretty(sync) {
            let tmp = self.sync_path.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() { let _ = std::fs::rename(tmp, &self.sync_path); }
        }
    }

    fn photo(&self, key: &str) -> Option<Vec<u8>> { std::fs::read(key).ok() }

    fn save_photo(&mut self, bytes: &[u8]) -> Option<String> {
        photos::save_bytes(bytes, &self.photo_dir).ok().map(|p| p.to_string_lossy().into_owned())
    }

    fn delete_photo(&mut self, key: &str) { photos::delete(Some(key), &self.photo_dir); }

    fn key(&self) -> Option<[u8; 32]> {
        let b = Zeroizing::new(STANDARD.decode(self.secrets.get(KEY)?).ok()?);
        b.as_slice().try_into().ok()
    }

    fn save_key(&mut self, key: &[u8; 32]) { self.secrets.set(KEY, &STANDARD.encode(key)); }

    fn forget_key(&mut self) { self.secrets.delete(KEY); }

    fn device_label(&self) -> String {
        if cfg!(target_os = "windows") { "Windows computer" } else if cfg!(target_os = "macos") { "Mac" } else { "Linux computer" }.into()
    }
}
