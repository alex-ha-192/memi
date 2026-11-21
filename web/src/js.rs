//! Bindings to glue.js, with Rust-friendly wrappers.

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/src/glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = idbGet)]
    fn idb_get_raw(key: &str) -> js_sys::Promise;
    #[wasm_bindgen(js_name = idbSet)]
    fn idb_set_raw(key: &str, value: &JsValue) -> js_sys::Promise;
    #[wasm_bindgen(js_name = idbDelete)]
    fn idb_delete_raw(key: &str) -> js_sys::Promise;
    #[wasm_bindgen(js_name = persistStorage)]
    pub fn persist_storage();

    #[wasm_bindgen(js_name = loadGis)]
    fn load_gis_raw() -> js_sys::Promise;
    #[wasm_bindgen(js_name = gisReady)]
    pub fn gis_ready() -> bool;
    #[wasm_bindgen(js_name = gisRequest)]
    fn gis_request_raw(client_id: &str, hint: &str, prompt: &str) -> js_sys::Promise;
    #[wasm_bindgen(js_name = gisRevoke)]
    pub fn gis_revoke(token: &str);

    #[wasm_bindgen(js_name = cropPhoto)]
    fn crop_photo_raw(file: &web_sys::File) -> js_sys::Promise;

    #[wasm_bindgen(js_name = systemDark)]
    pub fn system_dark() -> bool;
    #[wasm_bindgen(js_name = onSystemDarkChange)]
    pub fn on_system_dark_change(callback: &Closure<dyn FnMut(bool)>);
    #[wasm_bindgen(js_name = onVisibilityChange)]
    pub fn on_visibility_change(callback: &Closure<dyn FnMut(bool)>);
    #[wasm_bindgen(js_name = isVisible)]
    pub fn is_visible() -> bool;
    #[wasm_bindgen(js_name = copyText)]
    pub fn copy_text(text: &str);
    #[wasm_bindgen(js_name = deviceLabel)]
    pub fn device_label() -> String;
}

fn err(e: JsValue) -> anyhow::Error {
    anyhow::anyhow!(e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

/// Bytes stored under `key`, if any.
pub async fn idb_get_bytes(key: &str) -> Option<Vec<u8>> {
    let v = JsFuture::from(idb_get_raw(key)).await.ok()?;
    if v.is_undefined() || v.is_null() { return None; }
    Some(js_sys::Uint8Array::new(&v).to_vec())
}

pub async fn idb_get_text(key: &str) -> Option<String> {
    JsFuture::from(idb_get_raw(key)).await.ok()?.as_string()
}

pub async fn idb_set_bytes(key: &str, bytes: &[u8]) -> anyhow::Result<()> {
    JsFuture::from(idb_set_raw(key, &js_sys::Uint8Array::from(bytes).into())).await.map_err(err)?;
    Ok(())
}

pub async fn idb_set_text(key: &str, text: &str) -> anyhow::Result<()> {
    JsFuture::from(idb_set_raw(key, &JsValue::from_str(text))).await.map_err(err)?;
    Ok(())
}

pub async fn idb_delete(key: &str) {
    let _ = JsFuture::from(idb_delete_raw(key)).await;
}

pub struct Token {
    pub value: String,
    pub expires_in: f64,
}

/// Asks Google for a Drive app-folder token. See glue.js `gisRequest`.
pub async fn gis_request(client_id: &str, hint: &str, prompt: &str) -> anyhow::Result<Token> {
    let v = JsFuture::from(gis_request_raw(client_id, hint, prompt)).await.map_err(err)?;
    let json: serde_json::Value = serde_json::from_str(&v.as_string().unwrap_or_default())?;
    Ok(Token {
        value: json["token"].as_str().unwrap_or_default().to_string(),
        expires_in: json["expiresIn"].as_f64().unwrap_or(3600.),
    })
}

/// Loads Google's sign-in script (see glue.js `loadGis`).
pub async fn load_gis() -> bool {
    JsFuture::from(load_gis_raw()).await.is_ok()
}

pub async fn crop_photo(file: &web_sys::File) -> anyhow::Result<Vec<u8>> {
    let v = JsFuture::from(crop_photo_raw(file)).await.map_err(err)?;
    Ok(js_sys::Uint8Array::new(&v).to_vec())
}

/// A URL the page can show `bytes` (a JPEG) from.
pub fn blob_url(bytes: &[u8]) -> Option<String> {
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("image/jpeg");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
}

/// Saves `bytes` as a download called `name`.
pub fn download(name: &str, bytes: &[u8], mime: &str) -> bool {
    use wasm_bindgen::JsCast;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else { return false };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return false };
    let doc = web_sys::window().and_then(|w| w.document());
    let Some(a) = doc.and_then(|d| d.create_element("a").ok()).and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok()) else { return false };
    a.set_href(&url);
    a.set_download(name);
    a.click();
    // Give the browser a moment to start the download before letting go of the data.
    let u = url.clone();
    wasm_bindgen_futures::spawn_local(async move { gloo_timers::future::TimeoutFuture::new(30_000).await; revoke_url(&u) });
    true
}

/// The contents of a file the person picked.
pub async fn file_bytes(f: &web_sys::File) -> anyhow::Result<Vec<u8>> {
    let buf = wasm_bindgen_futures::JsFuture::from(f.array_buffer()).await.map_err(|_| anyhow::anyhow!("couldn't read the file"))?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

pub fn revoke_url(url: &str) {
    let _ = web_sys::Url::revoke_object_url(url);
}

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}
