# Releasing Memi

Notes for maintainers on how builds, releases and listings are published. Everything runs in GitHub Actions; nothing is built on a personal computer.

## Builds

Every push to `main` and every pull request runs `.github/workflows/build.yml`:

| Job | Output |
|---|---|
| Android APK | `YourMemi.apk`, plus its permissions and signing fingerprint in the log |
| Android screenshots | A screenshot tour on three emulators, uploaded as an artifact and pushed to the `ci-screens` branch |
| Linux | Tests, the binary, and `YourMemi-x86_64.AppImage` with its `.zsync` file (built on Ubuntu 22.04) |
| Windows | Tests and `YourMemi.msi` (WiX v5, per-user, fixed UpgradeCode so new versions upgrade in place) |

Untagged builds are numbered `0.0.<run>`, so test installers upgrade each other but never replace a real release.

## Making a release

1. Add a `<release version="X.Y.Z" date="YYYY-MM-DD">` entry with notes to `desktop/packaging/app.yourmemi.YourMemi.metainfo.xml` (it ships inside the AppImage).
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`. Or, without pushing a tag, run the Build workflow on main by hand with the version (`gh workflow run build.yml --ref main -f release=X.Y.Z`, or Actions › Build › Run workflow): the release creates the tag on that commit.
3. Publish the web app from the same commit: `gh workflow run web.yml --ref main -f deploy=true` (or Actions › Web app › Run workflow, with "Publish" ticked). Pushing to `main` builds and checks the web app but never publishes it.

The tag sets the version everywhere (Android versionName and versionCode, the .exe, the .msi and the AppImage). The Release job attaches only `YourMemi.apk`, `YourMemi.msi`, `YourMemi-x86_64.AppImage` and its `.zsync` file, with notes giving the Android certificate fingerprint and SHA-256 checksums. File names never change, so `https://github.com/alex-ha-192/memi/releases/latest/download/<name>` always serves the newest release; the website links there. Obtainium picks the APK up from there, and AppImage updaters the AppImage.

A tagged build fails if the Android signing secrets are missing: a release signed with any other key couldn't update installed copies.

## Secrets and variables

Under Settings › Secrets and variables › Actions:

| Secret | Used for |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | The release keystore, base64-encoded (`base64 -w0 file.jks`) |
| `ANDROID_KEYSTORE_PASSWORD` | Its password |
| `ANDROID_KEY_ALIAS` | The key alias |
| `ANDROID_KEY_PASSWORD` | The key password (the same as the keystore's for PKCS12) |
| `CLOUDFLARE_API_TOKEN` | Website deploys: a Cloudflare token with Account › Cloudflare Pages › Edit |
| `CLOUDFLARE_ACCOUNT_ID` | Website deploys: the Cloudflare account ID |

| Variable (not secret) | Used for |
|---|---|
| `MEMI_WEB_CLIENT_ID` | The web app's Google OAuth client ID. Without it the web app works, but Sync is off. Never add the client secret anywhere: browser sign-in doesn't use it. |

Without the Android secrets, untagged builds are signed with a throwaway key: the APK runs, but it can't update an installed copy or sign in to Google.

## Google sign-in

The Google Cloud project has the Drive API enabled and a consent screen that asks only for `.../auth/drive.appdata`, with the privacy policy URL and hello@yourmemi.app as the support contact.

- **Android:** an Android client for package `app.memi` for each signing key's SHA-1 (the release key, and any debug key used for development).
- **Desktop:** a Desktop app client, committed as `desktop/google-client.json`.
- **Web:** a Web application client with the authorised JavaScript origin `https://memi.yourmemi.app` (no redirect URIs), whose ID is the `MEMI_WEB_CLIENT_ID` variable.

All clients must be in the **same** Google Cloud project: Drive's hidden app folder belongs to the project, so a client from another project sees an empty folder.

## AppImage updates

The AppImage carries update information (`gh-releases-zsync|alex-ha-192|memi|latest|YourMemi-x86_64.AppImage.zsync`), and each release includes the matching `.zsync` file. Updaters such as Gear Lever and AppImageUpdate use them to fetch only the changed parts of a new release. Keep the file names as they are: the update info, the website and the README all point at them.

## Obtainium

Obtainium reads GitHub Releases directly, so nothing else is needed. The website's badge links to `obtainium://app/` with the app's config. The [crowdsourced list](https://github.com/ImranR98/apps.obtainium.imranr.dev) accepts projects with at least 35 GitHub stars that are at least four months old; once the project qualifies, add `public/data/apps/simple/app.memi.json` there with `id`, `url`, `author` and `name`.

## Website

`website/` is plain HTML and CSS, deployed to Cloudflare Pages (project `yourmemi`) by `.github/workflows/website.yml` whenever it changes. Security headers are in `website/_headers`. Keep Cloudflare's Web Analytics, Email Address Obfuscation and Rocket Loader off: the privacy policy promises no analytics, and the other two rewrite the pages.

## Web app

`web/` is built with [Trunk](https://trunkrs.dev) by `.github/workflows/web.yml` on every change to `web/`, `engine/` or `core/`, and pushed to the `ci-web` branch for checking. It's published to the Cloudflare Pages project `memi-web`, served at `memi.yourmemi.app` (Pages › memi-web › Custom domains), only when the workflow is started by hand with `deploy` ticked, as step 3 of a release. Its headers (including a strict Content-Security-Policy that allows only Google's sign-in and Drive) are in `web/public/_headers`. To roll the live site back to an earlier release: `gh workflow run web.yml --ref main -f deploy=true -f ref=vX.Y.Z`. To try it locally: `cd web && trunk serve`.
