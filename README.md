# Your Memi

<img src="docs/memi.png" width="120" align="right" alt="Memi the elephant">

Your Memi is for keeping the specific things you want to remember about the people in your life: gift ideas, plans, what they can't eat, what they mentioned in passing. It isn't a contact book or a profile of everything you know about someone. It runs on Android, Windows and Linux.

Everything stays on your own devices. Sync is optional, and when it's on, your data is encrypted before it leaves your device and stored in your own Google Drive. There are no servers, accounts, analytics or ads. Read the [privacy policy](https://yourmemi.app/privacy/).

**Website:** <https://yourmemi.app> · **Questions and bugs:** [open an issue](../../issues) or email hello@yourmemi.app

## Install

| Platform | How |
|---|---|
| Android 8+ | The APK from [Releases](../../releases/latest), or [Obtainium](https://obtainium.imranr.dev) for automatic updates: add `https://github.com/alex-ha-192/memi` |
| Windows 10/11 | `YourMemi-<version>.msi` from [Releases](../../releases/latest). It installs just for you, and a newer one updates it in place |
| Linux | [Flathub](https://flathub.org/apps/app.yourmemi.YourMemi) (`app.yourmemi.YourMemi`), or `YourMemi-<version>-x86_64.AppImage` from [Releases](../../releases/latest) |

The Windows installer isn't code-signed, so SmartScreen may say "Windows protected your PC". Choose **More info › Run anyway**. Smart App Control, if you've switched it on, blocks unsigned installers completely.

Each release lists the Android signing certificate's SHA-256 fingerprint, so you can check an APK is genuine before installing it (for example with [AppVerifier](https://github.com/soupslurpr/AppVerifier)). The package name is `app.memi`.

## What it does

- **Home:** the next birthday or dated category coming up, **Still fresh** (things added in the last two weeks, plus anything with a date coming up) and everyone. Pin people to the front or move them around from their menu.
- **People:** a birthday tile with their photo, then their own categories. Everyone starts with Gift ideas (building up to their birthday), Plans and Notes; add more per person. Any category can build up to a date, once or every year. A **Done** list keeps hits (✓) and misses (✕).
- **Notes:** press and hold (or right-click) for Hit, Miss, Mark done, Move to…, Edit and Delete.
- **Photos:** pick one and crop it to a slightly portrait frame. The saved copy is re-encoded, so location and camera details are dropped.
- **Themes:** Bento, Sorbet, Sea glass, Lavender and Clay, each in light and dark, or following the system.
- **Layouts:** one screen at a time on a phone, two panes on a tablet or computer.

## How sync works

Theme panel › **Sync** › **Choose a Google account**.

- The first device makes a random 256-bit key and encrypts everything (people, categories, notes, photos) with AES-256-GCM before upload. The file goes in Drive's hidden app-data folder (scope `drive.appdata`), which you can't browse; the app can't see anything else in your Drive.
- The key stays in the Android Keystore, Windows Credential Manager, or the Linux Secret Service. On Linux without a keyring, it's kept in a file only your user account can read.
- A new phone set up from the old one gets the key through Android's Block Store (end-to-end encrypted with your screen lock). Otherwise, type the recovery key from Sync › Show recovery key.
- **Adding a computer:** the computer shows a QR code holding a one-off public key. The phone encrypts the sync key to it (ECDH P-256, HKDF-SHA256, AES-GCM) and leaves it in the same hidden folder; the computer opens it, checks it unlocks the backup, and deletes it.
- **Limits:** it's one encrypted file, not a live merge. If two devices change things at the same moment, the last upload wins. When a device that already has people on it meets an account with a backup, the app asks which to keep.

Android uploads a few seconds after a change and when you leave the app, and pulls when you open it. The desktop app does the same when its window gains focus, and sends anything waiting before it closes.

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
- Android and desktop share formats. A change to the database, backup or sync format needs both sides. `cargo test` covers the desktop half, and `desktop/tests/interop/` checks the desktop crypto against Java's.
- CI builds every pull request on Android, Windows and Linux, including the Flatpak, and runs an Android screenshot tour on a phone, a phone with large text in dark mode, and a tablet.

Found a security problem? Please email hello@yourmemi.app rather than opening a public issue. See [SECURITY.md](SECURITY.md).

## Code map

```
app/src/main/java/app/memi/       Android app (package app.memi)
  data/        Room database, repository, dates, backup format, theme settings
  sync/        Encryption, Drive, sync engine, pairing (phone side), Play services sign-in and Block Store
  platform/    Photos, fonts, back handling, migrations, QR scanner
  ui/          Theme, components, screens, sheets
app/src/androidTest/              Screenshot tour used by CI
desktop/src/                      Desktop app (Rust)
  model, store, dates, insights   The same data model and rules as Android
  photos, backup                  Photo crop and import, backup format
  sync/                           crypto, drive, auth (browser sign-in), pairing (computer side), secrets (keyring)
  ui/                             GPUI views: app state, panes, sheets, widgets, theme, assets
desktop/packaging/                .desktop, metainfo, WiX installer, icons
flatpak/                          Flathub manifest
website/                          yourmemi.app
docs/RELEASING.md                 How releases, Flathub and the website are published
```

## Not built yet

- A true multi-device merge (sync is last-writer-wins)
- Reminders before birthdays
- Drag to reorder

## Licence

Your Memi is free software under the **GNU General Public License v3.0**; see [`LICENSE`](LICENSE). Bundled third-party pieces keep their own licences, all compatible with the GPL:

- Plus Jakarta Sans font: SIL Open Font License 1.1 (`licenses/plus-jakarta-sans-OFL.txt`)
- Elephant icon and mascot, from Microsoft Fluent Emoji: MIT (`licenses/fluent-emoji-MIT.txt`)
- Lucide icons (desktop): ISC (`licenses/lucide-ISC.txt`)
