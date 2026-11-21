//! WebDAV from the browser: the same calls as the native client (engine/src/webdav.rs), made with
//! fetch. The server has to allow requests from memi.yourmemi.app (CORS); Sync says so.

use anyhow::{anyhow, bail, Result};
use memi_engine::dav::{self, Entry};
use memi_engine::sync::{RemoteFile, Unauthorized};
use std::cell::Cell;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

pub struct DavDrive {
    base: String,
    user: String,
    auth: String,
    folder_ready: Cell<bool>,
}

impl DavDrive {
    pub fn new(url: &str, user: &str, password: &str) -> Result<Self> {
        let base = dav::normalise_url(url)?;
        if !base.to_ascii_lowercase().starts_with("https://") { bail!("In a browser, the address needs to start with https://"); }
        use base64::Engine;
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
        Ok(DavDrive { base, user: user.to_string(), auth: format!("Basic {token}"), folder_ready: Cell::new(false) })
    }

    fn url(&self, name: &str) -> String { format!("{}{}", self.base, dav::percent_encode(name)) }

    /// Returns the status and body.
    async fn request(&self, method: &str, url: &str, depth: Option<&str>, body: Option<&[u8]>) -> Result<(u16, Vec<u8>)> {
        let init = web_sys::RequestInit::new();
        init.set_method(method);
        let headers = web_sys::Headers::new().map_err(|_| anyhow!("headers"))?;
        headers.set("Authorization", &self.auth).map_err(|_| anyhow!("headers"))?;
        if let Some(d) = depth {
            headers.set("Depth", d).map_err(|_| anyhow!("headers"))?;
            headers.set("Content-Type", "application/xml; charset=utf-8").map_err(|_| anyhow!("headers"))?;
            init.set_body(&dav::PROPFIND_BODY.into());
        } else if let Some(bytes) = body {
            headers.set("Content-Type", "application/octet-stream").map_err(|_| anyhow!("headers"))?;
            init.set_body(&js_sys::Uint8Array::from(bytes).into());
        }
        init.set_headers(&headers.into());
        let req = web_sys::Request::new_with_str_and_init(url, &init).map_err(|_| anyhow!("Bad request"))?;
        let window = web_sys::window().ok_or_else(|| anyhow!("No window"))?;
        let res: web_sys::Response = JsFuture::from(window.fetch_with_request(&req)).await
            .map_err(|_| anyhow!("Couldn't reach the WebDAV server. Check your connection, and that the server allows this site (CORS)."))?
            .dyn_into().map_err(|_| anyhow!("Bad response"))?;
        let buf = JsFuture::from(res.array_buffer().map_err(|_| anyhow!("Bad response"))?).await.map_err(|_| anyhow!("Bad response"))?;
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        match res.status() {
            401 => Err(Unauthorized.into()),
            code => Ok((code, bytes)),
        }
    }

    fn ok(code: u16, bytes: &[u8]) -> Result<()> {
        if (200..300).contains(&code) { Ok(()) } else {
            bail!("The WebDAV server said {code}: {}", String::from_utf8_lossy(bytes).chars().take(200).collect::<String>())
        }
    }

    async fn ensure_folder(&self) -> Result<()> {
        if self.folder_ready.get() { return Ok(()); }
        let (code, body) = self.request("PROPFIND", &self.base, Some("0"), None).await?;
        if code == 404 {
            let (code, body) = self.request("MKCOL", &self.base, None, None).await?;
            Self::ok(code, &body)?;
        } else {
            Self::ok(code, &body)?;
        }
        self.folder_ready.set(true);
        Ok(())
    }

    fn as_remote(e: Entry) -> RemoteFile {
        RemoteFile { id: e.name.clone(), modified_time: e.etag.or(Some(e.modified.to_string())), name: e.name }
    }

    async fn put(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> {
        self.ensure_folder().await?;
        let (code, body) = self.request("PUT", &self.url(name), None, Some(bytes)).await?;
        Self::ok(code, &body)?;
        self.find_impl(name).await?.ok_or_else(|| anyhow!("The WebDAV server didn't keep the file"))
    }

    async fn find_impl(&self, name: &str) -> Result<Option<RemoteFile>> {
        self.ensure_folder().await?;
        let (code, body) = self.request("PROPFIND", &self.url(name), Some("0"), None).await?;
        if code == 404 { return Ok(None); }
        Self::ok(code, &body)?;
        let xml = String::from_utf8_lossy(&body);
        Ok(dav::parse_propfind(&xml).into_iter().find(|e| !e.folder).map(|mut e| { e.name = name.to_string(); Self::as_remote(e) }))
    }
}

impl memi_engine::sync::Drive for DavDrive {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>> { self.find_impl(name).await }

    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        self.ensure_folder().await?;
        let (code, body) = self.request("PROPFIND", &self.base, Some("1"), None).await?;
        Self::ok(code, &body)?;
        let xml = String::from_utf8_lossy(&body);
        let mut found: Vec<Entry> = dav::parse_propfind(&xml).into_iter().filter(|e| !e.folder && e.name.starts_with(prefix)).collect();
        found.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(found.into_iter().map(Self::as_remote).collect())
    }

    async fn download(&self, id: &str) -> Result<Vec<u8>> {
        let (code, body) = self.request("GET", &self.url(id), None, None).await?;
        Self::ok(code, &body)?;
        Ok(body)
    }

    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> { self.put(name, bytes).await }
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> { self.put(id, bytes).await }

    async fn delete(&self, id: &str) -> Result<()> {
        let (code, body) = self.request("DELETE", &self.url(id), None, None).await?;
        if code == 404 { Ok(()) } else { Self::ok(code, &body) }
    }

    async fn email(&self) -> Option<String> { Some(dav::account_label(&self.user, &self.base)) }
}

/// Whichever place sync uses in the browser.
pub enum WebDrive {
    Google(crate::drive::Drive),
    Dav(DavDrive),
}

impl memi_engine::sync::Drive for WebDrive {
    async fn find(&self, name: &str) -> Result<Option<RemoteFile>> { match self { WebDrive::Google(d) => d.find(name).await, WebDrive::Dav(d) => d.find(name).await } }
    async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> { match self { WebDrive::Google(d) => d.list(prefix).await, WebDrive::Dav(d) => d.list(prefix).await } }
    async fn download(&self, id: &str) -> Result<Vec<u8>> { match self { WebDrive::Google(d) => d.download(id).await, WebDrive::Dav(d) => d.download(id).await } }
    async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> { match self { WebDrive::Google(d) => d.create(name, bytes).await, WebDrive::Dav(d) => d.create(name, bytes).await } }
    async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> { match self { WebDrive::Google(d) => d.update(id, bytes).await, WebDrive::Dav(d) => d.update(id, bytes).await } }
    async fn delete(&self, id: &str) -> Result<()> { match self { WebDrive::Google(d) => d.delete(id).await, WebDrive::Dav(d) => d.delete(id).await } }
    async fn email(&self) -> Option<String> { match self { WebDrive::Google(d) => d.email().await, WebDrive::Dav(d) => d.email().await } }
}
