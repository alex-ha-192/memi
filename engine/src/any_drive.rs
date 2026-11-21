//! Whichever place sync uses, as one type the native apps can hold (the Drive trait's async
//! methods rule out `dyn Drive`).

use crate::{folder_drive::FolderDrive, native_drive::NativeDrive, sync::{Drive, RemoteFile}, webdav::WebDavDrive};
use anyhow::Result;

pub enum AnyDrive {
    Google(NativeDrive),
    WebDav(WebDavDrive),
    Folder(FolderDrive),
}

macro_rules! each {
    ($self:ident, $d:ident => $e:expr) => {
        match $self { AnyDrive::Google($d) => $e, AnyDrive::WebDav($d) => $e, AnyDrive::Folder($d) => $e }
    };
}

impl Drive for AnyDrive {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>> { each!(self, d => d.find(name).await) }
    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> { each!(self, d => d.list(prefix).await) }
    async fn download(&self, id: &str) -> Result<Vec<u8>> { each!(self, d => d.download(id).await) }
    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> { each!(self, d => d.create(name, bytes).await) }
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> { each!(self, d => d.update(id, bytes).await) }
    async fn delete(&self, id: &str) -> Result<()> { each!(self, d => d.delete(id).await) }
    async fn email(&self) -> Option<String> { each!(self, d => d.email().await) }
}
