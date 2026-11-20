# Releasing Your Memi

Notes for maintainers on how builds, releases and listings are published. Everything runs in GitHub Actions; nothing is built on a personal computer.

## Builds

Every push to `main` and every pull request runs `.github/workflows/build.yml`:

| Job | Output |
|---|---|
| Android APK | `YourMemi-<version>.apk`, plus its permissions and signing fingerprint in the log |
| Android screenshots | A screenshot tour on three emulators, uploaded as an artifact and pushed to the `ci-screens` branch |
| Linux | Tests, the binary, and `YourMemi-<version>-x86_64.AppImage` with its `.zsync` file (built on Ubuntu 22.04) |
| Windows | Tests and `YourMemi-<version>.msi` (WiX v5, per-user, fixed UpgradeCode so new versions upgrade in place) |

Untagged builds are numbered `0.0.<run>`, so test installers upgrade each other but never replace a real release.

## Making a release

1. Add a `<release version="X.Y.Z" date="YYYY-MM-DD">` entry with notes to `desktop/packaging/app.yourmemi.YourMemi.metainfo.xml` (it ships inside the AppImage).
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.

The tag sets the version everywhere (Android versionName and versionCode, the .exe, the .msi and the AppImage). The Release job attaches every file with `SHA256SUMS.txt` and writes notes with the Android certificate fingerprint. Obtainium picks the APK up from there, and AppImage updaters the AppImage.

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

Without the Android secrets, untagged builds are signed with a throwaway key: the APK runs, but it can't update an installed copy or sign in to Google.

## Google sign-in

The Google Cloud project has the Drive API enabled and a consent screen that asks only for `.../auth/drive.appdata`, with the privacy policy URL and hello@yourmemi.app as the support contact.

- **Android:** an Android client for package `app.memi` for each signing key's SHA-1 (the release key, and any debug key used for development).
- **Desktop:** a Desktop app client, committed as `desktop/google-client.json`.

## AppImage updates

The AppImage carries update information (`gh-releases-zsync|alex-ha-192|memi|latest|YourMemi-*-x86_64.AppImage.zsync`), and each release includes the matching `.zsync` file. Updaters such as Gear Lever and AppImageUpdate use them to fetch only the changed parts of a new release. Keep the file name pattern when renaming anything.

## Obtainium

Obtainium reads GitHub Releases directly, so nothing else is needed. The website's badge links to `obtainium://app/` with the app's config. The [crowdsourced list](https://github.com/ImranR98/apps.obtainium.imranr.dev) accepts projects with at least 35 GitHub stars that are at least four months old; once the project qualifies, add `public/data/apps/simple/app.memi.json` there with `id`, `url`, `author` and `name`.

## Website

`website/` is plain HTML and CSS, deployed to Cloudflare Pages (project `yourmemi`) by `.github/workflows/website.yml` whenever it changes. Security headers are in `website/_headers`. Keep Cloudflare's Web Analytics, Email Address Obfuscation and Rocket Loader off: the privacy policy promises no analytics, and the other two rewrite the pages.
