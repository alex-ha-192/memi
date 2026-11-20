//! The few Google Drive calls Memi needs, from the browser. Everything lives in the hidden app
//! data folder, which only Memi can see: it doesn't show up in your Drive and Memi can't see your
//! other files. The same calls as the desktop's sync/drive.rs and Android's DriveClient.kt.

use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";

#[derive(Clone, Debug)]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub modified_time: Option<String>,
}

/// Google said the sign-in has expired or was removed.
#[derive(Debug)]
pub struct Unauthorized;
impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Google sign-in expired") }
}
impl std::error::Error for Unauthorized {}

pub struct Drive {
    token: String,
}

fn enc(s: &str) -> String { js_sys::encode_uri_component(s).as_string().unwrap_or_default() }

fn parse_file(v: &Value) -> Option<RemoteFile> {
    Some(RemoteFile {
        id: v.get("id")?.as_str()?.to_string(),
        name: v.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
        modified_time: v.get("modifiedTime").and_then(Value::as_str).map(str::to_string),
    })
}

impl Drive {
    pub fn new(token: &str) -> Self { Drive { token: token.to_string() } }

    async fn request(&self, method: &str, url: &str, body: Option<(&[u8], &str)>) -> Result<Vec<u8>> {
        let init = web_sys::RequestInit::new();
        init.set_method(method);
        let headers = web_sys::Headers::new().map_err(|_| anyhow!("headers"))?;
        headers.set("Authorization", &format!("Bearer {}", self.token)).map_err(|_| anyhow!("headers"))?;
        if let Some((bytes, kind)) = body {
            headers.set("Content-Type", kind).map_err(|_| anyhow!("headers"))?;
            init.set_body(&js_sys::Uint8Array::from(bytes).into());
        }
        init.set_headers(&headers.into());
        let req = web_sys::Request::new_with_str_and_init(url, &init).map_err(|_| anyhow!("Bad request"))?;
        let window = web_sys::window().ok_or_else(|| anyhow!("No window"))?;
        let res: web_sys::Response = JsFuture::from(window.fetch_with_request(&req)).await
            .map_err(|_| anyhow!("Couldn't reach Google Drive. Check your connection."))?
            .dyn_into().map_err(|_| anyhow!("Bad response"))?;
        let buf = JsFuture::from(res.array_buffer().map_err(|_| anyhow!("Bad response"))?).await.map_err(|_| anyhow!("Bad response"))?;
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        match res.status() {
            200..=299 => Ok(bytes),
            401 => Err(Unauthorized.into()),
            code => bail!("Google Drive said {code}: {}", String::from_utf8_lossy(&bytes).chars().take(200).collect::<String>()),
        }
    }

    async fn files(&self, q: &str) -> Result<Vec<RemoteFile>> {
        let url = format!("{API}/files?spaces=appDataFolder&pageSize=50&orderBy=modifiedTime%20desc&fields=files(id,name,modifiedTime)&q={}", enc(q));
        let v: Value = serde_json::from_slice(&self.request("GET", &url, None).await?)?;
        Ok(v.get("files").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_file).collect()).unwrap_or_default())
    }

    pub async fn find(&self, name: &str) -> Result<Option<RemoteFile>> {
        Ok(self.files(&format!("name='{name}' and trashed=false")).await?.into_iter().next())
    }

    /// Every file whose name starts with `prefix`, newest first.
    pub async fn list(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        Ok(self.files(&format!("name contains '{prefix}' and trashed=false")).await?.into_iter().filter(|f| f.name.starts_with(prefix)).collect())
    }

    pub async fn download(&self, id: &str) -> Result<Vec<u8>> {
        self.request("GET", &format!("{API}/files/{id}?alt=media"), None).await
    }

    pub async fn create(&self, name: &str, bytes: &[u8]) -> Result<RemoteFile> {
        let boundary = format!("memi{}", js_sys::Date::now() as u64);
        let meta = serde_json::json!({ "name": name, "parents": ["appDataFolder"] }).to_string();
        let mut body = format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let res = self.request("POST", &format!("{UPLOAD}/files?uploadType=multipart&fields=id,name,modifiedTime"), Some((&body, &format!("multipart/related; boundary={boundary}")))).await?;
        parse_file(&serde_json::from_slice(&res)?).ok_or_else(|| anyhow!("Unexpected answer from Google Drive"))
    }

    pub async fn update(&self, id: &str, bytes: &[u8]) -> Result<RemoteFile> {
        let res = self.request("PATCH", &format!("{UPLOAD}/files/{id}?uploadType=media&fields=id,name,modifiedTime"), Some((bytes, "application/octet-stream"))).await?;
        parse_file(&serde_json::from_slice(&res)?).ok_or_else(|| anyhow!("Unexpected answer from Google Drive"))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        self.request("DELETE", &format!("{API}/files/{id}"), None).await.map(|_| ())
    }

    /// The email of the connected Google account, to show in Sync.
    pub async fn email(&self) -> Option<String> {
        let v: Value = serde_json::from_slice(&self.request("GET", &format!("{API}/about?fields=user(emailAddress)"), None).await.ok()?).ok()?;
        v.pointer("/user/emailAddress").and_then(Value::as_str).map(str::to_string)
    }
}
