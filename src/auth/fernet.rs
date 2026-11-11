//! Pure-Rust Fernet token encryption (byte-compatible with Python's
//! `cryptography.fernet.Fernet`, which the Python server used).
//!
//! Spec: https://github.com/fernet/spec/blob/master/TokenFormat.md
//!
//! token = base64url_nopad( version || timestamp || iv || ciphertext || hmac )
//!   version    = 0x80
//!   timestamp  = u64 big-endian unix seconds
//!   iv         = 16 random bytes
//!   ciphertext = AES-128-CBC(PKCS7(plaintext)) with key = key[16..32]
//!   hmac       = HMAC-SHA256(key = key[0..16], msg = everything above)
//!
//! The 32-byte key layout matches Python Fernet: first 16 bytes sign,
//! last 16 bytes encrypt.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type HmacSha256 = Hmac<Sha256>;

const VERSION_BYTE: u8 = 0x80;
const IV_LEN: usize = 16;
const HMAC_LEN: usize = 32;
const MIN_TOKEN_LEN: usize = 1 + 8 + IV_LEN + HMAC_LEN; // empty ciphertext still has a block

#[derive(Debug, thiserror::Error)]
pub enum FernetError {
    #[error("invalid key length: expected 32 bytes, got {0}")]
    InvalidKeyLength(usize),
    #[error("invalid token: malformed base64")]
    InvalidBase64,
    #[error("invalid token: too short")]
    TooShort,
    #[error("invalid token: unsupported version byte 0x{0:02x}")]
    UnsupportedVersion(u8),
    #[error("invalid token: HMAC mismatch")]
    HmacMismatch,
    #[error("invalid token: decryption failed")]
    DecryptFailed,
}

/// Fernet encryptor/decryptor bound to a 32-byte key.
#[derive(Clone)]
pub struct Fernet {
    hmac_key: [u8; 16],
    enc_key: [u8; 16],
}

impl Fernet {
    /// Create from a URL-safe base64 (no padding) 32-byte key — the exact
    /// format written by `Fernet.generate_key()` / the `.moidify_secret_key` file.
    pub fn new(key_b64: &[u8]) -> Result<Self, FernetError> {
        let raw = URL_SAFE_NO_PAD
            .decode(key_b64)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(key_b64))
            .map_err(|_| FernetError::InvalidBase64)?;
        Self::from_bytes(&raw)
    }

    pub fn from_bytes(raw: &[u8]) -> Result<Self, FernetError> {
        if raw.len() != 32 {
            return Err(FernetError::InvalidKeyLength(raw.len()));
        }
        let mut hmac_key = [0u8; 16];
        let mut enc_key = [0u8; 16];
        hmac_key.copy_from_slice(&raw[0..16]);
        enc_key.copy_from_slice(&raw[16..32]);
        Ok(Self { hmac_key, enc_key })
    }

    /// Generate a new random key, base64url-no-pad encoded (Python-compatible).
    pub fn generate_key() -> Vec<u8> {
        use rand::RngCore;
        let mut raw = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut raw);
        URL_SAFE_NO_PAD.encode(raw).into_bytes()
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Vec<u8> {
        use rand::RngCore;

        let mut iv = [0u8; IV_LEN];
        rand::thread_rng().fill_bytes(&mut iv);

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut ts_bytes = [0u8; 8];
        ts_bytes.copy_from_slice(&ts.to_be_bytes());

        let ciphertext = Aes128CbcEnc::new((&self.enc_key).into(), (&iv).into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext);

        // HMAC covers version || ts || iv || ciphertext
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.hmac_key).expect("hmac key len");
        mac.update(&[VERSION_BYTE]);
        mac.update(&ts_bytes);
        mac.update(&iv);
        mac.update(&ciphertext);
        let hmac = mac.finalize().into_bytes();

        let mut out = Vec::with_capacity(1 + 8 + IV_LEN + ciphertext.len() + HMAC_LEN);
        out.push(VERSION_BYTE);
        out.extend_from_slice(&ts_bytes);
        out.extend_from_slice(&iv);
        out.extend_from_slice(&ciphertext);
        out.extend_from_slice(&hmac);
        URL_SAFE_NO_PAD.encode(out).into_bytes()
    }

    pub fn decrypt(&self, token: &[u8]) -> Result<Vec<u8>, FernetError> {
        let raw = URL_SAFE_NO_PAD
            .decode(token)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(token))
            .map_err(|_| FernetError::InvalidBase64)?;

        if raw.len() < MIN_TOKEN_LEN {
            return Err(FernetError::TooShort);
        }
        if raw[0] != VERSION_BYTE {
            return Err(FernetError::UnsupportedVersion(raw[0]));
        }

        let msg = &raw[..raw.len() - HMAC_LEN];
        let provided_hmac = &raw[raw.len() - HMAC_LEN..];

        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.hmac_key).expect("hmac key len");
        mac.update(msg);
        mac.verify_slice(provided_hmac)
            .map_err(|_| FernetError::HmacMismatch)?;

        let iv = &raw[9..9 + IV_LEN];
        let ciphertext = &raw[9 + IV_LEN..raw.len() - HMAC_LEN];

        Aes128CbcDec::new((&self.enc_key).into(), iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
            .map_err(|_| FernetError::DecryptFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-answer test: encrypt with our impl, decrypt with the same key
    /// (round-trip). Cross-check against Python is done in tests/fernet_compat.rs
    /// when a Python interpreter is available.
    #[test]
    fn roundtrip() {
        let key = Fernet::generate_key();
        let f = Fernet::new(&key).unwrap();
        let ct = f.encrypt(b"hello moidify");
        let pt = f.decrypt(&ct).unwrap();
        assert_eq!(pt, b"hello moidify");
    }

    #[test]
    fn rejects_wrong_key() {
        let k1 = Fernet::generate_key();
        let k2 = Fernet::generate_key();
        let f1 = Fernet::new(&k1).unwrap();
        let f2 = Fernet::new(&k2).unwrap();
        let ct = f1.encrypt(b"secret");
        assert!(f2.decrypt(&ct).is_err());
    }

    #[test]
    fn rejects_tampered_token() {
        let key = Fernet::generate_key();
        let f = Fernet::new(&key).unwrap();
        let mut ct = f.encrypt(b"secret");
        // Flip a character in the middle of the base64 payload.
        let idx = ct.len() / 2;
        ct[idx] = if ct[idx] == b'A' { b'B' } else { b'A' };
        assert!(f.decrypt(&ct).is_err());
    }

    #[test]
    fn accepts_python_standard_base64_keys() {
        // Fernet keys are URL_SAFE_NO_PAD, but be liberal on read.
        let raw = [7u8; 32];
        let std_key = base64::engine::general_purpose::STANDARD.encode(raw);
        assert!(Fernet::new(std_key.as_bytes()).is_ok());
    }
}
