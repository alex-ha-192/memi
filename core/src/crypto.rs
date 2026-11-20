//! End-to-end encryption for sync, byte-for-byte compatible with the Android app's Crypto.kt.
//!
//! A random AES-256 key is made on the first device that turns sync on. Every backup is sealed
//! with AES-GCM before it leaves the device, so Google Drive only ever stores ciphertext.
//!
//! File layout: "MEMI1" | 16 reserved bytes | 12-byte IV | AES-GCM(gzip(json)) with a 128-bit tag,
//! with "MEMI1" as additional authenticated data.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, ensure, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use rand::RngCore;
use std::io::{Read, Write};
use zeroize::Zeroizing;

const MAGIC: &[u8] = b"MEMI1";
const SALT: usize = 16;
const IV: usize = 12;
/// A backup is a few megabytes at most; refuse to inflate anything enormous.
const MAX_PLAIN: u64 = 256 * 1024 * 1024;

pub type Key = Zeroizing<[u8; 32]>;

#[derive(Debug)]
pub struct WrongKey;
impl std::fmt::Display for WrongKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("This key doesn't open the backup.") }
}
impl std::error::Error for WrongKey {}

pub fn new_key() -> Key {
    let mut k = Zeroizing::new([0u8; 32]);
    rand::rngs::OsRng.fill_bytes(&mut *k);
    k
}

/// The key as text, for the recovery key: 8 groups of 8 hex digits.
pub fn key_to_text(key: &[u8; 32]) -> String {
    let text = Zeroizing::new(hex::encode(key));
    text.as_bytes().chunks(8).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("-")
}

/// Reads a recovery key, ignoring dashes, spaces and case.
pub fn key_from_text(text: &str) -> Option<Key> {
    let digits = Zeroizing::new(text.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    let mut k = Zeroizing::new([0u8; 32]);
    hex::decode_to_slice(digits.as_bytes(), &mut *k).ok()?;
    Some(k)
}

pub fn is_backup(blob: &[u8]) -> bool {
    blob.len() > MAGIC.len() + SALT + IV && &blob[..MAGIC.len()] == MAGIC
}

pub fn seal(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>> {
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(plain)?;
    let packed = gz.finish()?;
    let mut iv = [0u8; IV];
    rand::rngs::OsRng.fill_bytes(&mut iv);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("bad key"))?;
    let sealed = cipher.encrypt(&Nonce::try_from(&iv[..]).map_err(|_| anyhow!("bad nonce"))?, Payload { msg: &packed, aad: MAGIC }).map_err(|_| anyhow!("encryption failed"))?;
    let mut out = Vec::with_capacity(MAGIC.len() + SALT + IV + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&[0u8; SALT]);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&sealed);
    Ok(out)
}

pub fn open(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    ensure!(is_backup(blob), "This isn't a Your Memi backup.");
    let iv_start = MAGIC.len() + SALT;
    let iv = &blob[iv_start..iv_start + IV];
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("bad key"))?;
    let packed = cipher
        .decrypt(&Nonce::try_from(iv).map_err(|_| anyhow!("bad nonce"))?, Payload { msg: &blob[iv_start + IV..], aad: MAGIC })
        .map_err(|_| WrongKey)?;
    let mut plain = Vec::new();
    GzDecoder::new(&packed[..]).take(MAX_PLAIN + 1).read_to_end(&mut plain)?;
    ensure!((plain.len() as u64) <= MAX_PLAIN, "Backup is too large.");
    Ok(plain)
}

#[cfg(test)]
mod tests {
    #[test]
    fn recovery_key_text_round_trip() {
        let k = super::new_key();
        let text = super::key_to_text(&k);
        assert_eq!(text.len(), 64 + 7);
        assert_eq!(*super::key_from_text(&text).unwrap(), *k);
        assert_eq!(*super::key_from_text(&text.to_uppercase().replace('-', " ")).unwrap(), *k, "case and separators don't matter");
        assert!(super::key_from_text("abc").is_none());
        assert!(super::key_from_text(&"g".repeat(64)).is_none());
    }

    use super::*;

    #[test]
    fn round_trip_and_wrong_key() {
        let k = new_key();
        let blob = seal(&k, b"hello").unwrap();
        assert_eq!(open(&k, &blob).unwrap(), b"hello");
        let other = new_key();
        assert!(open(&other, &blob).unwrap_err().downcast_ref::<WrongKey>().is_some());
    }

    #[test]
    fn recovery_key_text() {
        let k = new_key();
        let t = key_to_text(&k);
        assert_eq!(t.len(), 64 + 7);
        assert_eq!(*key_from_text(&t.to_uppercase()).unwrap(), *k);
        assert!(key_from_text("not a key").is_none());
    }
}
