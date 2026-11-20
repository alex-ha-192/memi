# Your Memi

<img src="docs/memi.png" width="120" align="right" alt="Memi the elephant">

Your Memi is for keeping the specific things you want to remember about the people in your life: gift ideas, plans, what they can't eat, what they mentioned in passing. It isn't a contact book or a profile of everything you know about someone. It runs on Android, Windows and Linux.

Everything stays on your own devices. Sync is optional, and when it's on, the data is encrypted before it leaves your device and stored in your own Google Drive. There are no servers, accounts, analytics or ads. See the [privacy policy](https://yourmemi.app/privacy/) (source in `website/privacy/`).

- Website: <https://yourmemi.app> (source in `website/`)
- Questions and bugs: [open an issue](../../issues) or email hello@yourmemi.app

## Get it

| Platform | How |
|---|---|
| Android | The APK from [Releases](../../releases), or add this repo to [Obtainium](https://obtainium.imranr.dev) for automatic updates |
| Windows | `YourMemi-<version>.msi` (installs just for you and updates in place), or `YourMemi-<version>-portable.exe` |
| Linux | Flathub (`app.yourmemi.YourMemi`), or `YourMemi-<version>-x86_64.AppImage` |

## What's in it

- **Home:** the next birthday or dated category coming up, **Still fresh** (things added in the last two weeks, plus anything with a date coming up) and everyone. Pin people to the front or move them around from their menu.
- **Person:** a birthday tile with their photo, then their own categories. Everyone starts with Gift ideas (building up to their birthday), Plans and Notes. Add more per person. Any category can build up to a date, once or every year. A **Done** list keeps hits (✓) and misses (✕).
- **Notes:** press and hold (or right-click) for Hit, Miss, Mark done, Move to…, Edit and Delete.
- **Photos:** pick one and crop it to a slightly portrait frame. The saved copy is re-encoded, so location and camera details are dropped.
- **Themes:** Bento, Sorbet, Sea glass, Lavender and Clay, each in light and dark, or following the system.
- **Layouts:** one screen at a time on a phone, two panes on a tablet or computer.

## Sync

Theme panel › **Sync** › **Choose a Google account**.

- The first device makes a random 256-bit key and encrypts everything (people, categories, notes, photos) with AES-256-GCM before upload. The file goes in Drive's hidden app-data folder (scope `drive.appdata`), which isn't among your Drive files.
- The key stays in the Android Keystore, Windows Credential Manager, or the Linux Secret Service. On Linux without a keyring, it's kept in a file only your user account can read.
- A new phone set up from the old one gets the key through Android's Block Store (end-to-end encrypted with the screen lock). Otherwise, type the recovery key from Sync › Show recovery key.
- **Adding a computer:** the computer shows a QR code holding a one-off public key. The phone encrypts the sync key to it (ECDH P-256, HKDF-SHA256, AES-GCM) and leaves it in the same hidden Drive folder; the computer opens it, checks it unlocks the backup, and deletes it.
- **Limits:** it's one encrypted file, not a live merge. If two devices change things at the same moment, the last upload wins. When a device with people on it meets an account that already has a backup, the app asks which to keep.

Android uploads a few seconds after a change and when you leave the app, and pulls when you open it. The desktop app does the same when its window gains focus, and sends anything waiting before it closes.

## Building

### Android

Needs JDK 17 and the Android SDK (or Android Studio Ladybug or newer).

```
./gradlew assembleRelease            # app/build/outputs/apk/release/
./gradlew installRelease             # onto a plugged-in phone
./gradlew assembleRelease -PmemiVersion=0.5.0
```

Without signing secrets (below) the release build is signed with your local debug key. Debug builds are much slower than release builds, because Compose runs unoptimised.

### Desktop (Rust and GPUI)

The desktop app is a native Rust program in `desktop/`, using [GPUI](https://www.gpui.rs) for its interface. It shares the Android app's database layout, backup format and sync encryption byte for byte.

```
cd desktop
cargo run --release
cargo test
```

On Linux it needs: `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev libssl-dev pkg-config`, and a Vulkan driver at runtime.

Data lives in `%APPDATA%\YourMemi\data` on Windows and `~/.local/share/yourmemi` on Linux. **Portable mode:** if the program's file name contains "portable" (as the portable download's does), or a `portable.txt` or `Your Memi data` folder sits next to it, it keeps everything in a `Your Memi data` folder beside the program instead.

Packaging (CI does all of these; see `.github/workflows/build.yml`):

- **Windows .msi:** WiX v5, `desktop/packaging/main.wxs`. Its UpgradeCode never changes, so a newer .msi replaces the installed one and keeps your data.
- **AppImage:** built on Ubuntu 22.04 with `appimagetool`.
- **Flatpak:** `flatpak/app.yourmemi.YourMemi.yml`. It needs `cargo-sources.json`, which CI generates with `flatpak-cargo-generator.py` from `desktop/Cargo.lock`.

## Continuous builds and releases

Every push to `main` builds the APK, the .msi, the portable .exe, the AppImage and the Flatpak, and runs an Android screenshot tour on three emulator sizes (phone, phone in dark mode with large text, tablet). Results are under **Actions** › the latest **Build** run › **Artifacts**. Dev builds are numbered `0.0.<run>`.

To release, push a version tag. The files are attached to a GitHub Release with checksums; Obtainium picks up the APK from there.

```
git tag v0.5.0 && git push origin v0.5.0
```

### Secrets

Under **Settings › Secrets and variables › Actions**:

| Secret | What it is |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | The keystore that signs the APK, as base64 |
| `ANDROID_KEYSTORE_PASSWORD` | Its password |
| `ANDROID_KEY_ALIAS` | The key's alias |
| `ANDROID_KEY_PASSWORD` | The key's password |
| `CLOUDFLARE_API_TOKEN` | A Cloudflare API token with Account › Cloudflare Pages › Edit, for the website |
| `CLOUDFLARE_ACCOUNT_ID` | Your Cloudflare account ID (dashboard sidebar) |
| `FLATHUB_TOKEN` | A GitHub token that can push to `flathub/app.yourmemi.YourMemi` (after the app is accepted on Flathub) |

Android only installs an update signed with the same key as the installed app, and Google sign-in only works for keys registered in Google Cloud. Pick the signing key once and keep it safe. To get a keystore as base64: `base64 -w0 my.keystore` (Linux) or `[Convert]::ToBase64String([IO.File]::ReadAllBytes("my.keystore"))` (PowerShell). Each build prints the SHA-1 it was signed with.

Without the Android secrets, CI signs with a throwaway key: the APK runs, but it won't update an installed copy and Google sign-in won't work.

### Flathub

1. Fork [flathub/flathub](https://github.com/flathub/flathub), branch from `new-pr`, add `app.yourmemi.YourMemi.yml` and a `cargo-sources.json` from a CI run, and open a pull request. Follow [Flathub's submission guide](https://docs.flathub.org/docs/for-app-authors/submission).
2. Once accepted, Flathub creates `flathub/app.yourmemi.YourMemi`. Add `FLATHUB_TOKEN`, and each tagged release opens an update pull request there.

The metainfo file (`desktop/packaging/app.yourmemi.YourMemi.metainfo.xml`) points at screenshots on yourmemi.app, so the website needs to be live first.

### Website

`website/` is plain HTML and CSS, deployed to Cloudflare Pages (project `yourmemi`) by `.github/workflows/website.yml`. It needs the secrets `CLOUDFLARE_API_TOKEN` (with Cloudflare Pages: Edit) and `CLOUDFLARE_ACCOUNT_ID`; until they're set, the workflow skips the deploy. Security headers are in `website/_headers`. The site loads nothing from other sites, sets no cookies, and only stores the theme toggle in your browser. Keep Cloudflare's Web Analytics, Email Address Obfuscation and Rocket Loader off: each injects a script, which the site's security policy blocks and the privacy policy rules out.

## Google Cloud setup

Sync needs a Google Cloud project with the Drive API enabled and an OAuth consent screen asking only for `.../auth/drive.appdata`.

- **Android:** a client of type Android, package `app.memi`, with the SHA-1 of every key that signs the APK. Nothing goes in the code.
- **Desktop:** a client of type Desktop app, saved as `desktop/google-client.json`. Google treats desktop client secrets as not secret, since anyone could pull them out of the installed program; the file only identifies the app on the sign-in page.

## Code map

```
app/src/main/java/app/memi/       Android app (package kept as app.memi for updates and sign-in)
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
