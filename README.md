# Memi

<img src="docs/memi.png" width="120" align="right" alt="Memi the elephant">

An app for Android, Windows and Linux for keeping specific ideas and notes about the people in your life: gift ideas, plans, small practical things to remember. It isn't meant as a profile of everything you know about someone. Everything lives on your phone, with optional end-to-end encrypted sync to your Google account.

Built with Kotlin and Jetpack Compose in the **Bento** design: soft pastel tiles, Plus Jakarta Sans, five colour schemes in light and dark.

## Build and install

You need **Android Studio** (Ladybug 2024.2 or newer) on your computer.

1. Unzip this folder and open it in Android Studio (**File › Open**, pick the `Memi` folder).
2. Let Gradle sync. The first sync downloads Gradle and the libraries and takes a few minutes. If Studio offers to update the Android Gradle plugin or library versions, it's fine to accept.
3. On your phone, turn on **Developer options › USB debugging**. On a Fairphone: Settings › About phone › tap *Build number* seven times.
4. Plug the phone in, pick it in the device menu at the top, and press **Run ▶**.

From a terminal instead: `./gradlew installDebug` with the phone plugged in (needs JDK 17 and the Android SDK).

## Automatic builds

GitHub builds Memi on every push to `main`: the Android APK, the Windows .msi and the Linux .deb. Find them under the repo's **Actions** tab › the latest **Build** run › **Artifacts**. To make a proper release, push a version tag. The three files are then attached to a GitHub Release with a permanent download link:

```
git tag v0.4.0 && git push origin v0.4.0
```

On Android, [Obtainium](https://obtainium.imranr.dev) can watch the repo's releases and install updates for you.

### One-time setup: secrets

Add these under the repo's **Settings › Secrets and variables › Actions › New repository secret**:

| Secret | What it is |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | The key that signs the APK, as base64 (see below) |
| `ANDROID_KEYSTORE_PASSWORD` | Its password |
| `ANDROID_KEY_ALIAS` | The key's name inside it |
| `ANDROID_KEY_PASSWORD` | The key's password |

**The easiest signing key is the one you already use.** Android Studio's debug key signed the Memi on your phone, and its SHA-1 is already in Google Cloud. If CI uses the same key, its APKs install as normal updates and sync keeps working. The file is `~/.android/debug.keystore` on Linux, or `%USERPROFILE%\.android\debug.keystore` on Windows. Its details are always the same: password `android`, alias `androiddebugkey`, key password `android`. To get it as base64:

- Linux: `base64 -w0 ~/.android/debug.keystore`
- Windows (PowerShell): `[Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:USERPROFILE\.android\debug.keystore"))`

If you use a different key instead, Android won't install its APKs over the existing app. You'd need to uninstall first, so copy your recovery key out of Sync first, and add the new key's SHA-1 to Google Cloud. Each build prints its SHA-1 in the "Name the APK" step.

Without the secrets the builds still run, but the APK is signed with a throwaway key. It won't update your installed Memi, and Google sign-in won't work with it.

## Desktop (Windows and Linux)

The desktop app is the same Memi in a window: the same screens, data and sync. Below 840 pixels wide it shows one screen at a time; wider, it uses the two-pane layout. With a mouse, a pencil sits on the profile photo and a ⋯ button appears when you hover over an item (right-click works too). Escape goes back: it closes the sheet on top, then search, then the theme panel.

**Run it:** open the project in Android Studio or IntelliJ IDEA, then run the Gradle task **desktop › compose desktop › run**, or from a terminal:

```
./gradlew :desktop:run
```

**Make an installer:** installers are built on the system they're for (Java can't cross-build them).

- Windows: `gradlew.bat :desktop:packageMsi`. The build downloads the WiX tools it needs the first time. The .msi ends up in `desktop/build/compose/binaries/main/msi/`. It installs just for you, with a Start menu entry and a desktop shortcut.
- Linux (Debian, Ubuntu and similar): `./gradlew :desktop:packageDeb`, which needs `fakeroot` (`sudo apt install fakeroot`). The .deb is in `desktop/build/compose/binaries/main/deb/`. Install it with `sudo apt install ./memi_0.4.0-1_amd64.deb`.

Both include their own Java runtime, so nothing else needs installing on the computer. Memi keeps its data in `%APPDATA%\Memi` on Windows and `~/.local/share/memi` on Linux.

## Meet Memi

Memi's icon and mascot is the elephant from **Microsoft Fluent Emoji** (flat style), which is free to use under the MIT licence. It sits on a soft lilac background (#E3DEFF). There's also a one-colour version for Android's themed icons, and Memi bobs gently on the home screen's welcome and "No birthdays this month" tiles. The drawings are vector files in `app/src/main/res/drawable` (`ic_launcher_*.xml`, `memi_mascot.xml`).

Licences for the icon and the font are in `licenses/`.

## What's in it

- **Home:** the next birthday or dated category coming up, **Still fresh** (things added in the last two weeks, plus anything whose date is coming up) and everyone. Tap something in Still fresh to jump straight into its category. Press and hold a person to pin them to the front, move them earlier or later, edit or remove them.
- **Person:** a birthday tile with their photo, then **their own categories** as tiles. Everyone starts with Gift ideas (building up to their birthday), Plans and Notes. Add more per person. Any category can **build up to their birthday or a date**, either once (a trip) or every year (an anniversary). Things in it then show as due in Still fresh as the day gets close. **Tap a category to add to it**. Press and hold a category to edit it, move it earlier or later, or remove it. A **Done** list shows ✓ for hits and ✕ for misses.
- **Notes:** press and hold any note (or right-click with a mouse) for Hit, Miss, Mark done, Move to…, Edit and Delete. With a mouse, a ⋯ button appears on hover and a pencil sits on the profile photo.
- **Photos:** pick one, then drag and pinch to crop it to a slightly portrait frame. "Adjust crop" re-frames an existing photo.
- **Themes:** the palette button opens a panel with Bento, Sorbet, Sea glass, Lavender and Clay, each in light and dark, plus Light / Dark / System. Tap anywhere outside the panel to close it.
- **Phone, tablet and desktop:** below 840dp wide it's one screen at a time with Android's system back. Wider, it becomes two panes (people on the left, the person on the right).

## Sync

Theme panel › **Sync** › **Choose a Google account**. That's all you do in the app. From then on Memi:

- uploads a few seconds after you change something, and when you leave the app
- pulls in changes from your other phone or tablet when you open it
- on a new phone set up from your old one, finds the key and the backup by itself; pick the same account and everything comes back
- on a computer, opens Google's sign-in in your browser (pick the same account). The first time, the computer shows a **QR code**: on your phone, open Sync › **Add a computer** and scan it. The phone passes the key across encrypted, so only that computer can read it. After that the computer syncs by itself too. If you can't scan, typing the recovery key from Sync › Show recovery key works as well. It checks for changes whenever you switch back to the window, and sends anything waiting before it closes.

**Encrypted:** the first time, Memi makes a random 256-bit key on your phone and encrypts everything (people, categories, notes, photos) with AES-256-GCM before uploading. Google only stores ciphertext, in Drive's hidden app-data space. It's not among your Drive files, and Memi can't see those files either. On the phone, the key stays inside the Android Keystore. On a computer, it's kept in Windows Credential Manager or the Linux keyring (GNOME Keyring / KWallet). Without a keyring, it goes in a file in Memi's folder that only your user account can read. It reaches your next phone through Android's Block Store, which Google encrypts end to end with your screen lock. If a phone wasn't set up from your old one, Memi asks for the **recovery key** (Sync › Show recovery key on the old phone).

How the QR pairing works: the computer makes a one-off key pair and shows the public half. The phone uses it to encrypt the sync key (ECDH P-256 + HKDF + AES-GCM) and leaves the result in your Drive app folder. The computer opens it, checks it unlocks your backup, and deletes it. A photo of the code is no use to anyone else: they'd need your Google account as well, and even then they could only hand that computer a key, not read yours.

**Honest limits:** it's one encrypted file, not a live merge. If two devices change things at the same moment, the last one to upload wins. When you first connect a phone that already has people on it to an account that also has a backup, Memi asks which one to keep.

### One-time setup (because you build Memi yourself)

Google only lets an app use Drive once it's registered to that app's signing key. Because you build Memi yourself, you need to do this once (about 5 minutes, free). You never need to touch Drive itself.

1. Open [console.cloud.google.com](https://console.cloud.google.com), sign in, and create a project called **Memi**.
2. **APIs & Services › Library**: search for **Google Drive API** and click **Enable**.
3. **Google Auth Platform** (OAuth consent screen): app name *Memi* and your email, audience **External**. Under **Data access**, add the scope `.../auth/drive.appdata`. Under **Audience**, press **Publish app**. Only this hidden-folder scope is used, so Google doesn't need to review it. If Google does ask for verification, leave the app in Testing and add your account as a test user instead. Testing apps may make you sign in again about once a week.
4. **Clients › Create client › Android**: package name `app.memi`, and the SHA-1 fingerprint from Android Studio. Open the Gradle panel and run **Tasks › android › signingReport**, or run `./gradlew signingReport`, then copy the **SHA1** for the `debug` variant. The release build uses the same key.

There's nothing to paste into the code for Android: Google recognises Memi by its package name and key. If you build on a different computer, its debug key is different, so add a second Android client with that SHA-1.

### Desktop sync

5. In the same project: **Clients › Create client › Desktop app**, named *Memi desktop*. Click **Download JSON**, rename the file to `google-client.json`, put it in `desktop/src/main/resources/`, and commit it. Every build after that, on your computer or on GitHub, has desktop sync.

The file holds a client ID and a "secret". Google says desktop apps can't keep that secret, since anyone could pull it out of the installed program, so it isn't treated as one: it only identifies Memi to Google's sign-in page. Many open-source apps commit theirs. Your data stays protected by your Google account and Memi's encryption.

## Smoothness

Debug builds (what Android Studio's Run button installs by default) are much slower than the real app, because Compose runs without optimisation. To see how Memi actually feels, install the release build: **Build › Select Build Variant › release**, then Run, or `./gradlew installRelease`. The release build is set up to sign with your local debug key, so it installs straight away.

## Privacy

- Everything is stored on your phone in a private database. The only network access is the optional sync above, which uploads end-to-end encrypted data to your own Google account and nowhere else.
- **Photos** are cropped and copied into the app's private storage. The originals are untouched.
- **Android's own cloud backup is off** (`allowBackup="false"`). Memi's sync replaces it, with encryption Memi controls.

## Code map

Almost everything lives in `shared/` and is compiled into both apps. Each app adds a small platform layer with the same names: `app.memi.platform.*` (photos, fonts, back, sign-in) and a `SyncPlatform`.

```
shared/src/main/kotlin/app/memi/
  data/       Room database (people, per-person categories, notes), repository, dates, backup format, theme settings
  sync/       Encryption, Google Drive, the sync engine, and the SyncPlatform interface
  platform/   Photo cache, plus notes on what each app provides
  ui/
    theme/    Palettes (5 schemes × light/dark), type, motion, snapshot cross-fade between themes
    components/  Tiles, note cards, menus, toasts, theme panel
    screens/  Home, Person, collection sheet
    sheets/   Edit person, photo cropper, edit note, category editor, sync
    App.kt    Phone vs two-pane layout, dialogs
app/src/main/java/app/memi/
  MainActivity, MemiApp, platform/ (Android photos, fonts, migrations), sync/ (Play services sign-in, Keystore, Block Store)
desktop/src/main/kotlin/app/memi/
  Main.kt (window), platform/ (file dialog, Skia photos, Escape), sync/ (browser sign-in, system keyring)
```

## Not built yet

- A true multi-device merge (sync is last-writer-wins today)
- Reminders before birthdays
- Drag-to-reorder (people and categories move one place at a time from their menus for now)

## Licence

Memi is free software under the **GNU General Public License v3.0**; see [`LICENSE`](LICENSE). Bundled third-party pieces keep their own licences, both compatible with the GPL:

- Plus Jakarta Sans font: SIL Open Font License 1.1 (`licenses/plus-jakarta-sans-OFL.txt`)
- Elephant icon and mascot, from Microsoft Fluent Emoji: MIT (`licenses/fluent-emoji-MIT.txt`)
