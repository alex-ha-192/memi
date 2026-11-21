//! Sync through a folder: one that Syncthing, Dropbox, OneDrive, iCloud Drive, a USB stick or a
//! network share keeps the same on each device. Memi only reads and writes its own encrypted files
//! there; moving them between devices is the folder's job.
//!
//! A file's id is its name. Its "modified time", which sync only compares for equality, is the
//! modification time, the size and a hash of the contents, so even a quick rewrite shows.

use crate::sync::RemoteFile;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// FNV-1a: quick, and plenty to tell two versions of a file apart (not for security).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
}

pub struct FolderDrive {
    dir: PathBuf,
}

impl FolderDrive {
    pub fn new(dir: impl Into<PathBuf>) -> Self { FolderDrive { dir: dir.into() } }

    fn path(&self, name: &str) -> Result<PathBuf> {
        anyhow::ensure!(!name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != "..", "Bad file name");
        Ok(self.dir.join(name))
    }

    fn ready(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dir).with_context(|| format!("Couldn't open the folder {}", self.dir.display()))
    }

    fn remote(name: &str, path: &Path) -> Result<RemoteFile> {
        let meta = std::fs::metadata(path)?;
        let nanos = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
        // Many file systems only keep the time to a few milliseconds, so two quick rewrites of the
        // same size could look alike: a hash of the contents tells them apart.
        let hash = fnv1a(&std::fs::read(path)?);
        Ok(RemoteFile { id: name.to_string(), name: name.to_string(), modified_time: Some(format!("{nanos}-{}-{hash:016x}", meta.len())) })
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> {
        self.ready()?;
        let path = self.path(name)?;
        // Write beside it, then swap it in, so the other devices never see half a file.
        let tmp = self.dir.join(format!(".{name}.part"));
        std::fs::write(&tmp, bytes).with_context(|| format!("Couldn't write to {}", self.dir.display()))?;
        std::fs::rename(&tmp, &path)?;
        Self::remote(name, &path)
    }
}

impl crate::sync::Drive for FolderDrive {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>> {
        let path = self.path(name)?;
        if !path.is_file() { return Ok(None); }
        Ok(Some(Self::remote(name, &path)?))
    }

    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        self.ready()?;
        let mut found = vec![];
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with(prefix) || !entry.path().is_file() { continue; }
            let when = entry.metadata()?.modified().ok();
            found.push((when, Self::remote(&name, &entry.path())?));
        }
        found.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(found.into_iter().map(|(_, f)| f).collect())
    }

    async fn download(&self, id: &str) -> Result<Vec<u8>> {
        std::fs::read(self.path(id)?).with_context(|| format!("Couldn't read {id}"))
    }

    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> { self.write(name, bytes) }
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> { self.write(id, bytes) }

    async fn delete(&self, id: &str) -> Result<()> {
        match std::fs::remove_file(self.path(id)?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    async fn email(&self) -> Option<String> { Some(self.dir.display().to_string()) }
}
