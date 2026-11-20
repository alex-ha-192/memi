//! Secrets (the sync key and Google's sign-in token) go in the system password store: Windows
//! Credential Manager, or the Secret Service on Linux (GNOME Keyring, KWallet). Where there's no
//! password store, they fall back to a file in Your Memi's folder that only your account can read.

use std::collections::BTreeMap;
use std::path::PathBuf;

const SERVICE: &str = "Your Memi";

pub struct Secrets {
    fallback: PathBuf,
}

impl Secrets {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Secrets { fallback: data_dir.join("secrets.json") }
    }

    pub fn get(&self, name: &str) -> Option<String> {
        if let Ok(v) = keyring::Entry::new(SERVICE, name).and_then(|e| e.get_password()) {
            return Some(v);
        }
        self.read_file().remove(name)
    }

    pub fn set(&self, name: &str, value: &str) {
        let stored = keyring::Entry::new(SERVICE, name).and_then(|e| e.set_password(value)).is_ok()
            // Check it can be read back: some stores accept writes they can't return.
            && keyring::Entry::new(SERVICE, name).and_then(|e| e.get_password()).map(|v| v == value).unwrap_or(false);
        let mut file = self.read_file();
        if stored { file.remove(name); } else { file.insert(name.to_string(), value.to_string()); }
        self.write_file(&file);
    }

    pub fn delete(&self, name: &str) {
        if let Ok(e) = keyring::Entry::new(SERVICE, name) { let _ = e.delete_credential(); }
        let mut file = self.read_file();
        if file.remove(name).is_some() { self.write_file(&file); }
    }

    fn read_file(&self) -> BTreeMap<String, String> {
        std::fs::read(&self.fallback).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn write_file(&self, map: &BTreeMap<String, String>) {
        if map.is_empty() {
            let _ = std::fs::remove_file(&self.fallback);
            return;
        }
        let Ok(json) = serde_json::to_vec(map) else { return };
        let tmp = self.fallback.with_extension("tmp");
        // Start from a fresh file so its permissions are always the private ones set below.
        let _ = std::fs::remove_file(&tmp);
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let Ok(mut f) = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp) else { return };
            if f.write_all(&json).is_err() { return; }
        }
        #[cfg(not(unix))]
        {
            // On Windows the user's AppData folder is already private to them.
            if std::fs::write(&tmp, &json).is_err() { return; }
        }
        let _ = std::fs::rename(&tmp, &self.fallback);
    }
}
