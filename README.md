# Memi

<img src="docs/memi.png" width="120" align="right" alt="Memi the elephant">

Memi is the elephant who remembers for you. Tell Memi the specific things you want to remember about the people in your life (gift ideas, plans, what they can't eat, what they mentioned in passing) and Memi keeps them in order and brings them back when they matter. It isn't a contact book or a profile of everything you know about someone. Memi runs on Android, Windows, Linux and in your browser.

Everything stays on your own devices. Sync is optional, and when it's on, your Memi is encrypted before it leaves your device and stored in your own Google Drive. There are no servers, accounts, analytics or ads. Read the [privacy policy](https://yourmemi.app/privacy/).

**Website:** <https://yourmemi.app> · **Questions and bugs:** [open an issue](../../issues) or email hello@yourmemi.app

## Install

| Platform | How |
|---|---|
| Android 8+ | [YourMemi.apk](https://github.com/alex-ha-192/memi/releases/latest/download/YourMemi.apk), or [Obtainium](https://obtainium.imranr.dev) for automatic updates: add `https://github.com/alex-ha-192/memi` |
| Windows 10/11 | [YourMemi.msi](https://github.com/alex-ha-192/memi/releases/latest/download/YourMemi.msi). It installs just for you, and a newer one updates it in place |
| Any browser | [memi.yourmemi.app](https://memi.yourmemi.app): nothing to install, and it keeps everything in that browser |
| Linux | [YourMemi-x86_64.AppImage](https://github.com/alex-ha-192/memi/releases/latest/download/YourMemi-x86_64.AppImage). AppImage updaters such as [Gear Lever](https://github.com/mijorus/gearlever) can keep it up to date |

Older versions are on the [releases page](../../releases). The Windows installer isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info › Run anyway**. Smart App Control, if you've switched it on, blocks unsigned installers completely.

Each release lists the Android signing certificate's SHA-256 fingerprint, so you can check an APK is genuine before installing it (for example with [AppVerifier](https://github.com/soupslurpr/AppVerifier)). The package name is `app.memi`.

## What it does

- **Home:** **Coming up** (the next three birthdays, dated categories and dated plans or notes), **Still fresh** (things added in the last two weeks, plus anything with a date coming up; fold it away if you like) and everyone. Pin people to the front, or drag them into your own order.
- **People:** a birthday tile with their photo, then their own categories. Everyone starts with Gift ideas (building up to their birthday), Plans and Notes; add more per person. Any category can build up to a date, once or every year. A **Done** list keeps hits (✓) and misses (✕).
- **Notes:** press and hold (or right-click) for Hit, Miss, Mark done, Move to…, Edit and Delete. Hold and move (or drag with a mouse) to reorder notes, categories and people.
- **Birthday reminders (optional):** on the day, the day before and/or a week before. Android checks once a day around 9 am; the desktop app reminds you while it's open.
- **Photos:** pick one and crop it to a slightly portrait frame. The saved copy is re-encoded, so location and camera details are dropped.
- **Themes:** Bento, Sorbet, Sea glass, Lavender and Clay, each in light and dark, or following the system.
- **Layouts:** one screen at a time on a phone or a narrow window (half a screen works), two panes on a tablet or a wider window.

## How sync works

Theme panel › **Sync** › **Choose a Google account**.

- The first device makes a random 256-bit key and encrypts everything (people, categories, notes, photos) with AES-256-GCM before upload. The file goes in Drive's hidden app-data folder (scope `drive.appdata`), which you can't browse; the app can't see anything else in your Drive.
- The key stays in the Android Keystore, Windows Credential Manager, or the Linux Secret Service. On Linux without a keyring, it's kept in a file only your user account can read.
- A new phone set up from the old one gets the key through Android's Block Store (end-to-end encrypted with your screen lock). Otherwise, type the recovery key from Sync › Show recovery key.
- **Start anywhere:** a device without the key makes a one-off public key, shows it as a QR code and leaves it in the hidden folder as a join request. Any device that has the key shows *"… wants to join your Memi"* with a six-digit code to compare; Allow encrypts the sync key to that device (ECDH P-256, HKDF-SHA256, AES-GCM). The new device opens it, checks it unlocks the backup, and deletes it. A phone can also scan the QR code.
- **Merging:** every person, category and note has a permanent id and an edit time, and deletions are remembered for 180 days. Copies are merged item by item: edits made on two devices are both kept, and only the same item edited in two places at once keeps the later edit.

Every app checks for changes every 10 seconds while it's open and in front, and uploads 2 seconds after a change. In the web app, Google's sign-in lasts about an hour, after which sync pauses until you tap Reconnect.

## Building from source

### Android

Needs JDK 17 and the Android SDK (or Android Studio Ladybug or newer).

```
./gradlew assembleRelease            # app/build/outputs/apk/release/
./gradlew installRelease             # onto a plugged-in phone
```

Local release builds are signed with your own debug key. Use release builds to judge speed: debug builds run Compose unoptimised and are much slower.

### Desktop (Rust and GPUI)

The desktop app is a native Rust program in `desktop/`, using [GPUI](https://www.gpui.rs) for its interface. It shares the Android app's database layout, backup format and sync encryption byte for byte.

```
cd desktop
cargo run --release
cargo test
```

On Linux it needs `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev libssl-dev pkg-config`, and a Vulkan driver at runtime. Data lives in `%APPDATA%\YourMemi\data` on Windows and `~/.local/share/yourmemi` on Linux.

### Sign-in on your own builds

Google only lets registered apps sign in. Desktop builds use the client in `desktop/google-client.json` and work as they are (Google treats desktop client IDs as public). An Android build signed with your own key can't sign in until you register that key's SHA-1 in a Google Cloud project of your own: enable the Drive API, add the `drive.appdata` scope, and create an Android client for package `app.memi`. Everything except sync works without signing in.

## Contributing

Bug reports and pull requests are welcome. For anything large, please open an issue first so we can agree on the approach.

- Keep the app's promise: nothing leaves the device unless the user turns on sync, and then only encrypted. No analytics, crash reporting or new network calls.
- Every app runs the same engine. Below the screens, Android, desktop and web share `memi-engine` (`engine/`, on the Rust core in `core/`): the data, every edit, the SQLite store, Google Drive, merging, encryption and pairing. Only sign-in, where the key is kept, notifications and photo picking differ. `cargo test` in `core/` and `engine/` checks them (`engine/tests` runs two devices through the whole life of sync), `core/tests/interop/` checks the crypto against Java's, and `ffi/tests/kotlin/run.sh` runs Android's Kotlin bindings.
- Building Android needs Rust, the Android NDK and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk) (`cargo install cargo-ndk`, then `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`). Gradle builds the engine and its Kotlin bindings for you.
- CI builds every pull request on Android, Windows and Linux, and runs an Android screenshot tour on a phone, a phone with large text in dark mode, and a tablet.

Found a security problem? Please email hello@yourmemi.app rather than opening a public issue. See [SECURITY.md](SECURITY.md).

## Code map

```
engine/src/                       memi-engine: everything below the screens, for every app
  data, edits                     The data and every edit (by uid, and by row id for desktop and Android)
  memi                            Memi: data, saving, photos and sync state, over a Platform each app provides
  sync, native_drive              Sync (merge, push, joining devices, start over) and the Drive client
  sqlite                          The SQLite store (Android's and desktop's memi.db, back to Android 0.1)
core/src/                         Shared Rust core under the engine
  model, dates, insights, palette The data model, date rules, Home insights, colour schemes
  backup, merge                   Backup format 2 and the item-by-item merge
  crypto, pairing                 Encryption and handing the key between devices
ffi/                              The engine for Android, through UniFFI (Kotlin package app.memi.engine)
app/src/main/java/app/memi/       Android app (package app.memi)
  data/        Repository (the screens' way into the engine), dates, theme settings
  sync/        Sync timers and state, Play services sign-in, the key in Keystore and Block Store
  platform/    Photos, fonts, back handling, reminders, QR scanner
  ui/          Theme, components, screens, sheets
app/src/androidTest/              Screenshot tour used by CI
desktop/src/                      Desktop app (Rust)
  platform, photos                Where the engine keeps things on a computer, photo crop and import
  sync/                           auth (browser sign-in), secrets (keyring), running the engine's sync
  ui/                             GPUI views: app state, panes, sheets, widgets, theme, assets
desktop/packaging/                .desktop, metainfo, WiX installer, icons
web/                              memi.yourmemi.app (Rust, Leptos, WebAssembly)
website/                          yourmemi.app
docs/RELEASING.md                 How releases and the website are published
```

## Not built yet

- Reminders in the web app
- iOS and macOS apps (the web app covers them for now)

## Licence

Memi is free software under the **GNU General Public License v3.0**; see [`LICENSE`](LICENSE). Bundled third-party pieces keep their own licences, all compatible with the GPL:

- Plus Jakarta Sans font: SIL Open Font License 1.1 (`licenses/plus-jakarta-sans-OFL.txt`)
- Elephant icon and mascot, from Microsoft Fluent Emoji: MIT (`licenses/fluent-emoji-MIT.txt`)
- Lucide icons (desktop): ISC (`licenses/lucide-ISC.txt`)
