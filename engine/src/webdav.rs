//! WebDAV (Nextcloud, ownCloud, a NAS, Fastmail Files...) for the native apps. Memi keeps its files
//! in one folder the person chooses, given as a URL. Everything in it is encrypted, as on Google
//! Drive. Signing in is a user name and a password (on Nextcloud, an app password).
//!
//! A file's id is its name. Its "modified time", which sync only ever compares for equality, is
//! the server's ETag, which changes on every write; list() sorts by the real last-modified time.

use crate::sync::{RemoteFile, Unauthorized};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
pub use crate::dav::*;

const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;

pub struct WebDavDrive {
    /// The folder, always ending in "/".
    base: String,
    user: String,
    auth: String,
    agent: ureq::Agent,
    folder_ready: AtomicBool,
}

fn check(res: std::result::Result<ureq::Response, ureq::Error>) -> Result<ureq::Response> {
    match res {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(401, _)) => Err(Unauthorized.into()),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            bail!("The WebDAV server said {code}: {}", body.chars().take(200).collect::<String>())
        }
        Err(e) => Err(anyhow::anyhow!("Couldn't reach the WebDAV server ({e})")),
    }
}

impl WebDavDrive {
    /// `url` is the folder to keep Memi's files in, e.g.
    /// https://cloud.example.com/remote.php/dav/files/alex/Memi/
    pub fn new(url: &str, user: &str, password: &str) -> Result<Self> {
        let base = normalise_url(url)?;
        let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(20)).timeout(Duration::from_secs(120)).build();
        Ok(WebDavDrive {
            base,
            user: user.to_string(),
            auth: format!("Basic {}", STANDARD.encode(format!("{user}:{password}"))),
            agent,
            folder_ready: AtomicBool::new(false),
        })
    }

    fn url(&self, name: &str) -> String { format!("{}{}", self.base, percent_encode(name)) }

    fn propfind(&self, url: &str, depth: &str) -> std::result::Result<ureq::Response, ureq::Error> {
        self.agent.request("PROPFIND", url).set("Authorization", &self.auth).set("Depth", depth)
            .set("Content-Type", "application/xml; charset=utf-8").send_string(PROPFIND_BODY)
    }

    /// Makes the folder if it isn't there yet (once per connection).
    fn ensure_folder(&self) -> Result<()> {
        if self.folder_ready.load(Ordering::Relaxed) { return Ok(()); }
        match self.propfind(&self.base, "0") {
            Ok(_) => {}
            Err(ureq::Error::Status(404, _)) => {
                check(self.agent.request("MKCOL", &self.base).set("Authorization", &self.auth).call())
                    .context("Couldn't make the Memi folder on the WebDAV server")?;
            }
            Err(e) => return Err(check(Err(e)).unwrap_err()),
        }
        self.folder_ready.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn entries(&self) -> Result<Vec<Entry>> {
        self.ensure_folder()?;
        let r = check(self.propfind(&self.base, "1"))?;
        let xml = r.into_string()?;
        Ok(parse_propfind(&xml).into_iter().filter(|e| !e.folder && !e.name.is_empty()).collect())
    }

    fn as_remote(e: Entry) -> RemoteFile {
        RemoteFile { id: e.name.clone(), modified_time: e.etag.or(Some(e.modified.to_string())), name: e.name }
    }

    fn find_blocking(&self, name: &str) -> Result<Option<RemoteFile>> {
        self.ensure_folder()?;
        match self.propfind(&self.url(name), "0") {
            Ok(r) => {
                let xml = r.into_string()?;
                Ok(parse_propfind(&xml).into_iter().find(|e| !e.folder).map(|mut e| { e.name = name.to_string(); Self::as_remote(e) }))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(check(Err(e)).unwrap_err()),
        }
    }

    fn put(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> {
        self.ensure_folder()?;
        check(self.agent.put(&self.url(name)).set("Authorization", &self.auth)
            .set("Content-Type", "application/octet-stream").send_bytes(bytes))?;
        self.find_blocking(name)?.context("The WebDAV server didn't keep the file")
    }
}

impl crate::sync::Drive for WebDavDrive {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>> { self.find_blocking(name) }

    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        let mut found: Vec<Entry> = self.entries()?.into_iter().filter(|e| e.name.starts_with(prefix)).collect();
        found.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(found.into_iter().map(Self::as_remote).collect())
    }

    async fn download(&self, id: &str) -> Result<Vec<u8>> {
        let r = check(self.agent.get(&self.url(id)).set("Authorization", &self.auth).call())?;
        let mut bytes = Vec::new();
        r.into_reader().take(MAX_DOWNLOAD).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> { self.put(name, bytes) }
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> { self.put(id, bytes) }

    async fn delete(&self, id: &str) -> Result<()> {
        match self.agent.delete(&self.url(id)).set("Authorization", &self.auth).call() {
            Ok(_) | Err(ureq::Error::Status(404, _)) => Ok(()),
            Err(e) => Err(check(Err(e)).unwrap_err()),
        }
    }

    async fn email(&self) -> Option<String> { Some(account_label(&self.user, &self.base)) }
}
