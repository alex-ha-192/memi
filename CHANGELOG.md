# Changelog

Every release of Memi, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). Releases 0.1 to 0.4 came before this repository; their entries are reconstructed from the project's notes and have no exact dates.

## [Unreleased] — 0.6.1

A release you shouldn't notice: nothing looks or works differently. Underneath, every app now runs on one shared engine.

### Changed
- New `memi-engine` crate: the data and every edit, saving, photos, undo and sync, shared by the Android, desktop and web apps. Each app keeps its own screens (Compose, GPUI, Leptos) and only the small parts that have to differ: Google sign-in, where the key is kept, notifications, photo picking.
- The web app runs on the engine. It keeps everything already saved in your browser.
- The desktop app runs on the engine. It opens its existing `memi.db` in place and writes only the rows that changed.
- Desktop: Google sign-in uses the `oauth2` crate (PKCE, state check, token refresh and revoke) instead of hand-written requests.
- One SQLite store for Android and desktop. It opens the Android app's existing database in place, including files from 0.5.
- One Google Drive client and one sync implementation, tested end to end with two devices joining, editing at once, deleting and starting over.

### Removed
- Desktop: the "which copy should be kept?" choice when first connecting. Since 0.6.0 both copies are merged item by item, so it could no longer appear.

### Security
- Security and privacy code moves onto established libraries wherever one exists. See [docs/SECURITY-REVIEW.md](docs/SECURITY-REVIEW.md) for what changed and what stays, and why.

## [0.6.0] — 2026-11-20

### Added
- Live sync that merges item by item: every person, category and note has a permanent id and an edit time, and deletions are remembered, so edits made on two devices are both kept. Apps check for changes every 10 seconds while open and send changes 2 seconds after you make them.
- Start anywhere: a device without the key asks your other devices for it. They show "… wants to join your Memi" with a six-digit code to compare, and Allow sends the key encrypted for that device alone.
- The web app at [memi.yourmemi.app](https://memi.yourmemi.app) (Rust and WebAssembly). It keeps everything in your browser, and Google's sign-in script only loads if you use sync.
- Optional birthday reminders: on the day, the day before and/or a week before. Android checks once a day around 9 am; the desktop app reminds you while it's open.
- Drag to reorder people, categories and notes (hold, then move, on Android).
- Coming up on Home: the next three birthdays, dated categories and dated plans or notes.

### Changed
- Still fresh can be folded away, and stays that way.
- Backups use format 2. Apps older than 0.6.0 can't open them, so update every device together.
- The shared Rust core (`core/`) holds the data model, dates, backup format, merge, encryption, pairing and colour schemes for the desktop and web apps.

## [0.5.2] — 2026-11-20

### Fixed
- Desktop: text in Home's big tile wraps instead of pushing the elephant out of the tile.

### Changed
- Desktop: windows narrower than 820 px show one screen at a time, like the phone, so Memi fits in half a screen.

## [0.5.1] — 2026-11-20

### Added
- Lost your recovery key? Start sync over from the Sync screen: Memi makes a new key and a fresh backup from what's on this device, and your other devices ask for the new key.
- The Linux AppImage can update itself with Gear Lever or AppImageUpdate.

### Changed
- Downloads keep the same file names, so the website's links always get the newest version. Releases contain only the APK, the Windows installer and the AppImage.

### Fixed
- A device that still has the old key no longer overwrites a backup it can't open.

## [0.5.0] — 2026-11-20

### Added
- The desktop app, rebuilt from the ground up in Rust with GPUI, for Windows (an `.msi` installer) and Linux (an AppImage).
- The website [yourmemi.app](https://yourmemi.app), with the privacy policy, hosted on Cloudflare Pages. Contact: hello@yourmemi.app.
- Obtainium support for Android updates.

### Changed
- The app was called "Your Memi" in this release. (It's Memi; the rename in the apps is planned.)
- Photos are re-encoded when imported, which drops location and camera details.

### Security
- The recovery key screen on Android blocks screenshots, and a copied key is marked sensitive.
- The desktop's fallback secrets file is readable only by your user account.

## [0.4.0]

### Added
- Windows and Linux, as a Compose desktop app.
- Encrypted sync through your own Google Drive: one file in the hidden app folder, encrypted on the device with a key Google never sees.
- Pairing a computer by scanning a QR code with the phone.

## [0.3.0]

### Changed
- Categories belong to each person. Everyone starts with Gift ideas (which builds up to their birthday), Plans and Notes. You can add, rename, remove and reorder them, and any category can build up to a date.

## [0.2.0]

### Changed
- Adding is done in each category. The quick-add bar that guessed where things went is gone.

## [0.1.0]

### Added
- The first Android app: people, with their photo and birthday, and gift ideas, plans and notes for each, marked done as a hit or a miss.

[Unreleased]: https://github.com/alex-ha-192/memi/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/alex-ha-192/memi/compare/v0.5.2...v0.6.0
[0.5.2]: https://github.com/alex-ha-192/memi/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/alex-ha-192/memi/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/alex-ha-192/memi/releases/tag/v0.5.0
