//! Memi in the browser: Leptos screens on memi-engine, the same engine the Android and desktop
//! apps use. Everything stays in this browser (IndexedDB) and, if you turn sync on, in your own
//! Google Drive app folder, encrypted.

mod drive;
mod webdav;
mod js;
mod platform;
mod state;
mod sync;
mod ui;

fn main() {
    leptos::mount::mount_to_body(ui::Root);
}
