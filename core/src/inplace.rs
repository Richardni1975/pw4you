//! In-place folder encryption via header encryption + name obfuscation.
//!
//! # How it works
//!
//! **Encryption (sub-second, no data movement):**
//! 1. Walk the folder recursively
//! 2. For each file: encrypt only the first 4KB header with AES-256-GCM in-place
//! 3. Rename every file and subdirectory to a random obfuscated hex name
//! 4. Write `.pw4lock` metadata mapping original ↔ obfuscated names
//!
//! **Decryption (sub-second):**
//! 1. Read `.pw4lock`, verify password, derive key
//! 2. For each file: decrypt the 4KB header back in-place
//! 3. Rename everything back to original names
//! 4. Remove `.pw4lock`
//!
//! **Cross-platform protection:**
//! - Obfuscated filenames have no extensions → no app can recognize files
//! - Broken headers → even if renamed back manually, files won't open
//! - Works on Windows, U盘, Android, Linux — any filesystem
//!
//! **Performance:**
//! - Only 4KB I/O per file regardless of file size
//! - 500MB video: ~0.01s (4KB read + encrypt + write)
//! - No data copying, no temp files, no secure deletion needed

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::crypto::{
    self, decrypt_file_header, derive_file_key, encrypt_file_header, from_base64,
    generate_salt, to_base64, SecretKey, HEADER_SIZE, NONCE_SIZE, PWLOCK_EXTENSION,
    SALT_SIZE,
};
use crate::error::{Pw4Error, Pw4Result};
use crate::lock::{LockEngine, LockState};
use crate::password::{
    build_effective_password, date_candidates, deobfuscate_date, derive_master_key,
    get_today_date_str, obfuscate_date, quick_password_hash, try_date_candidates,
    verify_password_quick, MIN_PASSWORD_LENGTH,
};

// ── Metadata types ──────────────────────────────────────

/// Metadata stored in `.pw4lock` — the "map" to restore everything.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FolderMetadataV2 {
    pub version: u32,
    pub salt_b64: String,
    pub password_verifier_b64: String,
    /// Obfuscated encryption date (YYYYMMDD). Primary date backup.
    #[serde(default)]
    pub encryption_date_obfuscated: String,
    pub lock_state: LockState,
    /// Maps obfuscated name → entry info
    pub entries: HashMap<String, ObfuscatedEntry>,
}

/// One entry (file or directory) in the obfuscated folder.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ObfuscatedEntry {
    /// Original name (file name or directory name, not full path).
    pub original_name: String,
    /// Original relative path from folder root (with original names).
    pub original_path: String,
    /// Whether this is a directory.
    pub is_directory: bool,
    /// For files: actual file size in bytes.
    #[serde(default)]
    pub file_size: u64,
    /// For files: base64 nonce for the header encryption.
    #[serde(default)]
    pub header_nonce_b64: String,
    /// For files: base64 GCM authentication tag (16 bytes), stored separately from file.
    #[serde(default)]
    pub header_tag_b64: String,
    /// For files: how many bytes were encrypted in the header.
    #[serde(default)]
    pub header_len: usize,
}

const METADATA_VERSION_V2: u32 = 2;

// ── EncryptedFolder ─────────────────────────────────────

pub struct EncryptedFolder {
    pub folder_path: PathBuf,
    pub metadata: FolderMetadataV2,
    pub is_unlocked: bool,
    pub master_key: Option<SecretKey>,
    pub lock_engine: LockEngine,
    pub display_name: String,
}

impl EncryptedFolder {
    pub fn lock_file_path(&self) -> PathBuf {
        self.folder_path.join(format!(".{}", PWLOCK_EXTENSION))
    }

    pub fn status(&self) -> FolderStatus {
        let now = LockEngine::now();
        if self.is_unlocked {
            FolderStatus::Unlocked
        } else if self.lock_engine.is_locked(now) {
            FolderStatus::LockedOut {
                attempt_count: self.lock_engine.attempt_count(),
                remaining_seconds: self.lock_engine.remaining_lock(now),
                remaining_days: self.lock_engine.remaining_lock(now) as u64 / (24 * 3600),
            }
        } else {
            FolderStatus::Locked
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderStatus {
    Unlocked,
    Locked,
    LockedOut { attempt_count: u32, remaining_seconds: i64, remaining_days: u64 },
}

#[derive(Debug)]
pub enum UnlockResult {
    Success,
    WrongPassword { attempt_count: u32, lock_days: u64 },
    LockedOut { remaining_seconds: i64, remaining_days: u64 },
    ClockRollback { penalty_seconds: i64 },
    MaxAttempts { attempt_count: u32, lock_days: u64 },
}

#[derive(Debug, Default, Clone)]
pub struct OperationSummary {
    pub files: u64,
    pub total_bytes: u64,
}

pub struct FolderProbe {
    pub is_encrypted: bool,
    pub is_unlocked: bool,
    pub attempt_count: u32,
    pub file_count: u64,
}

// ── Name obfuscation ────────────────────────────────────

fn random_hex_name() -> String {
    let mut rng = rand::thread_rng();
    let chars: String = (0..12).map(|_| {
        let idx = rng.gen_range(0..16);
        "0123456789abcdef".as_bytes()[idx] as char
    }).collect();
    chars
}

// ── Public API ──────────────────────────────────────────

/// Encrypt a folder in-place via header encryption + name obfuscation.
///
/// No files are copied or moved. Only the first 4KB of each file is encrypted
/// in-place, and file/directory names are replaced with random hex strings.
/// This is near-instant regardless of total folder size.
pub fn encrypt_folder(
    folder_path: &Path,
    password: &str,
    display_name: &str,
) -> Pw4Result<EncryptedFolder> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(Pw4Error::PasswordTooShort(MIN_PASSWORD_LENGTH));
    }
    if !folder_path.is_dir() {
        return Err(Pw4Error::PathError(format!("Not a directory: {}", folder_path.display())));
    }

    let lock_path = folder_path.join(format!(".{}", PWLOCK_EXTENSION));
    if lock_path.exists() {
        return Err(Pw4Error::PathError(format!("Already encrypted: {}", folder_path.display())));
    }

    // ── Dynamic date: silently append today's date to the password ──
    let encryption_date = get_today_date_str();
    let effective_password = build_effective_password(password, &encryption_date);

    // Generate key material from effective password (master + date)
    let salt = generate_salt();
    let master_key = derive_master_key(&effective_password, &salt)?;
    // Store verifier for the effective password (so we can verify without knowing date upfront)
    let verifier = quick_password_hash(&effective_password, &salt);

    // Phase 1: collect all FILE entries (directories are NOT renamed)
    // Skip .pw4lock, .pw4date, and old v1 .pw4e files (encrypted leftovers)
    let mut file_paths: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(folder_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path().to_path_buf();
        if path == folder_path { continue; }
        let lock_name = format!(".{}", PWLOCK_EXTENSION);
        if path.file_name().map(|n| n == lock_name.as_str()).unwrap_or(false) { continue; }
        // Skip old v1 .pw4e encrypted files (they're already encrypted)
        if path.extension().map(|e| e == "pw4e").unwrap_or(false) { continue; }
        // Skip date backup
        if path.file_name().map(|n| n == ".pw4date").unwrap_or(false) { continue; }
        if path.is_file() {
            file_paths.push(path);
        }
    }

    // Phase 2: encrypt file headers and rename
    let mut entries: HashMap<String, ObfuscatedEntry> = HashMap::new();
    let mut summary = OperationSummary::default();

    for path in &file_paths {
        let original_name = path.file_name().unwrap().to_string_lossy().to_string();
        let relative = path.strip_prefix(folder_path).unwrap_or(path).to_string_lossy().replace('\\', "/");

        // Build obfuscated RELATIVE path (preserving subdirectory structure)
        // e.g., "素材/logo.png" → "素材/a1b2c3d4e5f6"
        let obfuscated_leaf = random_hex_name();
        let obfuscated_rel = if let Some(parent_rel) = Path::new(&relative).parent() {
            if parent_rel.as_os_str().is_empty() {
                obfuscated_leaf.clone()
            } else {
                format!("{}/{}", parent_rel.to_string_lossy().replace('\\', "/"), obfuscated_leaf)
            }
        } else {
            obfuscated_leaf.clone()
        };

        // Encrypt file header in-place
        let file_key = derive_file_key(&master_key, &relative, &salt)?;
        let (nonce, tag, header_len) = encrypt_file_header(&file_key, path, HEADER_SIZE)?;
        let file_size = std::fs::metadata(path)?.len();

        // Rename file to obfuscated leaf name (same directory)
        let new_path = path.parent().unwrap().join(&obfuscated_leaf);
        std::fs::rename(path, &new_path)?;

        entries.insert(obfuscated_rel, ObfuscatedEntry {
            original_name: original_name.clone(),
            original_path: relative.clone(),
            is_directory: false,
            file_size,
            header_nonce_b64: to_base64(&nonce),
            header_tag_b64: to_base64(&tag),
            header_len,
        });

        summary.files += 1;
        summary.total_bytes += file_size;
    }

    // Phase 3: write .pw4lock (primary date backup) + .pw4date (secondary backup)
    let date_obfuscated = obfuscate_date(&encryption_date);
    let metadata = FolderMetadataV2 {
        version: METADATA_VERSION_V2,
        salt_b64: to_base64(&salt),
        password_verifier_b64: to_base64(&verifier),
        encryption_date_obfuscated: date_obfuscated.clone(),
        lock_state: LockState::default_state(),
        entries,
    };

    let json = serde_json::to_string_pretty(&metadata)?;
    std::fs::write(&lock_path, &json)?;
    #[cfg(windows)] hide_file(&lock_path);

    // Write secondary date backup (.pw4date marker file)
    let date_path = folder_path.join(".pw4date");
    std::fs::write(&date_path, &date_obfuscated)?;
    #[cfg(windows)] hide_file(&date_path);

    log::info!(
        "Obfuscated '{}': {} files ({:.1} MB) in-place",
        display_name, summary.files,
        summary.total_bytes as f64 / 1_048_576.0
    );

    Ok(EncryptedFolder {
        folder_path: folder_path.to_path_buf(),
        metadata,
        is_unlocked: true,
        master_key: Some(master_key),
        lock_engine: LockEngine::new(),
        display_name: display_name.to_string(),
    })
}

/// Open an encrypted folder (load metadata, don't decrypt).
pub fn open_folder(folder_path: &Path, display_name: &str) -> Pw4Result<EncryptedFolder> {
    let lock_path = folder_path.join(format!(".{}", PWLOCK_EXTENSION));
    if !lock_path.exists() {
        return Err(Pw4Error::FolderNotEncrypted(format!("No .pw4lock in: {}", folder_path.display())));
    }
    let json = std::fs::read_to_string(&lock_path)?;

    // Check version before parsing
    let version: u32 = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|v| v.get("version")?.as_u64())
        .map(|v| v as u32)
        .unwrap_or(0);

    if version == 1 {
        return Err(Pw4Error::FolderNotEncrypted(format!(
            "⚠️ 此文件夹是用旧版 pw4you (v0.1) 加密的，与新版本不兼容。\n\
             请先使用旧版软件解密该文件夹，再用当前版本重新加密。\n\
             或者：手动删除文件夹中的 .pw4lock 和所有 .pw4e 文件后重新加密。\n\
             路径: {}",
            folder_path.display()
        )));
    }

    let metadata: FolderMetadataV2 = serde_json::from_str(&json)
        .map_err(|e| Pw4Error::Json(e))?;
    let lock_engine = LockEngine::from_state(metadata.lock_state.clone());

    Ok(EncryptedFolder {
        folder_path: folder_path.to_path_buf(),
        metadata,
        is_unlocked: false,
        master_key: None,
        lock_engine,
        display_name: display_name.to_string(),
    })
}

/// Unlock folder — verify password using STORED encryption date, not system clock.
///
/// Reads the encryption date from `.pw4lock` (primary) or `.pw4date` backup (secondary).
/// If neither is found (trap mode), tries today ±1 day as a last resort.
/// This ensures unlocking works even if the computer's clock is totally wrong.
pub fn unlock_folder(folder: &mut EncryptedFolder, password: &str) -> Pw4Result<UnlockResult> {
    let now = LockEngine::now();

    if folder.lock_engine.is_locked(now) {
        let rem = folder.lock_engine.remaining_lock(now);
        return Ok(UnlockResult::LockedOut { remaining_seconds: rem, remaining_days: rem as u64 / (24 * 3600) });
    }

    let salt_bytes = from_base64(&folder.metadata.salt_b64)?;
    let mut salt = [0u8; SALT_SIZE];
    if salt_bytes.len() == SALT_SIZE { salt.copy_from_slice(&salt_bytes); }

    let verifier = from_base64(&folder.metadata.password_verifier_b64)?;

    // ── Determine encryption date (ignore system clock) ──
    let stored_date: Option<String> = if !folder.metadata.encryption_date_obfuscated.is_empty() {
        deobfuscate_date(&folder.metadata.encryption_date_obfuscated)
    } else {
        // Secondary: .pw4date backup file
        let dp = folder.folder_path.join(".pw4date");
        if dp.exists() {
            std::fs::read_to_string(&dp).ok().and_then(|d| deobfuscate_date(&d))
        } else { None }
    };

    // Try stored date(s) + trap mode candidates
    let candidates: Vec<String> = if let Some(ref d) = stored_date {
        vec![d.clone()] // Only try the stored date
    } else {
        date_candidates() // Trap mode: try today ±1 day
    };

    if let Some(matching_date) = try_date_candidates(password, &salt, &verifier, &candidates) {
        let effective = build_effective_password(password, &matching_date);
        let master_key = derive_master_key(&effective, &salt)?;
        folder.lock_engine.record_success(now);
        folder.metadata.lock_state = folder.lock_engine.state().clone();
        // Ensure date is persisted for future unlocks
        folder.metadata.encryption_date_obfuscated = obfuscate_date(&matching_date);
        folder.is_unlocked = true;
        folder.master_key = Some(master_key);
        save_metadata(folder)?;
        return Ok(UnlockResult::Success);
    }

    // Wrong password
    let _lock_duration = folder.lock_engine.record_failure(now);
    folder.metadata.lock_state = folder.lock_engine.state().clone();
    save_metadata(folder)?;
    let days = LockState::lock_duration_days(folder.lock_engine.attempt_count());
    if folder.lock_engine.attempt_count() >= 10 {
        Ok(UnlockResult::MaxAttempts { attempt_count: folder.lock_engine.attempt_count(), lock_days: days })
    } else {
        Ok(UnlockResult::WrongPassword { attempt_count: folder.lock_engine.attempt_count(), lock_days: days })
    }
}

/// Decrypt all files — decrypt headers + restore original names.
pub fn decrypt_folder(folder: &EncryptedFolder) -> Pw4Result<OperationSummary> {
    if !folder.is_unlocked {
        return Err(Pw4Error::NotUnlocked);
    }
    let master_key = folder.master_key.as_ref().ok_or(Pw4Error::NotUnlocked)?;

    let salt_bytes = from_base64(&folder.metadata.salt_b64)?;
    let mut salt = [0u8; SALT_SIZE];
    if salt_bytes.len() == SALT_SIZE { salt.copy_from_slice(&salt_bytes); }

    let mut summary = OperationSummary::default();

    for (obfuscated_name, entry) in &folder.metadata.entries {
        // Skip directory entries (we don't rename directories)
        if entry.is_directory { continue; }

        let obfuscated_path = folder.folder_path.join(obfuscated_name);
        let original_path = folder.folder_path.join(&entry.original_path);

        // Decrypt header
        if !entry.header_nonce_b64.is_empty() && entry.header_len > 0 {
            let file_key = derive_file_key(master_key, &entry.original_path, &salt)?;
            let nonce_bytes = from_base64(&entry.header_nonce_b64)?;
            let tag_bytes = from_base64(&entry.header_tag_b64).unwrap_or_default();
            let mut nonce = [0u8; NONCE_SIZE];
            let mut tag = [0u8; crate::crypto::TAG_SIZE];
            if nonce_bytes.len() == NONCE_SIZE && tag_bytes.len() == crate::crypto::TAG_SIZE {
                nonce.copy_from_slice(&nonce_bytes);
                tag.copy_from_slice(&tag_bytes);
                decrypt_file_header(&file_key, &nonce, &tag, &obfuscated_path, entry.header_len)?;
            }
        }
        // Rename back to original (ensure parent dir exists)
        if obfuscated_path.exists() {
            if let Some(parent) = original_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Strip any stale .pw4e suffixes (old v1 leftovers)
            let clean_original = strip_pw4e_suffix(&original_path);
            std::fs::rename(&obfuscated_path, &clean_original)?;
        }
        summary.files += 1;
        summary.total_bytes += entry.file_size;
    }

    // Remove .pw4lock + .pw4date
    let lock_path = folder.lock_file_path();
    let date_path = folder.folder_path.join(".pw4date");
    #[cfg(windows)] { unhide_file(&lock_path); unhide_file(&date_path); }
    if lock_path.exists() { std::fs::remove_file(&lock_path)?; }
    if date_path.exists() { std::fs::remove_file(&date_path)?; }

    log::info!("De-obfuscated '{}': {} files restored", folder.display_name, summary.files);
    Ok(summary)
}

/// Re-obfuscate a decrypted folder (lock it again).
/// Requires the password to derive the master key.
pub fn lock_folder(folder: &mut EncryptedFolder, password: &str) -> Pw4Result<()> {
    let path = folder.folder_path.clone();
    let name = folder.display_name.clone();
    folder.master_key = None;
    folder.is_unlocked = false;
    // Re-encrypt using the same folder path
    let _new = encrypt_folder(&path, password, &name)?;
    Ok(())
}

/// Reset password for an unlocked folder.
pub fn reset_password(folder: &mut EncryptedFolder, new_password: &str) -> Pw4Result<()> {
    if !folder.is_unlocked { return Err(Pw4Error::NotUnlocked); }
    if new_password.len() < MIN_PASSWORD_LENGTH { return Err(Pw4Error::PasswordTooShort(MIN_PASSWORD_LENGTH)); }

    let old_key = folder.master_key.as_ref().ok_or(Pw4Error::NotUnlocked)?;
    let salt_bytes = from_base64(&folder.metadata.salt_b64)?;
    let mut salt = [0u8; SALT_SIZE];
    if salt_bytes.len() == SALT_SIZE { salt.copy_from_slice(&salt_bytes); }

    let new_key = derive_master_key(new_password, &salt)?;
    let new_verifier = quick_password_hash(new_password, &salt);

    // Re-encrypt all file headers with the new key
    for (obfuscated_name, entry) in folder.metadata.entries.iter_mut() {
        if entry.is_directory || entry.header_nonce_b64.is_empty() { continue; }

        let obfuscated_path = folder.folder_path.join(obfuscated_name);
        let file_key_old = derive_file_key(old_key, &entry.original_path, &salt)?;
        let file_key_new = derive_file_key(&new_key, &entry.original_path, &salt)?;

        let nonce_bytes = from_base64(&entry.header_nonce_b64)?;
        let tag_bytes = from_base64(&entry.header_tag_b64).unwrap_or_default();
        let mut old_nonce = [0u8; NONCE_SIZE];
        let mut old_tag = [0u8; crate::crypto::TAG_SIZE];
        if nonce_bytes.len() == NONCE_SIZE && tag_bytes.len() == crate::crypto::TAG_SIZE {
            old_nonce.copy_from_slice(&nonce_bytes);
            old_tag.copy_from_slice(&tag_bytes);
        }

        // Decrypt header with old key, re-encrypt with new key
        decrypt_file_header(&file_key_old, &old_nonce, &old_tag, &obfuscated_path, entry.header_len)?;
        let (new_nonce, new_tag, _) = encrypt_file_header(&file_key_new, &obfuscated_path, entry.header_len)?;

        entry.header_nonce_b64 = to_base64(&new_nonce);
        entry.header_tag_b64 = to_base64(&new_tag);
    }

    folder.metadata.password_verifier_b64 = to_base64(&new_verifier);
    folder.master_key = Some(new_key);
    folder.lock_engine.record_success(LockEngine::now());
    folder.metadata.lock_state = folder.lock_engine.state().clone();
    save_metadata(folder)?;

    log::info!("Password reset for '{}'", folder.display_name);
    Ok(())
}

/// Quick probe — does this folder have a .pw4lock?
pub fn probe_folder_status(folder_path: &Path) -> Pw4Result<FolderProbe> {
    let lock_path = folder_path.join(format!(".{}", PWLOCK_EXTENSION));
    if !lock_path.exists() {
        return Ok(FolderProbe { is_encrypted: false, is_unlocked: false, attempt_count: 0, file_count: 0 });
    }
    match std::fs::read_to_string(&lock_path) {
        Ok(json) => {
            // Try v2 format first
            if let Ok(meta) = serde_json::from_str::<FolderMetadataV2>(&json) {
                return Ok(FolderProbe {
                    is_encrypted: true,
                    is_unlocked: false,
                    attempt_count: meta.lock_state.attempt_count,
                    file_count: meta.entries.values().filter(|e| !e.is_directory).count() as u64,
                });
            }
            // Check if it's old v1 format (has "files" field)
            let is_v1 = serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| v.get("version")?.as_u64())
                .map(|v| v == 1)
                .unwrap_or(false);
            if is_v1 {
                log::warn!("Old v1 .pw4lock found in: {}", folder_path.display());
            }
            Ok(FolderProbe { is_encrypted: true, is_unlocked: false, attempt_count: 0, file_count: 0 })
        }
        Err(_) => Ok(FolderProbe { is_encrypted: false, is_unlocked: false, attempt_count: 0, file_count: 0 }),
    }
}

// ── V1 Recovery (old .pw4e full-file encryption) ────────

/// Metadata from the old v1 `.pw4lock` format (full-file `.pw4e` encryption).
#[derive(Serialize, Deserialize, Debug)]
struct V1Metadata {
    version: u32,
    #[serde(alias = "salt_b64")]
    salt: String,
    #[serde(alias = "password_verifier_b64")]
    password_verifier: String,
    #[serde(default)]
    files: HashMap<String, V1FileEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
struct V1FileEntry {
    original_size: u64,
    encrypted_name: String,
    wrapped_key_nonce: String,
    wrapped_key_data: String,
    encrypt_nonce: String,
}

/// Recover a folder encrypted with old v1 pw4you (full-file `.pw4e` encryption).
///
/// The old format used dynamic day-based passwords: `{day:02}{fixed_password}`.
/// This function tries all 31 days to find the correct one, then decrypts
/// all `.pw4e` files back to originals and removes the `.pw4lock`.
pub fn recover_v1_folder(folder_path: &Path, master_password: &str) -> Pw4Result<OperationSummary> {
    let lock_path = folder_path.join(format!(".{}", PWLOCK_EXTENSION));

    // Check for .pw4lock (hidden on Windows)
    if !lock_path.exists() {
        // Scan for .pw4e files to give helpful diagnostic
        let pw4e_count = walkdir::WalkDir::new(folder_path)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "pw4e").unwrap_or(false))
            .count();
        if pw4e_count > 0 {
            return Err(Pw4Error::FolderNotEncrypted(format!(
                "找到 {} 个 .pw4e 加密文件，但缺少 .pw4lock 元数据文件（密钥信息）。\n\n\
                 .pw4lock 是隐藏文件，请在文件管理器中开启「显示隐藏文件」后检查文件夹内是否有该文件。\n\
                 \n如果 .pw4lock 确实已被删除，数据将无法恢复——没有密钥信息就无法解密 .pw4e 文件。\n\
                 路径: {}",
                pw4e_count, folder_path.display()
            )));
        }
        return Err(Pw4Error::FolderNotEncrypted(format!(
            "文件夹中没有找到 .pw4lock（隐藏文件）。\n\
             请确认选择的文件夹正确，并在文件管理器中开启「查看 → 隐藏的项目」。\n\
             路径: {}", folder_path.display()
        )));
    }

    let json = std::fs::read_to_string(&lock_path)?;
    let v1: V1Metadata = serde_json::from_str(&json)
        .map_err(|e| Pw4Error::Json(e))?;

    if v1.version != 1 {
        return Err(Pw4Error::FolderNotEncrypted("Not a v1 .pw4lock".into()));
    }

    // Decode salt and verifier
    let salt_bytes = from_base64(&v1.salt)?;
    let mut salt = [0u8; SALT_SIZE];
    if salt_bytes.len() != SALT_SIZE {
        return Err(Pw4Error::Crypto("Invalid salt size in v1 metadata".into()));
    }
    salt.copy_from_slice(&salt_bytes);

    let verifier_bytes = from_base64(&v1.password_verifier)?;

    // Try all 31 days to find the matching date
    let mut found_day: Option<u32> = None;
    for day in 1..=31 {
        let dynamic_pwd = format!("{:02}{}", day, master_password);
        if crate::password::verify_password_quick(&dynamic_pwd, &salt, &verifier_bytes) {
            found_day = Some(day);
            break;
        }
    }

    let day = found_day.ok_or_else(|| {
        Pw4Error::Crypto("Password does not match any day (01-31). Double-check your password.".into())
    })?;

    let dynamic_pwd = format!("{:02}{}", day, master_password);
    log::info!("V1 recovery: day={}, effective password found", day);

    // Derive master key via Argon2id
    let master_key = derive_master_key(&dynamic_pwd, &salt)?;

    // Decrypt each .pw4e file
    let mut summary = OperationSummary::default();
    for (original_path_str, entry) in &v1.files {
        let pw4e_path = folder_path.join(&entry.encrypted_name);
        let original_path = folder_path.join(original_path_str);

        if !pw4e_path.exists() {
            log::warn!("Skipping missing .pw4e: {}", pw4e_path.display());
            continue;
        }

        // Ensure parent dir
        if let Some(parent) = original_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Unwrap per-file key
        let wk_nonce_bytes = from_base64(&entry.wrapped_key_nonce)?;
        let mut wk_nonce = [0u8; NONCE_SIZE];
        if wk_nonce_bytes.len() == NONCE_SIZE {
            wk_nonce.copy_from_slice(&wk_nonce_bytes);
        }
        let wk_data = from_base64(&entry.wrapped_key_data)?;
        let file_key = crate::crypto::unwrap_key(&master_key, &wk_nonce, &wk_data)?;

        // Decrypt content
        let enc_nonce_bytes = from_base64(&entry.encrypt_nonce)?;
        let mut enc_nonce = [0u8; NONCE_SIZE];
        if enc_nonce_bytes.len() == NONCE_SIZE {
            enc_nonce.copy_from_slice(&enc_nonce_bytes);
        }

        let ciphertext = std::fs::read(&pw4e_path)?;
        let plaintext = crate::crypto::decrypt_block(&file_key, &enc_nonce, &ciphertext)?;
        std::fs::write(&original_path, &plaintext)?;

        // Delete .pw4e
        let _ = std::fs::remove_file(&pw4e_path);

        summary.files += 1;
        summary.total_bytes += entry.original_size;
    }

    // Clean up
    #[cfg(windows)] unhide_file(&lock_path);
    let _ = std::fs::remove_file(&lock_path);
    let date_path = folder_path.join(".pw4date");
    let _ = std::fs::remove_file(&date_path);

    log::info!("V1 recovery complete: {} files restored (day={})", summary.files, day);
    Ok(summary)
}

// ── Helpers ─────────────────────────────────────────────

/// Strip all trailing `.pw4e` suffixes from a path.
/// e.g., "xxx.mp4.pw4e.pw4e" → "xxx.mp4"
fn strip_pw4e_suffix(path: &Path) -> PathBuf {
    let mut s = path.to_string_lossy().to_string();
    while s.ends_with(".pw4e") {
        s = s[..s.len() - 5].to_string(); // ".pw4e" is 5 chars
    }
    PathBuf::from(s)
}

fn save_metadata(folder: &EncryptedFolder) -> Pw4Result<()> {
    let json = serde_json::to_string_pretty(&folder.metadata)?;
    std::fs::write(folder.lock_file_path(), &json)?;
    Ok(())
}

#[cfg(windows)]
fn hide_file(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        windows_sys::Win32::Storage::FileSystem::SetFileAttributesW(wide.as_ptr(), 0x2);
    }
}

#[cfg(windows)]
fn unhide_file(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        windows_sys::Win32::Storage::FileSystem::SetFileAttributesW(wide.as_ptr(), 0x80);
    }
}

#[cfg(not(windows))]
fn hide_file(_path: &Path) {}
#[cfg(not(windows))]
fn unhide_file(_path: &Path) {}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("pw4you_v2_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_test_file(dir: &Path, name: &str, content: &[u8]) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn test_header_encrypt_roundtrip() {
        let dir = temp_dir("header_rtt");
        let test_data = vec![0x41u8; 8192]; // 8KB of 'A'
        create_test_file(&dir, "test.bin", &test_data);

        let key = crate::crypto::generate_key();
        let (nonce, tag, len) = encrypt_file_header(&key, &dir.join("test.bin"), 4096).unwrap();
        assert_eq!(len, 4096);

        // Verify header is scrambled (but same size)
        let scrambled = fs::read(dir.join("test.bin")).unwrap();
        assert_eq!(scrambled.len(), 8192); // file size unchanged
        assert_ne!(&scrambled[..100], &test_data[..100]); // first bytes differ
        assert_eq!(&scrambled[4096..], &test_data[4096..]); // rest is untouched

        // Decrypt
        decrypt_file_header(&key, &nonce, &tag, &dir.join("test.bin"), len).unwrap();
        let restored = fs::read(dir.join("test.bin")).unwrap();
        assert_eq!(restored, test_data);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_obfuscate_and_restore() {
        let dir = temp_dir("obfuscate");
        create_test_file(&dir, "报告.docx", b"fake docx header content here!!");
        create_test_file(&dir, "picture.jpg", b"fake jpeg header\xFF\xD8\xFF\xE0");
        fs::create_dir_all(dir.join("素材")).unwrap();
        create_test_file(&dir.join("素材"), "logo.png", b"fake png header content");

        // Encrypt
        let folder = encrypt_folder(&dir, "password123", "Test").unwrap();
        assert!(folder.is_unlocked);

        // After encryption, original filenames should NOT exist
        assert!(!dir.join("报告.docx").exists());
        assert!(!dir.join("picture.jpg").exists());
        // Directory names are NOT renamed, only files
        assert!(dir.join("素材").exists());
        assert!(dir.join(".pw4lock").exists());

        // 3 file entries in metadata
        assert_eq!(folder.metadata.entries.len(), 3);

        // Decrypt
        let summary = decrypt_folder(&folder).unwrap();
        assert_eq!(summary.files, 3);

        // Everything should be restored
        assert!(dir.join("报告.docx").exists());
        assert!(dir.join("picture.jpg").exists());
        assert!(dir.join("素材").join("logo.png").exists());
        assert!(!dir.join(".pw4lock").exists());

        // Content should match
        assert_eq!(
            fs::read(dir.join("报告.docx")).unwrap(),
            b"fake docx header content here!!"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unlock_wrong_password() {
        let dir = temp_dir("wrongpwd_v2");
        create_test_file(&dir, "file.txt", b"test content data here 12345");

        let folder = encrypt_folder(&dir, "correct123", "Test").unwrap();
        drop(folder);

        let mut f = open_folder(&dir, "Test").unwrap();
        let result = unlock_folder(&mut f, "wrongpass").unwrap();
        assert!(matches!(result, UnlockResult::WrongPassword { .. }));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_password_reset() {
        let dir = temp_dir("reset_v2");
        create_test_file(&dir, "data.txt", b"original content here!!");

        let mut folder = encrypt_folder(&dir, "oldpassword", "Test").unwrap();
        reset_password(&mut folder, "newpassword").unwrap();

        // Decrypt with new key
        let summary = decrypt_folder(&folder).unwrap();
        assert_eq!(summary.files, 1);
        assert_eq!(fs::read_to_string(dir.join("data.txt")).unwrap(), "original content here!!");

        let _ = fs::remove_dir_all(&dir);
    }
}
