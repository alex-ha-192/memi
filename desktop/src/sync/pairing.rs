//! The computer's half of QR pairing; the phone's half is the Android app's Pairing.kt, and the
//! two must stay byte-for-byte compatible.
//!
//! 1. The computer makes a one-off P-256 key pair and shows its public half (X.509 DER, base64url)
//!    in a QR code: `memi:pair:1:<id>:<public key>`.
//! 2. The phone agrees a secret with it (ECDH, then HKDF-SHA256 with info "memi-pair-v1:<id>"),
//!    encrypts the sync key with AES-GCM and leaves it in the Drive app folder as
//!    `memi-pair-<id>.bin`: u16 length | phone public key | 12-byte IV | ciphertext.
//! 3. The computer opens it, checks the key opens the backup, and deletes the file.

use anyhow::{anyhow, ensure, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hkdf::Hkdf;
use p256::ecdh::EphemeralSecret;
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
        let secret = EphemeralSecret::random(&mut rand::rngs::OsRng);
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
        let plain = Zeroizing::new(cipher.decrypt(aes_gcm::Nonce::from_slice(iv), &blob[2 + len + 12..]).map_err(|_| anyhow!("Couldn't open the key"))?);
        ensure!(plain.len() == 32, "Wrong key size");
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&plain);
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the Android app's code (as Java) against ours: `cargo test -- --ignored interop`.
    #[test]
    #[ignore]
    fn interop_with_android() {
        use crate::sync::crypto;
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
