//! pw4you-core — In-place folder encryption library.
//!
//! Provides the core functionality for encrypting and decrypting
//! native folders directly on disk using AES-256-GCM and Argon2id.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  High-Level API                                          │
//! │  encrypt_folder / unlock_folder / decrypt_folder         │
//! │  lock_folder / reset_password / probe_folder_status      │
//! ├─────────────────────────────────────────────────────────┤
//! │  In-Place Engine (inplace.rs)                            │
//! │  EncryptedFolder, .pw4lock metadata, file manifest       │
//! ├────────────────┬──────────────────┬─────────────────────┤
//! │  crypto.rs     │  password.rs     │  lock.rs            │
//! │  AES-256-GCM   │  Argon2id key   │  Per-folder anti-    │
//! │  key wrap      │  derivation      │  tamper lock state  │
//! ├────────────────┼──────────────────┼─────────────────────┤
//! │  config.rs     │  email.rs        │                     │
//! │  AppData       │  SMTP password   │                     │
//! │  config mgmt   │  recovery codes  │                     │
//! └────────────────┴──────────────────┴─────────────────────┘
//! ```

pub mod config;
pub mod crypto;
pub mod email;
pub mod error;
pub mod inplace;
pub mod lock;
pub mod password;

// Re-export core types for convenience
pub use config::{AppConfig, AppSettings, EncryptedFolderEntry, SmtpConfig};
pub use crypto::{
    SecretKey, KEY_SIZE, NONCE_SIZE, SALT_SIZE, HEADER_SIZE, PWLOCK_EXTENSION,
};
pub use email::{generate_verification_code, send_verification_email, verify_code, mask_email};
pub use error::{Pw4Error, Pw4Result};
pub use inplace::{
    encrypt_folder, decrypt_folder, lock_folder, open_folder, unlock_folder,
    reset_password, probe_folder_status, recover_v1_folder,
    EncryptedFolder, FolderStatus, UnlockResult, OperationSummary, FolderProbe,
};
pub use lock::{LockEngine, LockState};
pub use password::{
    derive_master_key, quick_password_hash, verify_password_quick, FixedPassword,
    MIN_PASSWORD_LENGTH, MAX_ATTEMPTS,
};
