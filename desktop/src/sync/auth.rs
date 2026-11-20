//! Google sign-in for the desktop: your browser opens Google's own page, you pick the account,
//! and Google sends you back to a one-off address on this computer (127.0.0.1, a random port).
//! The OAuth itself (PKCE, the state check, token exchange, refresh and revocation) is the oauth2
//! crate's; this file only runs the one-off loopback page. Asks only for Memi's hidden app-data
//! folder in Drive.

use super::secrets::Secrets;
use anyhow::{bail, ensure, Context, Result};
use oauth2::basic::{BasicClient, BasicErrorResponseType};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet, PkceCodeChallenge, RedirectUrl,
    RefreshToken, RequestTokenError, RevocationUrl, Scope, StandardRevocableToken, TokenResponse, TokenUrl,
};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const REVOKE: &str = "https://oauth2.googleapis.com/revoke";
const REFRESH: &str = "google-refresh-token";
/// Google's "Desktop app" client for Memi. Google treats desktop client secrets as public.
const CLIENT_JSON: &str = include_str!("../../google-client.json");

type Client = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet, EndpointSet>;

/// The OAuth client (the oauth2 crate does PKCE, the state check and the token requests).
fn client(redirect: Option<&str>) -> Result<Client> {
    let v: Value = serde_json::from_str(CLIENT_JSON)?;
    let o = v.get("installed").or_else(|| v.get("web")).unwrap_or(&v);
    let id = o.get("client_id").and_then(Value::as_str).context("client_id")?;
    let secret = o.get("client_secret").and_then(Value::as_str).context("client_secret")?;
    let c = BasicClient::new(ClientId::new(id.into()))
        .set_client_secret(ClientSecret::new(secret.into()))
        .set_auth_uri(AuthUrl::new(AUTH.into())?)
        .set_token_uri(TokenUrl::new(TOKEN.into())?)
        .set_revocation_url(RevocationUrl::new(REVOKE.into())?);
    Ok(match redirect { Some(r) => c.set_redirect_uri(RedirectUrl::new(r.into())?), None => c })
}

/// HTTPS for the token requests. No redirects, as the oauth2 crate recommends.
fn http() -> ureq::Agent {
    ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_secs(30)).build()
}

pub struct Auth {
    access: Mutex<Option<(String, Instant)>>,
}

/// The query of a request the browser sent to the loopback address.
fn query(path: &str) -> std::collections::HashMap<String, String> {
    url::Url::parse(&format!("http://127.0.0.1{path}"))
        .map(|u| u.query_pairs().into_owned().collect())
        .unwrap_or_default()
}

const PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>Memi</title>
<body style="font-family:system-ui,sans-serif;background:#F4F3F8;color:#1C1B2E;display:grid;place-items:center;height:90vh;margin:0">
<div style="text-align:center"><h2 style="margin:0 0 8px">%TITLE%</h2><p style="margin:0;color:#6B6880">You can close this tab and go back to Memi.</p></div>"#;

impl Auth {
    pub fn new() -> Self { Auth { access: Mutex::new(None) } }

    /// Opens the browser for sign-in and waits (up to five minutes) for Google to send the person back.
    pub fn sign_in(&self, secrets: &Secrets) -> Result<String> {
        let server = tiny_http::Server::http("127.0.0.1:0").map_err(|e| anyhow::anyhow!("{e}"))?;
        let port = server.server_addr().to_ip().context("port")?.port();
        let redirect = format!("http://127.0.0.1:{port}");
        let c = client(Some(&redirect))?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state) = c
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(SCOPE.into()))
            .set_pkce_challenge(challenge)
            .add_extra_param("access_type", "offline")
            .add_extra_param("prompt", "select_account consent")
            .url();
        webbrowser::open(url.as_str()).context("Couldn't open your web browser")?;

        let deadline = Instant::now() + Duration::from_secs(300);
        let code = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            ensure!(!left.is_zero(), "Sign-in timed out. Please try again.");
            let Some(req) = server.recv_timeout(left)? else { continue };
            let q = query(req.url());
            // Only the reply carrying our state counts; anything else (a favicon, a stray tab) is ignored.
            if q.get("state").map(String::as_str) != Some(state.secret().as_str()) {
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
        let res = c.exchange_code(AuthorizationCode::new(code)).set_pkce_verifier(verifier).request(&http())
            .map_err(|e| anyhow::anyhow!("Google sign-in failed ({e})"))?;
        if let Some(refresh) = res.refresh_token() { secrets.set(REFRESH, refresh.secret()); }
        Ok(self.remember(res.access_token().secret(), res.expires_in()))
    }

    /// A fresh access token from the saved sign-in, or None if you need to sign in again.
    pub fn silent_token(&self, secrets: &Secrets) -> Result<Option<String>> {
        if let Some((t, until)) = self.access.lock().unwrap().clone() {
            if Instant::now() + Duration::from_secs(60) < until { return Ok(Some(t)); }
        }
        let Some(refresh) = secrets.get(REFRESH) else { return Ok(None) };
        match client(None)?.exchange_refresh_token(&RefreshToken::new(refresh)).request(&http()) {
            Ok(res) => Ok(Some(self.remember(res.access_token().secret(), res.expires_in()))),
            Err(RequestTokenError::ServerResponse(r)) if *r.error() == BasicErrorResponseType::InvalidGrant => {
                // Revoked or expired: sign in again.
                secrets.delete(REFRESH);
                Ok(None)
            }
            Err(e) => bail!("Couldn't reach Google ({e})"),
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
            if let Ok(c) = client(None) {
                if let Ok(req) = c.revoke_token(StandardRevocableToken::RefreshToken(RefreshToken::new(refresh))) {
                    let _ = req.request(&http());
                }
            }
        }
        secrets.delete(REFRESH);
    }

    fn remember(&self, token: &str, expires_in: Option<Duration>) -> String {
        let until = Instant::now() + expires_in.unwrap_or(Duration::from_secs(3600));
        *self.access.lock().unwrap() = Some((token.to_string(), until));
        token.to_string()
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
