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

/**
 * Lets the person place a picture in the 4:5 frame before it's kept, as the apps do: drag it with
 * a finger or the mouse, zoom with the slider (or pinch, or the mouse wheel). Resolves to the
 * cropped JPEG bytes, or null if they cancel.
 */
export async function cropInteractive(file) {
  const bitmap = await createImageBitmap(file);
  const url = URL.createObjectURL(file);
  const FW = 260, FH = 325; // the frame, 4:5
  const cover = Math.max(FW / bitmap.width, FH / bitmap.height);
  let zoom = 1, ox = 0, oy = 0; // the picture's offset from centred, in frame pixels

  const el = (tag, cls, parent) => { const e = document.createElement(tag); if (cls) e.className = cls; if (parent) parent.appendChild(e); return e; };
  const scrim = el("div", "scrim crop-scrim", document.body);
  const box = el("div", "crop-box", document.body);
  box.setAttribute("role", "dialog");
  box.setAttribute("aria-label", "Place the photo");
  el("h2", "", box).textContent = "Place the photo";
  el("p", "caption muted", box).textContent = "Drag it into place, and zoom with the slider.";
  const frame = el("div", "crop-frame", box);
  const img = el("img", "", frame);
  img.src = url; img.alt = ""; img.draggable = false;
  const zoomRow = el("label", "crop-zoom", box);
  el("span", "caption muted", zoomRow).textContent = "Zoom";
  const slider = el("input", "", zoomRow);
  Object.assign(slider, { type: "range", min: "1", max: "4", step: "0.01", value: "1" });
  const buttons = el("div", "row gap", box);
  const cancel = el("button", "button grow", buttons); cancel.textContent = "Cancel";
  const done = el("button", "button ink grow", buttons); done.textContent = "Use photo";

  const scale = () => cover * zoom;
  const clamp = () => {
    const w = bitmap.width * scale(), h = bitmap.height * scale();
    const mx = Math.max(0, (w - FW) / 2), my = Math.max(0, (h - FH) / 2);
    ox = Math.min(mx, Math.max(-mx, ox)); oy = Math.min(my, Math.max(-my, oy));
  };
  const draw = () => {
    clamp();
    const w = bitmap.width * scale(), h = bitmap.height * scale();
    img.style.width = w + "px"; img.style.height = h + "px";
    img.style.left = (FW - w) / 2 + ox + "px"; img.style.top = (FH - h) / 2 + oy + "px";
  };
  const setZoom = (z) => { zoom = Math.min(4, Math.max(1, z)); slider.value = String(zoom); draw(); };
  draw();

  // Dragging and pinching, with any pointer.
  const pointers = new Map();
  let pinch = null;
  frame.addEventListener("pointerdown", (e) => { frame.setPointerCapture(e.pointerId); pointers.set(e.pointerId, [e.clientX, e.clientY]); e.preventDefault(); });
  frame.addEventListener("pointermove", (e) => {
    const last = pointers.get(e.pointerId);
    if (!last) return;
    pointers.set(e.pointerId, [e.clientX, e.clientY]);
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      const d = Math.hypot(a[0] - b[0], a[1] - b[1]);
      if (pinch) setZoom(zoom * d / pinch);
      pinch = d;
    } else {
      ox += e.clientX - last[0]; oy += e.clientY - last[1]; draw();
    }
  });
  const up = (e) => { pointers.delete(e.pointerId); if (pointers.size < 2) pinch = null; };
  frame.addEventListener("pointerup", up);
  frame.addEventListener("pointercancel", up);
  frame.addEventListener("wheel", (e) => { e.preventDefault(); setZoom(zoom * (e.deltaY < 0 ? 1.08 : 1 / 1.08)); }, { passive: false });
  slider.addEventListener("input", () => setZoom(Number(slider.value)));
  // Arrow keys move it, for keyboards.
  frame.tabIndex = 0;
  frame.setAttribute("aria-label", "Photo position: arrow keys move it");
  frame.addEventListener("keydown", (e) => {
    const step = 10, moves = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
    if (moves[e.key]) { ox += moves[e.key][0]; oy += moves[e.key][1]; draw(); e.preventDefault(); }
  });

  const result = await new Promise((resolve) => {
    const close = (v) => { scrim.remove(); box.remove(); URL.revokeObjectURL(url); document.removeEventListener("keydown", esc, true); resolve(v); };
    const esc = (e) => { if (e.key === "Escape") { e.stopPropagation(); close(null); } };
    document.addEventListener("keydown", esc, true);
    cancel.onclick = () => close(null);
    scrim.onclick = () => close(null);
    done.onclick = () => {
      // The frame's area, in the picture's own pixels.
      const s = scale(), w = bitmap.width * s, h = bitmap.height * s;
      const left = (FW - w) / 2 + ox, top = (FH - h) / 2 + oy;
      close({ x: -left / s, y: -top / s, w: FW / s, h: FH / s });
    };
    done.focus();
  });
  if (!result) return null;
  const outW = Math.min(1080, Math.round(result.w)), outH = Math.round(outW * 5 / 4);
  const canvas = document.createElement("canvas");
  canvas.width = outW; canvas.height = outH;
  canvas.getContext("2d").drawImage(bitmap, result.x, result.y, result.w, result.h, 0, 0, outW, outH);
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

// --- Birthday reminders --------------------------------------------------------------------------

export function notificationsAllowed() {
  return typeof Notification !== "undefined" && Notification.permission === "granted";
}

/** Asks to show notifications (call from a tap). Resolves to true if allowed. */
export async function askNotifications() {
  if (typeof Notification === "undefined") return false;
  if (Notification.permission === "granted") return true;
  try { return (await Notification.requestPermission()) === "granted"; } catch (_) { return false; }
}

/** Shows a notification: through the offline worker where there is one (Chrome on Android
 * allows no other way), otherwise directly. */
export function notify(title, body) {
  const options = { body, icon: "/memi.png", badge: "/memi.png" };
  if (navigator.serviceWorker && navigator.serviceWorker.controller) {
    navigator.serviceWorker.ready.then((r) => r.showNotification(title, options)).catch(() => {});
  } else {
    try { new Notification(title, options); } catch (_) {}
  }
}

export function copyText(text) {
  if (navigator.clipboard) navigator.clipboard.writeText(text).catch(() => {});
}

export function deviceLabel() {
  const ua = navigator.userAgent;
  const browser = /Edg\//.test(ua) ? "Edge" : /Firefox\//.test(ua) ? "Firefox" : /Chrome\//.test(ua) ? "Chrome" : /Safari\//.test(ua) ? "Safari" : "A browser";
  const os = /Android/.test(ua) ? "Android" : /iPhone|iPad/.test(ua) ? "iOS" : /Windows/.test(ua) ? "Windows" : /Mac OS/.test(ua) ? "Mac" : /Linux/.test(ua) ? "Linux" : "";
  return os ? `${browser} on ${os}` : browser;
}
