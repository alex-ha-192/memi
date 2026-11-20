//! The parts of Memi that every app shares, so the Android, desktop and web apps read and write
//! exactly the same data: the model, date rules, Home insights, the backup format, how two copies
//! merge, end-to-end encryption, and handing the sync key between devices.
//!
//! The Android app is Kotlin and has its own copy of these rules (app/src/main/java/app/memi);
//! the two must stay byte-for-byte compatible.

pub mod backup;
pub mod crypto;
pub mod dates;
pub mod insights;
pub mod merge;
pub mod model;
pub mod pairing;
