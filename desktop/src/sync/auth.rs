//! Google sign-in for the desktop: your browser opens Google's own page, you pick the account,
//! and Google sends you back to a one-off address on this computer (127.0.0.1, a random port).
//! Uses PKCE and asks only for Your Memi's hidden app-data folder in Drive.

use super::secrets::Secrets;
use anyhow::{bail, ensure, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const REFRESH: &str = "google-refresh-token";
/// Google's "Desktop app" client for Your Memi. Google treats desktop client secrets as public.
const CLIENT_JSON: &str = include_str!("../../google-client.json");

struct Client { id: String, secret: String }

fn client() -> Result<Client> {
    let v: Value = serde_json::from_str(CLIENT_JSON)?;
    let o = v.get("installed").or_else(|| v.get("web")).unwrap_or(&v);
    Ok(Client {
        id: o.get("client_id").and_then(Value::as_str).context("client_id")?.to_string(),
        secret: o.get("client_secret").and_then(Value::as_str).context("client_secret")?.to_string(),
    })
}

pub struct Auth {
    access: Mutex<Option<(String, Instant)>>,
}

fn random_token(n: usize) -> String {
    let mut b = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

fn form(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| format!("{k}={}", urlencode(v))).collect::<Vec<_>>().join("&")
}

fn urlencode(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1; }
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => { out.push(v); i += 3; }
                    Err(_) => { out.push(b'%'); i += 1; }
                }
            }
            c => { out.push(c); i += 1; }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query(url: &str) -> std::collections::HashMap<String, String> {
    url.split_once('?').map(|(_, q)| q).unwrap_or("").split('&').filter_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        Some((urldecode(k), urldecode(v)))
    }).collect()
}

const PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>Your Memi</title>
<body style="font-family:system-ui,sans-serif;background:#F4F3F8;color:#1C1B2E;display:grid;place-items:center;height:90vh;margin:0">
<div style="text-align:center"><h2 style="margin:0 0 8px">%TITLE%</h2><p style="margin:0;color:#6B6880">You can close this tab and go back to Your Memi.</p></div>"#;

impl Auth {
    pub fn new() -> Self { Auth { access: Mutex::new(None) } }

    /// Opens the browser for sign-in and waits (up to five minutes) for Google to send the person back.
    pub fn sign_in(&self, secrets: &Secrets) -> Result<String> {
        let c = client()?;
        let verifier = random_token(48);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_token(16);
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(|e| anyhow::anyhow!("{e}"))?;
        let port = server.server_addr().to_ip().context("port")?.port();
        let redirect = format!("http://127.0.0.1:{port}");
        let url = format!("{AUTH}?{}", form(&[
            ("client_id", &c.id), ("redirect_uri", &redirect), ("response_type", "code"), ("scope", SCOPE),
            ("code_challenge", &challenge), ("code_challenge_method", "S256"), ("state", &state),
            ("access_type", "offline"), ("prompt", "select_account consent"),
        ]));
        webbrowser::open(&url).context("Couldn't open your web browser")?;

        let deadline = Instant::now() + Duration::from_secs(300);
        let code = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            ensure!(!left.is_zero(), "Sign-in timed out. Please try again.");
            let Some(req) = server.recv_timeout(left)? else { continue };
            let q = query(req.url());
            // Only the reply carrying our state counts; anything else (a favicon, a stray tab) is ignored.
            if q.get("state") != Some(&state) {
                let _ = req.respond(tiny_http::Response::empty(404));
                continue;
            }
            let ok = q.contains_key("code");
            let page = PAGE.replace("%TITLE%", if ok { "You're signed in" } else { "Sign-in didn't finish" });
            let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap();
            let _ = req.respond(tiny_http::Response::from_string(page).with_header(header));
            match q.get("code") {
                Some(code) => break code.clone(),
                None => bail!("Sign-in was cancelled."),
            }
        };
        let res = post(&[
            ("code", &code), ("client_id", &c.id), ("client_secret", &c.secret),
            ("redirect_uri", &redirect), ("grant_type", "authorization_code"), ("code_verifier", &verifier),
        ])?;
        if let Some(refresh) = res.get("refresh_token").and_then(Value::as_str) {
            secrets.set(REFRESH, refresh);
        }
        self.remember(&res)
    }

    /// A fresh access token from the saved sign-in, or None if you need to sign in again.
    pub fn silent_token(&self, secrets: &Secrets) -> Result<Option<String>> {
        if let Some((t, until)) = self.access.lock().unwrap().clone() {
            if Instant::now() + Duration::from_secs(60) < until { return Ok(Some(t)); }
        }
        let Some(refresh) = secrets.get(REFRESH) else { return Ok(None) };
        let c = client()?;
        match post(&[("client_id", &c.id), ("client_secret", &c.secret), ("refresh_token", &refresh), ("grant_type", "refresh_token")]) {
            Ok(res) => Ok(Some(self.remember(&res)?)),
            Err(e) if e.to_string().contains("invalid_grant") => {
                secrets.delete(REFRESH);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    pub fn forget(&self, token: &str) {
        let mut a = self.access.lock().unwrap();
        if a.as_ref().map(|(t, _)| t == token).unwrap_or(false) { *a = None; }
    }

    pub fn sign_out(&self, secrets: &Secrets) {
        *self.access.lock().unwrap() = None;
        if let Some(refresh) = secrets.get(REFRESH) {
            // Tell Google to revoke it too, so the sign-in can't be used again (best effort).
            let _ = ureq::post("https://oauth2.googleapis.com/revoke").timeout(Duration::from_secs(10))
                .set("Content-Type", "application/x-www-form-urlencoded").send_string(&form(&[("token", &refresh)]));
        }
        secrets.delete(REFRESH);
    }

    fn remember(&self, res: &Value) -> Result<String> {
        let token = res.get("access_token").and_then(Value::as_str).context("No access token from Google")?.to_string();
        let secs = res.get("expires_in").and_then(Value::as_u64).unwrap_or(3600);
        *self.access.lock().unwrap() = Some((token.clone(), Instant::now() + Duration::from_secs(secs)));
        Ok(token)
    }
}

fn post(fields: &[(&str, &str)]) -> Result<Value> {
    match ureq::post(TOKEN).timeout(Duration::from_secs(30)).set("Content-Type", "application/x-www-form-urlencoded").send_string(&form(fields)) {
        Ok(r) => Ok(r.into_json()?),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            if body.contains("invalid_grant") { bail!("invalid_grant") }
            bail!("Google sign-in failed ({code})")
        }
        Err(e) => bail!("Couldn't reach Google ({e})"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn query_parsing() {
        let q = super::query("/?state=a%2Bb&code=4%2F0Ab-cd&scope=x+y");
        assert_eq!(q["state"], "a+b");
        assert_eq!(q["code"], "4/0Ab-cd");
        assert_eq!(q["scope"], "x y");
    }
}
