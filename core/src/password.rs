//! Password handling — key derivation and verification.
//!
//! Uses Argon2id to derive a 256-bit master key from the user's password.
//! A SHA-256 quick verifier is stored alongside the encrypted data so that
//! wrong passwords can be rejected quickly without running the expensive
//! Argon2 derivation every time.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::crypto::{SecretKey, KEY_SIZE, SALT_SIZE};
use crate::error::{Pw4Error, Pw4Result};

/// Minimum password length.
pub const MIN_PASSWORD_LENGTH: usize = 6;

/// Maximum failed attempts before permanent lockout.
pub const MAX_ATTEMPTS: u32 = 10;

/// Argon2id memory size in KiB (64 MB).
const ARGON2_MEMORY: u32 = 65536;

/// Argon2id iteration count.
const ARGON2_ITERATIONS: u32 = 3;

/// Argon2id parallelism degree.
const ARGON2_PARALLELISM: u32 = 4;

/// Fixed password stored in encrypted form (Argon2 hash).
///
/// The password itself is never stored; only the Argon2id hash
/// is kept for verification purposes.
#[derive(Clone)]
pub struct FixedPassword {
    password_hash: String,
}

impl Zeroize for FixedPassword {
    fn zeroize(&mut self) {
        self.password_hash.zeroize();
    }
}

impl Drop for FixedPassword {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl FixedPassword {
    /// Create a new fixed password from plaintext.
    ///
    /// The password is hashed with Argon2id and only the hash is stored.
    pub fn new(password: &str) -> Pw4Result<Self> {
        if password.len() < MIN_PASSWORD_LENGTH {
            return Err(Pw4Error::PasswordTooShort(MIN_PASSWORD_LENGTH));
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Pw4Error::KeyDerivation(format!("Failed to hash password: {}", e)))?
            .to_string();

        Ok(Self { password_hash: hash })
    }

    /// Verify that the given password matches the stored hash.
    pub fn verify(&self, password: &str) -> bool {
        if let Ok(parsed_hash) = PasswordHash::new(&self.password_hash) {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok()
        } else {
            false
        }
    }

    /// Get the password hash string (for serialization to config).
    pub fn hash_string(&self) -> &str {
        &self.password_hash
    }

    /// Create from an existing hash string (for deserialization from config).
    pub fn from_hash(hash: String) -> Self {
        Self { password_hash: hash }
    }
}

/// Derive a 256-bit master key from a password using Argon2id.
///
/// Uses memory-hard Argon2id (64MB, 3 iterations, 4 parallelism)
/// to resist GPU/ASIC brute-force attacks.
pub fn derive_master_key(
    password: &str,
    salt: &[u8; SALT_SIZE],
) -> Pw4Result<SecretKey> {
    let mut key_bytes = [0u8; KEY_SIZE];

    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(
            ARGON2_MEMORY,
            ARGON2_ITERATIONS,
            ARGON2_PARALLELISM,
            Some(KEY_SIZE),
        )
        .map_err(|e| Pw4Error::KeyDerivation(format!("Argon2 params: {}", e)))?,
    );

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key_bytes)
        .map_err(|e| Pw4Error::KeyDerivation(format!("Argon2 hash failed: {}", e)))?;

    Ok(SecretKey::from_bytes(key_bytes))
}

/// Quick SHA-256 hash for password verification without full Argon2 derivation.
///
/// This is stored in `.pw4lock` alongside the salt so that wrong passwords
/// can be rejected quickly. The actual master key still requires Argon2,
/// so an attacker who recovers the SHA-256 verifier still needs to run
/// Argon2 to get the decryption key.
pub fn quick_password_hash(password: &str, salt: &[u8; SALT_SIZE]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt);
    hasher.update(b"pw4you-verifier-v2");
    hasher.finalize().to_vec()
}

/// Verify a password attempt using the quick verifier.
///
/// Uses constant-time comparison to prevent timing side-channel attacks.
pub fn verify_password_quick(
    attempted_password: &str,
    salt: &[u8; SALT_SIZE],
    expected_hash: &[u8],
) -> bool {
    let hash = quick_password_hash(attempted_password, salt);
    use subtle::ConstantTimeEq;
    hash.as_slice().ct_eq(expected_hash).into()
}

/// Compute HMAC-SHA256 for lock state signing.
pub fn compute_hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .expect("HMAC key size is fine for SHA-256");
    mac.update(data);
    let result = mac.finalize();
    let mut tag = [0u8; 32];
    tag.copy_from_slice(&result.into_bytes());
    tag
}

// ── Dynamic date-based password (zero user burden) ──────

/// Date format: YYYYMMDD (8 chars).
pub const DATE_LEN: usize = 8;

/// Build the effective password by silently appending the encryption date.
/// The user only enters their master password — the date is automatic.
/// Effective password = master_password + YYYYMMDD
pub fn build_effective_password(master_password: &str, date_str: &str) -> String {
    format!("{}{}", master_password, date_str)
}

/// Get today's date as YYYYMMDD string.
pub fn get_today_date_str() -> String {
    chrono::Local::now().format("%Y%m%d").to_string()
}

/// Get a list of candidate dates to try during trap mode (no stored date).
/// Includes today, yesterday, and tomorrow for timezone edge cases.
pub fn date_candidates() -> Vec<String> {
    let today = chrono::Local::now();
    let mut candidates = Vec::new();
    candidates.push(today.format("%Y%m%d").to_string());
    // Yesterday
    let yesterday = today - chrono::Duration::days(1);
    candidates.push(yesterday.format("%Y%m%d").to_string());
    // Tomorrow
    let tomorrow = today + chrono::Duration::days(1);
    candidates.push(tomorrow.format("%Y%m%d").to_string());
    candidates
}

/// Try a list of date candidates against a stored verifier.
/// Returns the matching date string, or None if none match.
pub fn try_date_candidates(
    master_password: &str,
    salt: &[u8; SALT_SIZE],
    verifier: &[u8],
    dates: &[String],
) -> Option<String> {
    for date in dates {
        let effective = build_effective_password(master_password, date);
        if verify_password_quick(&effective, salt, verifier) {
            return Some(date.clone());
        }
    }
    None
}

/// Obfuscate a date string for storage (simple XOR with fixed key).
pub fn obfuscate_date(date_str: &str) -> String {
    let key = b"pw4you-date-obfuscation-v2";
    let bytes: Vec<u8> = date_str.as_bytes().iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect();
    crate::crypto::to_base64(&bytes)
}

/// De-obfuscate a stored date string.
pub fn deobfuscate_date(obfuscated: &str) -> Option<String> {
    let bytes = crate::crypto::from_base64(obfuscated).ok()?;
    let key = b"pw4you-date-obfuscation-v2";
    let date: Vec<u8> = bytes.iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect();
    String::from_utf8(date).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::generate_salt;

    #[test]
    fn test_fixed_password_create_and_verify() {
        let fp = FixedPassword::new("mysecret").unwrap();
        assert!(fp.verify("mysecret"));
        assert!(!fp.verify("wrongpass"));
    }

    #[test]
    fn test_password_too_short() {
        let result = FixedPassword::new("abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_min_password_length_ok() {
        let result = FixedPassword::new("123456");
        assert!(result.is_ok());
    }

    #[test]
    fn test_derive_master_key_deterministic() {
        let salt = generate_salt();
        let key1 = derive_master_key("testpass", &salt).unwrap();
        let key2 = derive_master_key("testpass", &salt).unwrap();
        assert_eq!(key1.as_bytes(), key2.as_bytes());

        // Different password gives different key
        let key3 = derive_master_key("otherpass", &salt).unwrap();
        assert_ne!(key1.as_bytes(), key3.as_bytes());
    }

    #[test]
    fn test_quick_password_verifier() {
        let salt = generate_salt();
        let hash = quick_password_hash("mypass", &salt);
        assert!(verify_password_quick("mypass", &salt, &hash));
        assert!(!verify_password_quick("wrongpass", &salt, &hash));
    }

    #[test]
    fn test_hash_string_roundtrip() {
        let fp = FixedPassword::new("secure123").unwrap();
        let hash_str = fp.hash_string().to_string();
        let fp2 = FixedPassword::from_hash(hash_str);
        assert!(fp2.verify("secure123"));
    }
}
