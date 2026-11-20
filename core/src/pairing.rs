//! Handing the sync key to a new device without it ever being readable on the way. Byte-for-byte
//! compatible with the Android app's Pairing.kt.
//!
//! 1. The new device makes a one-off P-256 key pair and offers its public half (X.509 DER,
//!    base64url) as a code: `memi:pair:1:<id>:<public key>`. It shows the code as a QR code, and
//!    also leaves it in the Drive app folder as a join request (`memi-join-<id>.json`) so a device
//!    without a camera can answer. Both sides show the same 6-digit [confirm_code] to compare.
//! 2. A device that has the key agrees a secret with it (ECDH, then HKDF-SHA256 with info
//!    "memi-pair-v1:<id>"), encrypts the sync key with AES-GCM ([seal_for]) and leaves it in the
//!    Drive app folder as `memi-pair-<id>.bin`: u16 length | its public key | 12-byte IV | ciphertext.
//! 3. The new device opens it ([Offer::open]), checks the key opens the backup, and deletes the files.

use anyhow::{anyhow, ensure, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hkdf::Hkdf;
use p256::ecdh::EphemeralSecret;
use p256::elliptic_curve::Generate;
use p256::pkcs8::{DecodePublicKey, EncodePublicKey};
use p256::PublicKey;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

pub struct Offer {
    pub id: String,
    secret: EphemeralSecret,
    public_der: Vec<u8>,
}

impl Offer {
    pub fn new() -> Self {
        let mut id = [0u8; 9];
        rand::rngs::OsRng.fill_bytes(&mut id);
        let secret = EphemeralSecret::generate();
        let public_der = secret.public_key().to_public_key_der().expect("DER").as_bytes().to_vec();
        Offer { id: URL_SAFE_NO_PAD.encode(id), secret, public_der }
    }

    /// The text that goes in the QR code.
    pub fn code(&self) -> String {
        format!("memi:pair:1:{}:{}", self.id, URL_SAFE_NO_PAD.encode(&self.public_der))
    }

    pub fn file_name(&self) -> String {
        format!("memi-pair-{}.bin", self.id)
    }

    /// The join request left in Drive for devices without a camera. `device` is shown to them,
    /// e.g. "Windows computer".
    pub fn join_file_name(&self) -> String {
        format!("{JOIN_PREFIX}{}.json", self.id)
    }

    pub fn join_request(&self, device: &str, now: i64) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({ "code": self.code(), "device": device, "at": now })).expect("JSON")
    }

    pub fn confirm_code(&self) -> String { confirm_code(&self.code()) }

    /// Opens what the phone sent: the sync key.
    pub fn open(&self, blob: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
        use aes_gcm::aead::{Aead, KeyInit};
        ensure!(blob.len() > 2, "Too short");
        let len = u16::from_be_bytes([blob[0], blob[1]]) as usize;
        ensure!(blob.len() > 2 + len + 12, "Too short");
        let phone = PublicKey::from_public_key_der(&blob[2..2 + len]).map_err(|_| anyhow!("Bad key"))?;
        let iv = &blob[2 + len..2 + len + 12];
        let shared = self.secret.diffie_hellman(&phone);
        let hk = Hkdf::<Sha256>::new(Some(&[0u8; 32]), shared.raw_secret_bytes());
        let mut aes = Zeroizing::new([0u8; 32]);
        hk.expand(format!("memi-pair-v1:{}", self.id).as_bytes(), &mut *aes).map_err(|_| anyhow!("HKDF"))?;
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&*aes).map_err(|_| anyhow!("key"))?;
        let plain = Zeroizing::new(cipher.decrypt(&aes_gcm::Nonce::try_from(iv).map_err(|_| anyhow!("bad nonce"))?, &blob[2 + len + 12..]).map_err(|_| anyhow!("Couldn't open the key"))?);
        ensure!(plain.len() == 32, "Wrong key size");
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&plain);
        Ok(key)
    }
}

pub const JOIN_PREFIX: &str = "memi-join-";
/// Join requests older than this are ignored and cleaned up.
pub const JOIN_MINUTES: i64 = 15;

/// A code someone offered: read from a QR code or a join request.
pub struct Scanned {
    pub id: String,
    pub code: String,
    key: PublicKey,
}

impl Scanned {
    pub fn file_name(&self) -> String { format!("memi-pair-{}.bin", self.id) }
    pub fn confirm_code(&self) -> String { confirm_code(&self.code) }
}

/// Reads a pairing code, or None if it isn't one.
pub fn parse(code: &str) -> Option<Scanned> {
    let rest = code.trim().strip_prefix("memi:pair:1:")?;
    let (id, key) = rest.split_once(':')?;
    let der = URL_SAFE_NO_PAD.decode(key).ok()?;
    let key = PublicKey::from_public_key_der(&der).ok()?;
    Some(Scanned { id: id.to_string(), code: code.trim().to_string(), key })
}

/// A join request found in Drive.
pub struct JoinRequest {
    pub scanned: Scanned,
    pub device: String,
    pub at: i64,
}

pub fn parse_join_request(bytes: &[u8]) -> Option<JoinRequest> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let scanned = parse(v.get("code")?.as_str()?)?;
    let device = v.get("device").and_then(|d| d.as_str()).unwrap_or("A new device").chars().take(60).collect();
    Some(JoinRequest { scanned, device, at: v.get("at").and_then(|a| a.as_i64()).unwrap_or(0) })
}

/// Six digits both devices show, so you can check you're handing the key to the right one.
pub fn confirm_code(code: &str) -> String {
    use sha2::Digest;
    let h = Sha256::digest(code.trim().as_bytes());
    let n = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

/// On a device that has the key: encrypts `sync_key` so only the device that offered the code can read it.
pub fn seal_for(to: &Scanned, sync_key: &[u8; 32]) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    let mine = EphemeralSecret::generate();
    let my_der = mine.public_key().to_public_key_der().map_err(|_| anyhow!("DER"))?.as_bytes().to_vec();
    let shared = mine.diffie_hellman(&to.key);
    let hk = Hkdf::<Sha256>::new(Some(&[0u8; 32]), shared.raw_secret_bytes());
    let mut aes = Zeroizing::new([0u8; 32]);
    hk.expand(format!("memi-pair-v1:{}", to.id).as_bytes(), &mut *aes).map_err(|_| anyhow!("HKDF"))?;
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(&*aes).map_err(|_| anyhow!("key"))?;
    let mut iv = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut iv);
    let sealed = cipher.encrypt(&aes_gcm::Nonce::try_from(&iv[..]).map_err(|_| anyhow!("bad nonce"))?, sync_key.as_slice()).map_err(|_| anyhow!("encrypt"))?;
    let mut out = Vec::with_capacity(2 + my_der.len() + 12 + sealed.len());
    out.extend_from_slice(&(my_der.len() as u16).to_be_bytes());
    out.extend_from_slice(&my_der);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&sealed);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_handed_over_opens_only_for_the_offer() {
        let key = crate::crypto::new_key();
        let offer = Offer::new();
        let req = parse_join_request(&offer.join_request("Web browser", 1)).unwrap();
        assert_eq!(req.device, "Web browser");
        assert_eq!(req.scanned.confirm_code(), offer.confirm_code());
        assert_eq!(offer.confirm_code().len(), 7);
        let sealed = seal_for(&req.scanned, &key).unwrap();
        assert_eq!(*offer.open(&sealed).unwrap(), *key);
        assert!(Offer::new().open(&sealed).is_err(), "another device can't open it");
        assert!(parse("memi:pair:1:nope").is_none());
    }

    /// Runs the Android app's code (as Java) against ours: `cargo test -- --ignored interop`.
    #[test]
    #[ignore]
    fn interop_with_android() {
        use crate::crypto;
        let dir = std::env::temp_dir().join(format!("memi-interop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key = crypto::new_key();
        std::fs::write(dir.join("key.bin"), &*key).unwrap();
        std::fs::write(dir.join("backup.json"), br#"{"format":1,"people":[]}"#).unwrap();
        std::fs::write(dir.join("from-desktop.bin"), crypto::seal(&key, b"from the computer").unwrap()).unwrap();
        let offer = Offer::new();
        std::fs::write(dir.join("code.txt"), offer.code()).unwrap();
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/interop/Phone.java");
        let ok = std::process::Command::new("java").arg(src).arg(&dir).status().expect("java").success();
        assert!(ok);
        assert_eq!(crypto::open(&key, &std::fs::read(dir.join("from-phone.bin")).unwrap()).unwrap(), br#"{"format":1,"people":[]}"#);
        assert_eq!(std::fs::read(dir.join("desktop-opened.json")).unwrap(), b"from the computer");
        let got = offer.open(&std::fs::read(dir.join("pair.bin")).unwrap()).unwrap();
        assert_eq!(*got, *key);
        std::fs::remove_dir_all(dir).ok();
    }
}
