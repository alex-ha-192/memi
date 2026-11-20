//! The few Google Drive calls sync needs. Everything lives in Drive's hidden app-data folder,
//! which only Memi can see: it doesn't show among your files, and Memi can't see them.

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";
/// Backups are a few megabytes; anything far bigger isn't ours.
const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;

#[derive(Debug)]
pub struct Unauthorized;
impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("Google sign-in expired") }
}
impl std::error::Error for Unauthorized {}

#[derive(Clone, Debug)]
pub struct RemoteFile {
    pub id: String,
    pub modified_time: Option<String>,
    pub name: String,
}

pub struct Drive {
    token: String,
    agent: ureq::Agent,
}

fn check(res: std::result::Result<ureq::Response, ureq::Error>) -> Result<ureq::Response> {
    match res {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(401, _)) => Err(Unauthorized.into()),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            bail!("Google Drive said {code}: {}", body.chars().take(200).collect::<String>())
        }
        Err(e) => Err(anyhow::anyhow!("Couldn't reach Google Drive ({e})")),
    }
}

fn parse_file(v: &Value) -> Option<RemoteFile> {
    Some(RemoteFile {
        id: v.get("id")?.as_str()?.to_string(),
        modified_time: v.get("modifiedTime").and_then(Value::as_str).map(str::to_string),
        name: v.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

impl Drive {
    pub fn new(token: &str) -> Self {
        let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(20)).timeout(Duration::from_secs(120)).build();
        Drive { token: token.to_string(), agent }
    }

    fn auth(&self) -> String { format!("Bearer {}", self.token) }

    pub fn find(&self, name: &str) -> Result<Option<RemoteFile>> {
        // Names are ours (fixed or generated from base64url), but quote-escape anyway.
        let q = format!("name='{}' and trashed=false", name.replace('\\', "\\\\").replace('\'', "\\'"));
        let r = check(self.agent.get(&format!("{API}/files")).set("Authorization", &self.auth())
            .query("spaces", "appDataFolder").query("q", &q)
            .query("fields", "files(id,modifiedTime,name)").query("orderBy", "modifiedTime desc").call())?;
        let v: Value = r.into_json()?;
        Ok(v.get("files").and_then(Value::as_array).and_then(|a| a.first()).and_then(parse_file))
    }

    /// Files whose name starts with `prefix` (join requests), newest first.
    pub fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        let q = format!("name contains '{}' and trashed=false", prefix.replace('\\', "\\\\").replace('\'', "\\'"));
        let r = check(self.agent.get(&format!("{API}/files")).set("Authorization", &self.auth())
            .query("spaces", "appDataFolder").query("q", &q).query("pageSize", "20")
            .query("fields", "files(id,modifiedTime,name)").query("orderBy", "modifiedTime desc").call())?;
        let v: Value = r.into_json()?;
        Ok(v.get("files").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_file).filter(|f| f.name.starts_with(prefix)).collect()).unwrap_or_default())
    }

    /// The email of the connected Google account, to show in settings.
    pub fn email(&self) -> Option<String> {
        let r = self.agent.get(&format!("{API}/about")).set("Authorization", &self.auth()).query("fields", "user(emailAddress)").call().ok()?;
        let v: Value = r.into_json().ok()?;
        v.get("user")?.get("emailAddress")?.as_str().map(str::to_string)
    }

    pub fn download(&self, id: &str) -> Result<Vec<u8>> {
        let r = check(self.agent.get(&format!("{API}/files/{}", urlenc(id))).set("Authorization", &self.auth()).query("alt", "media").call())?;
        let mut out = Vec::new();
        r.into_reader().take(MAX_DOWNLOAD + 1).read_to_end(&mut out)?;
        anyhow::ensure!((out.len() as u64) <= MAX_DOWNLOAD, "Backup is too large");
        Ok(out)
    }

    pub fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> {
        let boundary = format!("memi{}", uuid::Uuid::new_v4().simple());
        let meta = serde_json::json!({ "name": name, "parents": ["appDataFolder"] }).to_string();
        let mut body = Vec::with_capacity(bytes.len() + 512);
        body.extend_from_slice(format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n").as_bytes());
        body.extend_from_slice(format!("--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let r = check(self.agent.post(&format!("{UPLOAD}/files")).set("Authorization", &self.auth())
            .set("Content-Type", &format!("multipart/related; boundary={boundary}"))
            .query("uploadType", "multipart").query("fields", "id,modifiedTime,name").send_bytes(&body))?;
        parse_file(&r.into_json()?).context("Unexpected reply from Google Drive")
    }

    pub fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> {
        let r = check(self.agent.request("PATCH", &format!("{UPLOAD}/files/{}", urlenc(id))).set("Authorization", &self.auth())
            .set("Content-Type", "application/octet-stream")
            .query("uploadType", "media").query("fields", "id,modifiedTime,name").send_bytes(bytes))?;
        parse_file(&r.into_json()?).context("Unexpected reply from Google Drive")
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        check(self.agent.delete(&format!("{API}/files/{}", urlenc(id))).set("Authorization", &self.auth()).call())?;
        Ok(())
    }
}

/// File ids are alphanumeric, but never trust a server value inside a URL path.
fn urlenc(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect()
}
