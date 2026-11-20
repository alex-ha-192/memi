# Security and privacy review (0.6.1)

For 0.6.1, every part of Memi that protects your data was checked against one rule: **use an established, widely reviewed library or platform service wherever one exists, and keep hand-written code only where no better-proven option exists.** This file records each decision, so the next change can be measured against it.

The threat model hasn't changed. Memi lives on your devices and, if you turn sync on, as one encrypted file in your own Google Drive's hidden app folder. Nothing goes through a server of ours, because there isn't one. Google stores only ciphertext and never sees the key.

## What changed in 0.6.1

| Area | Before | After | Why |
|---|---|---|---|
| Desktop Google sign-in (PKCE, `state` check, token exchange, refresh, revocation) | Written by hand: random verifier and challenge, URL building and parsing, form posts | The [`oauth2`](https://crates.io/crates/oauth2) crate (PKCE S256, CSRF state, typed token responses, RFC 7009 revocation), with redirects disabled on its HTTP client as it recommends. Loopback query parsing uses the [`url`](https://crates.io/crates/url) crate (WHATWG URL Standard) | OAuth details are easy to get subtly wrong. The crate is the standard Rust OAuth 2 client |
| Recovery key text | Hand-written hex encoding and decoding | The [`hex`](https://crates.io/crates/hex) crate, with the text held in `Zeroizing` buffers | Less code of our own on the key path |
| Elliptic-curve arithmetic (pairing) | `p256` 0.13, which uses `crypto-bigint` 0.5 with timing variability in operations meant to be constant-time ([RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387)) | `p256` 0.14 (`crypto-bigint` 0.7.11), keys from the OS via the crate's own `Generate` | Fixes a known advisory |
| AES-GCM, HKDF, SHA-2 | `aes-gcm` 0.10, `hkdf` 0.12, `sha2` 0.10 | `aes-gcm` 0.11, `hkdf` 0.13, `sha2` 0.11 (current RustCrypto releases) | Stay on maintained releases |
| SQLite (Android and desktop) | `rusqlite` 0.32 (bundled SQLite with a CVE, [RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459)) | `rusqlite` 0.43 | Fixes a known advisory |
| Android encryption, pairing, the backup format and Google Drive | A Kotlin copy using `javax.crypto` and `HttpURLConnection` | The shared Rust engine (RustCrypto, `ureq` with `rustls`), reached from Kotlin through [UniFFI](https://mozilla.github.io/uniffi-rs/) | One implementation to review instead of two |
| Watching for new advisories | None | `cargo audit` in CI on every dependency change and weekly; Dependabot for Rust, Gradle and GitHub Actions | Advisories appear after code is written |
| Unused crypto dependencies | The desktop app still depended on its own `p256`, `aes-gcm`, `hkdf`, `sha2`, `rand` and `flate2` | Removed; all of that lives in `memi-core` | Fewer things to keep patched |

The wire formats are unchanged: backups, pairing and join requests stay byte-for-byte compatible with 0.6.0. The Java interop test (`core/tests/interop/`) still passes against the upgraded crates.

## What stays, and why

| Area | Implementation | Why it stays |
|---|---|---|
| Backup encryption | AES-256-GCM ([`aes-gcm`](https://crates.io/crates/aes-gcm), RustCrypto) over gzip ([`flate2`](https://crates.io/crates/flate2)), a random 96-bit nonce per backup from the OS, the format tag as associated data, keys in `Zeroizing` memory | Standard AEAD from a widely used, audited library family. The only code of ours is the file layout around it |
| Handing the key to another device (pairing and join requests) | ECDH on P-256 (`p256`), HKDF-SHA256 (`hkdf`), AES-256-GCM: an ephemeral-static scheme of the same shape as HPKE's base mode | HPKE (RFC 9180) is the standard for this, and was considered. Its main Rust implementations are no better proven than the RustCrypto primitives used now: rust-hpke says it has never had a paid audit, and hpke-rs had a critical advisory fixed in 0.6.0. Switching would also break pairing with 0.6.0 devices. To revisit once a well-reviewed HPKE crate exists |
| Six-digit confirm code | The first 4 bytes of SHA-256 of the pairing code, mod 1,000,000 | A short authentication string for comparing two screens, not a secret |
| Key storage | Android Keystore and Block Store (end-to-end encrypted with your screen lock); Windows Credential Manager or the Linux Secret Service through the [`keyring`](https://crates.io/crates/keyring) crate; in the browser, IndexedDB for memi.yourmemi.app | Platform services. The desktop's fallback, used only without a keyring, is a file created owner-only (`0600`) and replaced atomically |
| Sign-in on Android and the web | Google Play services (Credential Manager and the authorisation client) and Google Identity Services | Google's own libraries |
| HTTPS | `ureq` with `rustls` and the Mozilla root store on Android and desktop; the browser's `fetch` on the web | Standard TLS, no custom certificate handling |
| Randomness | The OS generator (`getrandom`, `OsRng`; `crypto.getRandomValues` in the browser) | Standard |
| Photo privacy | Photos are decoded and re-encoded with the [`image`](https://crates.io/crates/image) crate (Android: platform decoders), with size limits on the decoder. Re-encoding drops location and camera details by construction | No metadata-stripping code of our own to get wrong |
| Web app isolation | A strict Content-Security-Policy (`web/public/_headers`): scripts from the site and Google's sign-in only, connections only to Google Drive, no framing. Google's script loads only once you use sync | Browser-enforced |

## Known warnings

`cargo audit` reports no vulnerabilities. It does flag some crates as unmaintained:

- **`chrono`** (dates, all apps). A move to [`jiff`](https://crates.io/crates/jiff) is planned. Date handling isn't security-facing, so it isn't part of 0.6.1.
- **`instant`, `paste`, `rustls-pemfile`, `rustybuzz`, `ttf-parser`, `zbus_names`, `zvariant`**: pulled in by the GPUI desktop toolkit and the notification library. They go away when those projects update.

## Checking it yourself

```sh
cargo install cargo-audit --locked
for c in core engine desktop web; do (cd $c && cargo audit); done
```
