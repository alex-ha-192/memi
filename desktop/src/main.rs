//! Memi for Windows and Linux.
//!
//! Everything lives in one folder on this computer (see `data_dir`). Nothing is sent anywhere
//! unless you turn on sync, which uploads one end-to-end encrypted file to your own Google
//! Drive. There's no analytics, no crash reporting and no logging to disk.

// No console window on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

// The data model, date rules and Home insights are shared with the web app.
pub use memi_core::{dates, insights, model};
mod photos;
mod platform;
mod sync;
mod ui;

use gpui::*;
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ui::app::{Back, Dirs, Find, MemiView, NewPerson};

pub const APP_ID: &str = "app.yourmemi.YourMemi";

/// ~/.local/share/yourmemi on Linux, %APPDATA%\YourMemi on Windows.
fn data_dir() -> std::path::PathBuf {
    let dirs = directories::ProjectDirs::from("app", "", "YourMemi").expect("a home folder");
    dirs.data_dir().to_path_buf()
}

fn main() {
    let data = data_dir();
    std::fs::create_dir_all(&data).expect("Couldn't create Memi's data folder");
    let photo_dir = data.join("photos");
    let _ = std::fs::create_dir_all(&photo_dir);
    // Everything below the screens is memi-engine; this computer's storage is its SQLite store.
    let store = memi_engine::sqlite::SqliteStore::open(&data.join("memi.db")).expect("Couldn't open Memi's data");
    let snapshot = store.load();
    let secrets = Arc::new(sync::secrets::Secrets::new(&data));
    let platform = platform::DesktopPlatform::new(store, photo_dir.clone(), &data, secrets.clone());
    let memi: sync::Shared = Arc::new(Mutex::new(memi_engine::Memi::new(snapshot, platform::DesktopPlatform::load_sync(&data), platform)));
    let engine = Arc::new(sync::Engine::new(memi.clone(), secrets));

    gpui_platform::application().with_assets(ui::assets::Assets).run(move |cx: &mut App| {
        gpui_component::init(cx);
        let fonts: Vec<Cow<'static, [u8]>> = ui::assets::FONTS.iter().map(|f| Cow::Borrowed(*f)).collect();
        let _ = cx.text_system().add_fonts(fonts);
        cx.bind_keys([
            KeyBinding::new("escape", Back, Some("Memi")),
            KeyBinding::new("ctrl-n", NewPerson, Some("Memi")),
            KeyBinding::new("ctrl-f", Find, Some("Memi")),
        ]);
        let icon = image::load_from_memory(include_bytes!("../assets/app-icon.png")).ok().map(|i| Arc::new(i.to_rgba8()));
        let bounds = Bounds::centered(None, size(px(1180.), px(820.)), cx);
        let dirs = Dirs { data: data.clone(), photos: photo_dir.clone() };
        let (memi2, engine2) = (memi.clone(), engine.clone());
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions { title: Some("Memi".into()), ..Default::default() }),
                app_id: Some(APP_ID.into()),
                window_min_size: Some(size(px(480.), px(560.))),
                icon,
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| MemiView::new(memi2, engine2, dirs, window, cx));
                // Before the window closes, send anything not yet synced (for a few seconds at most).
                let engine = engine.clone();
                window.on_window_should_close(cx, move |_, _| {
                    if engine.snapshot().enabled && engine.snapshot().dirty {
                        let e = engine.clone();
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || { let _ = tx.send(e.sync()); });
                        let _ = rx.recv_timeout(Duration::from_secs(8));
                    }
                    true
                });
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        );
        if window.is_err() {
            eprintln!("Memi couldn't open its window.");
            cx.quit();
        }
        cx.on_window_closed(|cx, _| { if cx.windows().is_empty() { cx.quit(); } }).detach();
    });
}
