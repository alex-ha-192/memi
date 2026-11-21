// The few browser pieces that are simplest in JavaScript: IndexedDB, Google sign-in (Google
// Identity Services, loaded by index.html), and resizing a photo with a canvas.

// --- IndexedDB: everything Memi keeps in this browser, including the sync key -------------------

let dbPromise = null;

function db() {
  if (!dbPromise) {
    dbPromise = new Promise((resolve, reject) => {
      const req = indexedDB.open("memi", 1);
      req.onupgradeneeded = () => req.result.createObjectStore("kv");
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }
  return dbPromise;
}

function tx(mode, run) {
  return db().then((d) => new Promise((resolve, reject) => {
    const t = d.transaction("kv", mode);
    const req = run(t.objectStore("kv"));
    t.oncomplete = () => resolve(req ? req.result : undefined);
    t.onerror = () => reject(t.error);
  }));
}

export function idbGet(key) { return tx("readonly", (s) => s.get(key)); }
export function idbSet(key, value) { return tx("readwrite", (s) => s.put(value, key)); }
export function idbDelete(key) { return tx("readwrite", (s) => s.delete(key)); }

/** Asks the browser not to clear Memi's storage when space runs low. */
export function persistStorage() {
  if (navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(() => {});
}

// --- Google sign-in -----------------------------------------------------------------------------

let gisLoading = null;

/** Loads Google's sign-in script, only once you open Sync or sync is already on. */
export function loadGis() {
  if (gisReady()) return Promise.resolve();
  if (!gisLoading) {
    gisLoading = new Promise((resolve, reject) => {
      const s = document.createElement("script");
      s.src = "https://accounts.google.com/gsi/client";
      s.async = true;
      s.onload = () => resolve();
      s.onerror = () => { gisLoading = null; reject("Couldn't load Google sign-in. Check your connection."); };
      document.head.appendChild(s);
    });
  }
  return gisLoading;
}

export function gisReady() {
  return !!(window.google && google.accounts && google.accounts.oauth2);
}

/**
 * An access token for the Drive app folder only. `prompt` is "" to reuse an earlier consent
 * (no screen if Google allows it) or "select_account" to pick an account. Resolves to
 * JSON {token, expiresIn}.
 */
export function gisRequest(clientId, hint, prompt) {
  return new Promise((resolve, reject) => {
    if (!gisReady()) { reject("Google sign-in didn't load. Check your connection and try again."); return; }
    const client = google.accounts.oauth2.initTokenClient({
      client_id: clientId,
      scope: "https://www.googleapis.com/auth/drive.appdata",
      login_hint: hint || undefined,
      callback: (r) => {
        if (r.error) reject(r.error_description || r.error);
        else resolve(JSON.stringify({ token: r.access_token, expiresIn: Number(r.expires_in) || 3600 }));
      },
      error_callback: (e) => reject((e && e.type) || "closed"),
    });
    client.requestAccessToken({ prompt });
  });
}

export function gisRevoke(token) {
  try { google.accounts.oauth2.revoke(token, () => {}); } catch (_) {}
}

// --- Photos -------------------------------------------------------------------------------------

/** The middle of a picture at 4:5, at most 1080 wide, as JPEG bytes (the same as the apps keep). */
export async function cropPhoto(file) {
  const bitmap = await createImageBitmap(file);
  const want = 4 / 5;
  let w = bitmap.width, h = bitmap.height, x = 0, y = 0;
  if (w / h > want) { const nw = Math.round(h * want); x = Math.round((w - nw) / 2); w = nw; }
  else { const nh = Math.round(w / want); y = Math.round((h - nh) / 2); h = nh; }
  const outW = Math.min(1080, w), outH = Math.round(outW / want);
  const canvas = document.createElement("canvas");
  canvas.width = outW; canvas.height = outH;
  canvas.getContext("2d").drawImage(bitmap, x, y, w, h, 0, 0, outW, outH);
  const blob = await new Promise((r) => canvas.toBlob(r, "image/jpeg", 0.88));
  return new Uint8Array(await blob.arrayBuffer());
}

// --- Small things -------------------------------------------------------------------------------

export function systemDark() {
  return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function onSystemDarkChange(callback) {
  if (window.matchMedia) window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => callback(e.matches));
}

export function onVisibilityChange(callback) {
  document.addEventListener("visibilitychange", () => callback(document.visibilityState === "visible"));
}

export function isVisible() { return document.visibilityState === "visible"; }

export function copyText(text) {
  if (navigator.clipboard) navigator.clipboard.writeText(text).catch(() => {});
}

export function deviceLabel() {
  const ua = navigator.userAgent;
  const browser = /Edg\//.test(ua) ? "Edge" : /Firefox\//.test(ua) ? "Firefox" : /Chrome\//.test(ua) ? "Chrome" : /Safari\//.test(ua) ? "Safari" : "A browser";
  const os = /Android/.test(ua) ? "Android" : /iPhone|iPad/.test(ua) ? "iOS" : /Windows/.test(ua) ? "Windows" : /Mac OS/.test(ua) ? "Mac" : /Linux/.test(ua) ? "Linux" : "";
  return os ? `${browser} on ${os}` : browser;
}
