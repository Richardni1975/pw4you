//! Error types for pw4you-core.

use thiserror::Error;

/// Main error type for the pw4you core library.
#[derive(Error, Debug)]
pub enum Pw4Error {
    /// I/O error from the operating system.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Cryptographic operation failed.
    #[error("Cryptography error: {0}")]
    Crypto(String),

    /// Folder is not encrypted (no .pw4lock found).
    #[error("Folder is not encrypted: {0}")]
    FolderNotEncrypted(String),

    /// Folder is currently locked due to failed attempts.
    #[error("Folder is locked. Remaining: {seconds}s ({days} days)")]
    FolderLocked {
        seconds: i64,
        days: i64,
        attempt_count: u32,
    },

    /// Wrong password provided during unlock.
    #[error("Wrong password. Attempt {attempt}/{max_attempts} — lock duration: {lock_days} days")]
    WrongPassword {
        attempt: u32,
        max_attempts: u32,
        lock_days: u64,
    },

    /// Maximum attempts reached (10+).
    #[error("Maximum attempts reached ({attempt_count}). Vault locked for {lock_days} days.")]
    MaxAttemptsReached {
        attempt_count: u32,
        lock_days: u64,
    },

    /// System clock rollback detected.
    #[error("System clock rollback detected. Lock extended by {penalty_seconds}s as penalty.")]
    ClockRollback { penalty_seconds: i64 },

    /// Lock state integrity check failed (possible tampering).
    #[error("Lock state integrity check failed — possible tampering detected.")]
    LockStateTampered,

    /// Argon2 key derivation error.
    #[error("Key derivation error: {0}")]
    KeyDerivation(String),

    /// Serialization/bincode error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    /// JSON error (for config files and .pw4lock metadata).
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Folder is not unlocked — operation requires unlocked state.
    #[error("Folder is not unlocked")]
    NotUnlocked,

    /// Password is too short.
    #[error("Password must be at least {0} characters long")]
    PasswordTooShort(usize),

    /// Email is not configured (SMTP settings missing).
    #[error("Email is not configured. Please set up SMTP in Settings.")]
    EmailNotConfigured,

    /// SMTP error from lettre.
    #[error("SMTP error: {0}")]
    SmtpError(String),

    /// No email is bound to this app.
    #[error("No security email is bound. Please bind an email in Settings first.")]
    EmailNotBound,

    /// Verification code is invalid.
    #[error("Invalid verification code")]
    InvalidVerificationCode,

    /// Verification code has expired.
    #[error("Verification code has expired. Please request a new one.")]
    VerificationCodeExpired,

    /// General path-related error.
    #[error("Path error: {0}")]
    PathError(String),

    /// Walkdir traversal error.
    #[error("Directory traversal error: {0}")]
    WalkDir(#[from] walkdir::Error),

    /// Base64 decode error.
    #[error("Base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
}

/// Result type alias for pw4you operations.
pub type Pw4Result<T> = Result<T, Pw4Error>;
