//! Memi in the browser. The same data, merge rules, encryption and sync as the Android and desktop
//! apps (memi-core); everything stays in this browser (IndexedDB) and, if you turn sync on, in your
//! own Google Drive app folder, encrypted.

mod data;
mod drive;
mod js;
mod state;
mod sync;
mod ui;

fn main() {
    leptos::mount::mount_to_body(ui::Root);
}
