//! Everything in Memi below the screens, shared by the Android, desktop and web apps: the data and
//! every edit to it, saving (through each app's [Platform]), and sync.
//!
//! The screens stay native to each platform (Compose, GPUI, Leptos). They read from a [Memi] and
//! make every change through it.

pub mod data;
pub mod memi;
pub mod sync;
#[cfg(feature = "native-drive")]
pub mod native_drive;
#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use data::Data;
pub use memi::{Asking, Join, Memi, PhotoChange, Platform, Problem, SyncSettings, SyncStatus};
pub use memi_core as core;
