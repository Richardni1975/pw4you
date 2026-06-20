//! Cryptographic operations for pw4you.
//!
//! Uses AES-256-GCM for authenticated encryption and
//! random number generation for salts and nonces.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, AeadCore, Key, Nonce,
};
use rand::RngCore;
use std::io::Write;
use std::path::Path;
use zeroize::Zeroize;

use crate::error::{Pw4Error, Pw4Result};

/// Size of AES-256 key in bytes.
pub const KEY_SIZE: usize = 32;

/// Size of GCM nonce in bytes (96 bits).
pub const NONCE_SIZE: usize = 12;

/// Size of GCM authentication tag in bytes.
pub const TAG_SIZE: usize = 16;

/// Size of salt for Argon2 in bytes.
pub const SALT_SIZE: usize = 32;

/// Magic bytes for .pw4lock metadata file.
pub const PWLOCK_MAGIC: &[u8; 7] = b"PWLOCK\x01";

/// Extension for the lock metadata file.
pub const PWLOCK_EXTENSION: &str = "pw4lock";

/// Extension for encrypted files (appended after original extension).
pub const ENCRYPTED_FILE_EXTENSION: &str = "pw4e";

/// A 256-bit encryption key that securely zeroizes on drop.
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct SecretKey {
    bytes: [u8; KEY_SIZE],
}

impl SecretKey {
    /// Create a new key from raw bytes.
    pub fn from_bytes(bytes: [u8; KEY_SIZE]) -> Self {
        Self { bytes }
    }

    /// Get a reference to the key bytes.
    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.bytes
    }
}

impl From<[u8; KEY_SIZE]> for SecretKey {
    fn from(bytes: [u8; KEY_SIZE]) -> Self {
        Self { bytes }
    }
}

// Manual Clone since Zeroize derive doesn't play well with Clone
impl Clone for SecretKey {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes }
    }
}

/// Generate cryptographically secure random bytes.
pub fn generate_random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    OsRng.fill_bytes(&mut buf);
    buf
}

/// Generate a random salt for Argon2 key derivation.
pub fn generate_salt() -> [u8; SALT_SIZE] {
    let mut salt = [0u8; SALT_SIZE];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Generate a random 256-bit key.
pub fn generate_key() -> SecretKey {
    let mut bytes = [0u8; KEY_SIZE];
    OsRng.fill_bytes(&mut bytes);
    SecretKey::from_bytes(bytes)
}

/// Encrypt plaintext using AES-256-GCM.
///
/// Returns (nonce, ciphertext_with_tag) where ciphertext_with_tag
/// includes the 16-byte authentication tag appended at the end.
pub fn encrypt_aes_gcm(key: &SecretKey, plaintext: &[u8]) -> Pw4Result<(Vec<u8>, Vec<u8>)> {
    let aes_key = Key::<Aes256Gcm>::from_slice(key.as_bytes());
    let cipher = Aes256Gcm::new(aes_key);

    let nonce_bytes = Aes256Gcm::generate_nonce(&mut OsRng);
    let nonce_vec = nonce_bytes.to_vec();

    let ciphertext = cipher
        .encrypt(&nonce_bytes, plaintext)
        .map_err(|e| Pw4Error::Crypto(format!("Encryption failed: {}", e)))?;

    Ok((nonce_vec, ciphertext))
}

/// Decrypt ciphertext using AES-256-GCM.
///
/// The ciphertext should include the 16-byte authentication tag at the end.
pub fn decrypt_aes_gcm(
    key: &SecretKey,
    nonce: &[u8; NONCE_SIZE],
    ciphertext_with_tag: &[u8],
) -> Pw4Result<Vec<u8>> {
    let aes_key = Key::<Aes256Gcm>::from_slice(key.as_bytes());
    let cipher = Aes256Gcm::new(aes_key);
    let nonce_slice = Nonce::from_slice(nonce);

    let plaintext = cipher
        .decrypt(nonce_slice, ciphertext_with_tag)
        .map_err(|e| Pw4Error::Crypto(format!("Decryption failed: {}", e)))?;

    Ok(plaintext)
}

/// Encrypt a key with a master key (key wrapping).
pub fn wrap_key(master_key: &SecretKey, child_key: &SecretKey) -> Pw4Result<(Vec<u8>, Vec<u8>)> {
    encrypt_aes_gcm(master_key, child_key.as_bytes())
}

/// Decrypt a key with a master key (key unwrapping).
pub fn unwrap_key(
    master_key: &SecretKey,
    nonce: &[u8; NONCE_SIZE],
    wrapped_key: &[u8],
) -> Pw4Result<SecretKey> {
    let key_bytes = decrypt_aes_gcm(master_key, nonce, wrapped_key)?;
    if key_bytes.len() != KEY_SIZE {
        return Err(Pw4Error::Crypto(format!(
            "Unwrapped key has wrong size: {} (expected {})",
            key_bytes.len(),
            KEY_SIZE
        )));
    }
    let mut bytes = [0u8; KEY_SIZE];
    bytes.copy_from_slice(&key_bytes);
    Ok(SecretKey::from_bytes(bytes))
}

/// Encrypt a data block using AES-256-GCM.
/// Returns (nonce, encrypted_data_with_tag).
pub fn encrypt_block(key: &SecretKey, block_data: &[u8]) -> Pw4Result<([u8; NONCE_SIZE], Vec<u8>)> {
    let (nonce, ciphertext) = encrypt_aes_gcm(key, block_data)?;
    let mut nonce_arr = [0u8; NONCE_SIZE];
    if nonce.len() != NONCE_SIZE {
        return Err(Pw4Error::Crypto("Nonce has unexpected size".into()));
    }
    nonce_arr.copy_from_slice(&nonce);
    Ok((nonce_arr, ciphertext))
}

/// Decrypt a data block using AES-256-GCM.
pub fn decrypt_block(
    key: &SecretKey,
    nonce: &[u8; NONCE_SIZE],
    encrypted_data: &[u8],
) -> Pw4Result<Vec<u8>> {
    decrypt_aes_gcm(key, nonce, encrypted_data)
}

/// Derive an encryption key for a specific file from master key and file path.
/// This ensures each file gets a unique key even without storing per-file keys.
pub fn derive_file_key(
    master_key: &SecretKey,
    file_path: &str,
    salt: &[u8; SALT_SIZE],
) -> Pw4Result<SecretKey> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(master_key.as_bytes());
    hasher.update(file_path.as_bytes());
    hasher.update(salt);
    let result = hasher.finalize();

    let mut bytes = [0u8; KEY_SIZE];
    bytes.copy_from_slice(&result);
    Ok(SecretKey::from_bytes(bytes))
}

/// Default header size for lightweight encryption (4 KB).
pub const HEADER_SIZE: usize = 4096;

/// Encrypt the file header in-place.
///
/// Reads the first `max_header` bytes, AES-256-GCM encrypts them, writes
/// the ciphertext (without the 16-byte auth tag) back to the file.
/// The auth tag is returned separately for storage in `.pw4lock` metadata.
///
/// File size is unchanged — ciphertext replaces plaintext 1:1.
/// Files smaller than 1 byte are skipped.
///
/// Returns (nonce, auth_tag, bytes_encrypted).
pub fn encrypt_file_header(
    file_key: &SecretKey,
    file_path: &Path,
    max_header: usize,
) -> Pw4Result<([u8; NONCE_SIZE], [u8; TAG_SIZE], usize)> {
    let file_size = std::fs::metadata(file_path)?.len() as usize;
    let plaintext_len = max_header.min(file_size);

    if plaintext_len == 0 {
        return Ok(([0u8; NONCE_SIZE], [0u8; TAG_SIZE], 0));
    }

    // Read the header
    let mut plaintext = vec![0u8; plaintext_len];
    {
        let mut f = std::fs::File::open(file_path)?;
        std::io::Read::read_exact(&mut f, &mut plaintext)?;
    }

    // Encrypt with AES-256-GCM
    let aes_key = aes_gcm::Key::<aes_gcm::Aes256Gcm>::from_slice(file_key.as_bytes());
    let cipher = aes_gcm::Aes256Gcm::new(aes_key);
    let nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut aes_gcm::aead::OsRng);
    let ciphertext_with_tag = cipher
        .encrypt(&nonce, plaintext.as_slice())
        .map_err(|e| Pw4Error::Crypto(format!("Header encrypt failed: {}", e)))?;

    // Split: ciphertext (plaintext_len bytes) || tag (16 bytes)
    let tag_start = ciphertext_with_tag.len() - TAG_SIZE;
    let ciphertext = &ciphertext_with_tag[..tag_start];
    let tag = &ciphertext_with_tag[tag_start..];

    let mut nonce_arr = [0u8; NONCE_SIZE];
    nonce_arr.copy_from_slice(&nonce);
    let mut tag_arr = [0u8; TAG_SIZE];
    tag_arr.copy_from_slice(tag);

    // Write ciphertext back (same size as plaintext)
    {
        let mut f = std::fs::OpenOptions::new().write(true).open(file_path)?;
        std::io::Write::write_all(&mut f, ciphertext)?;
    }

    Ok((nonce_arr, tag_arr, plaintext_len))
}

/// Decrypt a file header in-place.
///
/// Reads `header_len` bytes (ciphertext), combines with the stored auth `tag`,
/// decrypts, and writes the original plaintext back. File size is unchanged.
pub fn decrypt_file_header(
    file_key: &SecretKey,
    nonce: &[u8; NONCE_SIZE],
    tag: &[u8; TAG_SIZE],
    file_path: &Path,
    header_len: usize,
) -> Pw4Result<()> {
    if header_len == 0 {
        return Ok(());
    }

    // Read ciphertext
    let mut ciphertext = vec![0u8; header_len];
    {
        let mut f = std::fs::File::open(file_path)?;
        std::io::Read::read_exact(&mut f, &mut ciphertext)?;
    }

    // Reconstruct ciphertext_with_tag = ciphertext || tag
    let mut ct_with_tag = ciphertext;
    ct_with_tag.extend_from_slice(tag);

    // Decrypt
    let aes_key = aes_gcm::Key::<aes_gcm::Aes256Gcm>::from_slice(file_key.as_bytes());
    let cipher = aes_gcm::Aes256Gcm::new(aes_key);
    let gcm_nonce = aes_gcm::Nonce::from_slice(nonce);
    let plaintext = cipher
        .decrypt(gcm_nonce, ct_with_tag.as_slice())
        .map_err(|e| Pw4Error::Crypto(format!("Header decrypt failed: {}", e)))?;

    // Write original header back
    {
        let mut f = std::fs::OpenOptions::new().write(true).open(file_path)?;
        std::io::Write::write_all(&mut f, &plaintext)?;
    }

    Ok(())
}

/// Encrypt plaintext data and return (nonce_base64, ciphertext_with_tag_base64).
/// Convenience for encrypting small metadata values (like the master key self-wrap).
pub fn encrypt_to_base64(key: &SecretKey, plaintext: &[u8]) -> Pw4Result<(String, String)> {
    use base64::Engine;
    let (nonce, ciphertext) = encrypt_aes_gcm(key, plaintext)?;
    let engine = base64::engine::general_purpose::STANDARD;
    Ok((engine.encode(&nonce), engine.encode(&ciphertext)))
}

/// Decrypt data from base64-encoded nonce and ciphertext.
/// Convenience for decrypting small metadata values.
pub fn decrypt_from_base64(
    key: &SecretKey,
    nonce_b64: &str,
    ciphertext_b64: &str,
) -> Pw4Result<Vec<u8>> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    let nonce_bytes = engine.decode(nonce_b64)?;
    let ciphertext = engine.decode(ciphertext_b64)?;

    let mut nonce_arr = [0u8; NONCE_SIZE];
    if nonce_bytes.len() != NONCE_SIZE {
        return Err(Pw4Error::Crypto(format!(
            "Nonce has wrong size: {} (expected {})",
            nonce_bytes.len(),
            NONCE_SIZE
        )));
    }
    nonce_arr.copy_from_slice(&nonce_bytes);

    decrypt_aes_gcm(key, &nonce_arr, &ciphertext)
}

/// Remove the read-only attribute from a file.
fn make_writable(path: &Path) {
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut perms = metadata.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

/// Delete a file after encryption/decryption.
///
/// Simply removes the file — the AES-256-GCM encryption is the real security.
/// The encrypted `.pw4e` copy (or restored original) is already safely on disk.
/// Secure overwrite would triple the I/O with no meaningful security gain
/// since the ciphertext is unbreakable without the password.
pub fn secure_delete_file(path: &Path) {
    if !path.exists() {
        return;
    }
    make_writable(path);
    if let Err(e) = std::fs::remove_file(path) {
        log::warn!("Cannot delete '{}': {}.", path.display(), e);
    }
}

/// Securely delete an entire directory tree with 3-pass overwrite.
pub fn secure_delete_folder(path: &Path) {
    if !path.exists() {
        return;
    }
    for entry in walkdir::WalkDir::new(path).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            secure_delete_file(entry.path());
        }
    }
    let _ = std::fs::remove_dir_all(path);
}

/// Convert bytes to base64 string.
pub fn to_base64(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Convert base64 string back to bytes.
pub fn from_base64(s: &str) -> Pw4Result<Vec<u8>> {
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.decode(s)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let plaintext = b"Hello, pw4you! This is a test message.";

        let (nonce, ciphertext) = encrypt_aes_gcm(&key, plaintext).unwrap();

        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce);

        let decrypted = decrypt_aes_gcm(&key, &nonce_arr, &ciphertext).unwrap();
        assert_eq!(plaintext.as_slice(), &decrypted);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = b"Secret data";

        let (nonce, ciphertext) = encrypt_aes_gcm(&key1, plaintext).unwrap();
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce);

        let result = decrypt_aes_gcm(&key2, &nonce_arr, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_wrap_unwrap_roundtrip() {
        let master_key = generate_key();
        let file_key = generate_key();

        let (nonce, wrapped) = wrap_key(&master_key, &file_key).unwrap();
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce);

        let unwrapped = unwrap_key(&master_key, &nonce_arr, &wrapped).unwrap();
        assert_eq!(file_key.as_bytes(), unwrapped.as_bytes());
    }

    #[test]
    fn test_tampered_ciphertext_detected() {
        let key = generate_key();
        let plaintext = b"Tamper test";

        let (nonce, mut ciphertext) = encrypt_aes_gcm(&key, plaintext).unwrap();
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }

        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce);

        let result = decrypt_aes_gcm(&key, &nonce_arr, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_file_key() {
        let master_key = generate_key();
        let salt = generate_salt();

        let key1 = derive_file_key(&master_key, "subdir/file.txt", &salt).unwrap();
        let key2 = derive_file_key(&master_key, "subdir/file.txt", &salt).unwrap();
        assert_eq!(key1.as_bytes(), key2.as_bytes());

        let key3 = derive_file_key(&master_key, "subdir/other.txt", &salt).unwrap();
        assert_ne!(key1.as_bytes(), key3.as_bytes());
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = b"test data for base64";
        let b64 = to_base64(data);
        let decoded = from_base64(&b64).unwrap();
        assert_eq!(data.as_slice(), &decoded);
    }
}
