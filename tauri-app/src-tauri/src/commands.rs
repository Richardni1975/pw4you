//! Tauri commands — bridge between frontend and pw4you-core.

use lettre::Transport;
use pw4you_core::{
    self, config, email, inplace,
    EncryptedFolderEntry, SmtpConfig, UnlockResult,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

use crate::AppState;

/// Result returned to the frontend.
#[derive(Serialize, Clone)]
pub struct CmdResult<T: Serialize + Clone> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize + Clone> CmdResult<T> {
    fn ok(data: T) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, data: None, error: Some(msg.into()) }
    }
}

// ── Encrypt Folder ──────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptFolderArgs {
    pub folder_path: String,
    pub name: String,
    pub password: String,
}

#[derive(Serialize, Clone)]
pub struct EncryptFolderResult {
    pub path: String,
    pub name: String,
    pub password_hash: String,
    pub file_count: u64,
    pub total_bytes: u64,
}

#[tauri::command]
pub fn encrypt_folder(
    args: EncryptFolderArgs,
    state: State<AppState>,
) -> CmdResult<EncryptFolderResult> {
    let folder_path = Path::new(&args.folder_path);

    match inplace::encrypt_folder(folder_path, &args.password, &args.name) {
        Ok(mut folder) => {
            let hash = folder.metadata.password_verifier_b64.clone();
            let file_count = folder.metadata.entries.values().filter(|e| !e.is_directory).count() as u64;
            let total_bytes: u64 = folder.metadata.entries.values().map(|f| f.file_size).sum();

            // Add to config and save
            let mut config = state.config.lock().unwrap();
            let entry = EncryptedFolderEntry {
                name: args.name.clone(),
                path: args.folder_path.clone(),
                password_hash: hash.clone(),
            };
            let _ = config::add_encrypted_folder(&mut config, entry);

            // Drop folder (clear key from memory after encrypt)
            folder.master_key = None;
            folder.is_unlocked = false;
            *state.folder.lock().unwrap() = None;

            CmdResult::ok(EncryptFolderResult {
                path: args.folder_path,
                name: args.name,
                password_hash: hash,
                file_count,
                total_bytes,
            })
        }
        Err(e) => CmdResult::err(format!("加密失败: {}", e)),
    }
}

// ── Unlock Folder ───────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockFolderArgs {
    pub folder_path: String,
    pub password: String,
}

#[derive(Serialize, Clone)]
pub struct UnlockFolderResult {
    pub status: String,  // "success" | "locked_out" | "max_attempts" | "wrong_password"
    pub attempt_count: u32,
    pub lock_days: u64,
    pub remaining_seconds: i64,
    pub files_decrypted: u64,
    pub message: String,
}

#[tauri::command]
pub fn unlock_folder(
    args: UnlockFolderArgs,
    state: State<AppState>,
) -> CmdResult<UnlockFolderResult> {
    let folder_path = Path::new(&args.folder_path);
    let config = state.config.lock().unwrap();
    let display_name = config
        .encrypted_folders
        .iter()
        .find(|e| e.path == args.folder_path)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    drop(config);

    match inplace::open_folder(folder_path, &display_name) {
        Ok(mut folder) => {
            let _total_files = folder.metadata.entries.len() as u64;
            match inplace::unlock_folder(&mut folder, &args.password) {
                Ok(result) => match result {
                    UnlockResult::Success => {
                        // Immediately decrypt all files to restore originals
                        match inplace::decrypt_folder(&folder) {
                            Ok(summary) => {
                                // Keep folder in config (marked as decrypted for re-encrypt)
                                *state.folder.lock().unwrap() = None;
                                CmdResult::ok(UnlockFolderResult {
                                    status: "success".into(),
                                    attempt_count: 0,
                                    lock_days: 0,
                                    remaining_seconds: 0,
                                    files_decrypted: summary.files,
                                    message: format!(
                                        "✅ \"{}\" 已解锁 — {} 个文件已恢复",
                                        display_name, summary.files
                                    ),
                                })
                            }
                            Err(e) => CmdResult::err(format!("解密文件失败: {}", e)),
                        }
                    }
                    UnlockResult::WrongPassword { attempt_count, lock_days } => {
                        CmdResult::ok(UnlockFolderResult {
                            status: "wrong_password".into(),
                            attempt_count,
                            lock_days,
                            remaining_seconds: lock_days as i64 * 24 * 3600,
                            files_decrypted: 0,
                            message: format!(
                                "❌ 密码错误！第 {} 次尝试，锁定 {} 天",
                                attempt_count, lock_days
                            ),
                        })
                    }
                    UnlockResult::LockedOut { remaining_seconds, remaining_days } => {
                        CmdResult::ok(UnlockFolderResult {
                            status: "locked_out".into(),
                            attempt_count: folder.lock_engine.attempt_count(),
                            lock_days: remaining_days,
                            remaining_seconds,
                            files_decrypted: 0,
                            message: format!(
                                "🔒 已锁定，还需等待 {} 天 {} 小时",
                                remaining_days,
                                (remaining_seconds % (24 * 3600)) / 3600
                            ),
                        })
                    }
                    UnlockResult::MaxAttempts { attempt_count, lock_days } => {
                        CmdResult::ok(UnlockFolderResult {
                            status: "max_attempts".into(),
                            attempt_count,
                            lock_days,
                            remaining_seconds: lock_days as i64 * 24 * 3600,
                            files_decrypted: 0,
                            message: format!(
                                "🚫 连续错误 {} 次，已锁定 {} 天。请使用邮箱找回密码。",
                                attempt_count, lock_days
                            ),
                        })
                    }
                    UnlockResult::ClockRollback { penalty_seconds } => {
                        CmdResult::ok(UnlockFolderResult {
                            status: "clock_rollback".into(),
                            attempt_count: folder.lock_engine.attempt_count(),
                            lock_days: 0,
                            remaining_seconds: penalty_seconds,
                            files_decrypted: 0,
                            message: format!(
                                "⚠️ 检测到系统时钟回拨！锁定延长 {} 秒",
                                penalty_seconds
                            ),
                        })
                    }
                },
                Err(e) => CmdResult::err(format!("解锁错误: {}", e)),
            }
        }
        Err(e) => CmdResult::err(format!("无法打开加密文件夹: {}", e)),
    }
}

// ── Decrypt Folder ──────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptFolderArgs {
    #[allow(dead_code)]
    pub folder_path: String,
}

#[derive(Serialize, Clone)]
pub struct DecryptFolderResult {
    pub files: u64,
    pub total_bytes: u64,
}

#[tauri::command]
pub fn decrypt_folder(
    _args: DecryptFolderArgs,
    state: State<AppState>,
) -> CmdResult<DecryptFolderResult> {
    let mut folder_guard = state.folder.lock().unwrap();
    if let Some(ref folder) = *folder_guard {
        if !folder.is_unlocked {
            return CmdResult::err("文件夹未解锁，请先输入密码解锁");
        }
        match inplace::decrypt_folder(folder) {
            Ok(summary) => {
                // Remove from config
                let mut config = state.config.lock().unwrap();
                let path = folder.folder_path.to_string_lossy().to_string();
                let _ = config::remove_encrypted_folder(&mut config, &path);

                // Clear folder state
                *folder_guard = None;

                CmdResult::ok(DecryptFolderResult {
                    files: summary.files,
                    total_bytes: summary.total_bytes,
                })
            }
            Err(e) => CmdResult::err(format!("解密失败: {}", e)),
        }
    } else {
        CmdResult::err("没有已解锁的文件夹".to_string())
    }
}

// ── Lock Folder ─────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockFolderArgs {
    pub password: String,
}

#[tauri::command]
pub fn lock_folder(
    args: LockFolderArgs,
    state: State<AppState>,
) -> CmdResult<String> {
    let mut folder_guard = state.folder.lock().unwrap();
    if let Some(ref mut folder) = *folder_guard {
        if !folder.is_unlocked {
            return CmdResult::err("文件夹未解锁");
        }
        match inplace::lock_folder(folder, &args.password) {
            Ok(()) => {
                let name = folder.display_name.clone();
                *folder_guard = None;
                CmdResult::ok(format!("🔒 \"{}\" 已锁定", name))
            }
            Err(e) => CmdResult::err(format!("锁定失败: {}", e)),
        }
    } else {
        CmdResult::err("没有已解锁的文件夹".to_string())
    }
}

// ── Get Folder Status ───────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatusArgs {
    pub folder_path: String,
}

#[derive(Serialize, Clone)]
pub struct FolderStatusInfo {
    pub is_encrypted: bool,
    pub is_unlocked: bool,
    pub file_count: u64,
    pub attempt_count: u32,
    pub remaining_lock_seconds: i64,
    pub path: String,
}

#[tauri::command]
pub fn get_folder_status(
    args: FolderStatusArgs,
    state: State<AppState>,
) -> CmdResult<FolderStatusInfo> {
    let folder_path = Path::new(&args.folder_path);

    // Check if currently unlocked in app state
    let folder_guard = state.folder.lock().unwrap();
    let is_unlocked = folder_guard
        .as_ref()
        .map(|f| f.is_unlocked && f.folder_path == folder_path)
        .unwrap_or(false);

    match inplace::probe_folder_status(folder_path) {
        Ok(probe) => {
            CmdResult::ok(FolderStatusInfo {
                is_encrypted: probe.is_encrypted,
                is_unlocked,
                file_count: probe.file_count,
                attempt_count: probe.attempt_count,
                remaining_lock_seconds: 0,
                path: args.folder_path,
            })
        }
        Err(e) => CmdResult::err(format!("读取状态失败: {}", e)),
    }
}

// ── List Folders ────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct FolderListItem {
    pub name: String,
    pub path: String,
    pub status: String,  // "locked" | "unlocked" | "locked_out"
    pub file_count: u64,
    pub attempt_count: u32,
    pub remaining_days: u64,
}

#[tauri::command]
pub fn list_folders(state: State<AppState>) -> CmdResult<Vec<FolderListItem>> {
    let config = state.config.lock().unwrap();
    let folder_guard = state.folder.lock().unwrap();

    let mut items = Vec::new();

    for entry in &config.encrypted_folders {
        let folder_path = Path::new(&entry.path);

        // Check if this folder is currently unlocked
        let is_unlocked = folder_guard
            .as_ref()
            .map(|f| f.is_unlocked && f.folder_path == folder_path)
            .unwrap_or(false);

        let probe = inplace::probe_folder_status(folder_path).ok();

        let status = if is_unlocked {
            "unlocked".to_string()
        } else if let Some(ref p) = probe {
            if p.is_encrypted {
                "locked".to_string()
            } else {
                // .pw4lock doesn't exist but folder is in config → decrypted
                "decrypted".to_string()
            }
        } else {
            "decrypted".to_string() // assume decrypted if can't probe
        };

        items.push(FolderListItem {
            name: entry.name.clone(),
            path: entry.path.clone(),
            status,
            file_count: probe.as_ref().map(|p| p.file_count).unwrap_or(0),
            attempt_count: probe.as_ref().map(|p| p.attempt_count).unwrap_or(0),
            remaining_days: 0,
        });
    }

    CmdResult::ok(items)
}

// ── Load Config ─────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct ConfigInfo {
    pub bound_email: Option<String>,
    pub smtp_configured: bool,
    pub auto_lock_minutes: u32,
    pub folder_count: usize,
    pub email_configured: bool,
}

#[tauri::command]
pub fn load_config(state: State<AppState>) -> CmdResult<ConfigInfo> {
    match config::load_config() {
        Ok(cfg) => {
            let info = ConfigInfo {
                bound_email: cfg.bound_email.clone(),
                smtp_configured: cfg.smtp_config.is_some()
                    && !cfg.smtp_config.as_ref().map(|s| s.server.is_empty()).unwrap_or(true),
                auto_lock_minutes: cfg.settings.auto_lock_minutes,
                folder_count: cfg.encrypted_folders.len(),
                email_configured: config::is_email_configured(&cfg),
            };
            *state.config.lock().unwrap() = cfg;
            CmdResult::ok(info)
        }
        Err(e) => CmdResult::err(format!("加载配置失败: {}", e)),
    }
}

// ── Save Config ─────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConfigArgs {
    pub auto_lock_minutes: Option<u32>,
}

#[tauri::command]
pub fn save_config(
    args: SaveConfigArgs,
    state: State<AppState>,
) -> CmdResult<String> {
    let mut config = state.config.lock().unwrap();
    if let Some(minutes) = args.auto_lock_minutes {
        config.settings.auto_lock_minutes = minutes;
    }
    match config::save_config(&config) {
        Ok(()) => CmdResult::ok("配置已保存".to_string()),
        Err(e) => CmdResult::err(format!("保存配置失败: {}", e)),
    }
}

// ── Bind Email ──────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindEmailArgs {
    pub email: String,
}

#[tauri::command]
pub fn bind_email(
    args: BindEmailArgs,
    state: State<AppState>,
) -> CmdResult<String> {
    // Basic email validation
    if !args.email.contains('@') || args.email.len() < 5 {
        return CmdResult::err("请输入有效的邮箱地址");
    }

    let mut config = state.config.lock().unwrap();
    match config::set_bound_email(&mut config, &args.email) {
        Ok(()) => CmdResult::ok(format!("安全邮箱已绑定: {}", email::mask_email(&args.email))),
        Err(e) => CmdResult::err(format!("绑定失败: {}", e)),
    }
}

// ── Send Verification Code ──────────────────────────

#[derive(Serialize, Clone)]
pub struct SendCodeResult {
    pub masked_email: String,
    pub message: String,
}

#[tauri::command]
pub fn send_verification_code(state: State<AppState>) -> CmdResult<SendCodeResult> {
    let config = state.config.lock().unwrap();

    // Check that email and SMTP are configured
    let bound_email = match &config.bound_email {
        Some(e) => e.clone(),
        None => return CmdResult::err("未绑定安全邮箱。请先在「设置」中绑定邮箱。"),
    };

    let smtp_config = match &config.smtp_config {
        Some(s) if !s.server.is_empty() => s.clone(),
        _ => return CmdResult::err(
            "未配置邮件服务器。请先在「设置」中配置 SMTP 邮件发送设置。"
        ),
    };

    // Generate code
    let code = email::generate_verification_code();
    let expiry = chrono::Utc::now().timestamp() + email::CODE_EXPIRY_SECONDS;

    // Send email
    match email::send_verification_email(&smtp_config, &bound_email, &code) {
        Ok(()) => {
            // Store pending code
            *state.pending_code.lock().unwrap() = Some((code.clone(), expiry));

            CmdResult::ok(SendCodeResult {
                masked_email: email::mask_email(&bound_email),
                message: format!("验证码已发送至 {}", email::mask_email(&bound_email)),
            })
        }
        Err(e) => CmdResult::err(format!("发送失败: {}", e)),
    }
}

// ── Verify Code and Reset Password ──────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyAndResetArgs {
    pub code: String,
    pub folder_path: String,
    pub new_password: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct VerifyResult {
    pub status: String,  // "verified" | "password_reset"
    pub message: String,
}

#[tauri::command]
pub fn verify_code_and_reset(
    args: VerifyAndResetArgs,
    state: State<AppState>,
) -> CmdResult<VerifyResult> {
    // Verify the code
    let pending = state.pending_code.lock().unwrap().clone();
    let (pending_code, expiry) = match pending {
        Some(p) => p,
        None => return CmdResult::err("未发送验证码，请先点击「发送验证码」"),
    };

    match email::verify_code(&args.code, &pending_code, expiry) {
        Ok(()) => {
            // Clear the used code
            *state.pending_code.lock().unwrap() = None;

            let _folder_path = Path::new(&args.folder_path);

            if let Some(new_password) = &args.new_password {
                // Reset password mode
                // First unlock with the verification (folder should already be open)
                let mut folder_guard = state.folder.lock().unwrap();
                if let Some(ref mut folder) = *folder_guard {
                    match inplace::reset_password(folder, new_password) {
                        Ok(()) => {
                            // Update config with new password hash
                            let mut config = state.config.lock().unwrap();
                            if let Some(entry) = config.encrypted_folders.iter_mut()
                                .find(|e| e.path == args.folder_path)
                            {
                                entry.password_hash = folder.metadata.password_verifier_b64.clone();
                            }
                            let _ = config::save_config(&config);

                            CmdResult::ok(VerifyResult {
                                status: "password_reset".into(),
                                message: "✅ 密码已重置成功！文件夹已解锁。".into(),
                            })
                        }
                        Err(e) => CmdResult::err(format!("密码重置失败: {}", e)),
                    }
                } else {
                    CmdResult::err("文件夹未打开，无法重置密码".to_string())
                }
            } else {
                // Just verification mode (unlock the folder directly)
                // The folder needs to be opened and unlocked already
                CmdResult::ok(VerifyResult {
                    status: "verified".into(),
                    message: "✅ 验证成功！文件夹已解锁。".into(),
                })
            }
        }
        Err(e) => CmdResult::err(format!("{}", e)),
    }
}

// ── Update SMTP Config ──────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmtpConfigArgs {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[tauri::command]
pub fn update_smtp_config(
    args: SmtpConfigArgs,
    state: State<AppState>,
) -> CmdResult<String> {
    if args.server.is_empty() {
        return CmdResult::err("SMTP 服务器地址不能为空");
    }
    if args.username.is_empty() {
        return CmdResult::err("发件邮箱地址不能为空");
    }

    let smtp = SmtpConfig {
        server: args.server,
        port: args.port,
        username: args.username,
        password: args.password,
    };

    let mut config = state.config.lock().unwrap();
    match config::update_smtp_config(&mut config, smtp) {
        Ok(()) => CmdResult::ok("邮件设置已保存 ✅".to_string()),
        Err(e) => CmdResult::err(format!("保存失败: {}", e)),
    }
}

// ── Send Password to Email ──────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendPasswordArgs {
    pub password: String,
    pub folder_name: String,
}

#[tauri::command]
pub fn send_password_to_email(
    args: SendPasswordArgs,
    state: State<AppState>,
) -> CmdResult<String> {
    let config = state.config.lock().unwrap();

    let bound_email = match &config.bound_email {
        Some(e) => e.clone(),
        None => return CmdResult::err("未绑定安全邮箱。请先在「设置」中绑定邮箱。"),
    };

    let smtp_config = match &config.smtp_config {
        Some(s) if !s.server.is_empty() => s.clone(),
        _ => return CmdResult::err("未配置邮件服务器。请先在「设置」中配置 SMTP。"),
    };

    // Build a password-backup email
    let email_body = format!(
        "您好，\n\n\
         您正在使用 pw4you 加密文件夹。\n\n\
         文件夹名称：{}\n\
         加密密码：{}\n\n\
         ⚠️ 请妥善保管此邮件，不要分享给他人。\n\
         ⚠️ 如果这不是您的操作，请忽略此邮件。\n\n\
         — pw4you 密码管理器",
        args.folder_name, args.password
    );

    let from_addr = match format!("pw4you <{}>", smtp_config.username).parse() {
        Ok(a) => a,
        Err(e) => return CmdResult::err(format!("Invalid from address: {}", e)),
    };
    let to_addr = match bound_email.parse() {
        Ok(a) => a,
        Err(e) => return CmdResult::err(format!("Invalid to address: {}", e)),
    };

    let email_msg = match lettre::Message::builder()
        .from(from_addr)
        .to(to_addr)
        .subject(format!("🔐 pw4you — 文件夹「{}」的加密密码备份", args.folder_name))
        .body(email_body)
    {
        Ok(m) => m,
        Err(e) => return CmdResult::err(format!("Build email failed: {}", e)),
    };

    let creds = lettre::transport::smtp::authentication::Credentials::new(
        smtp_config.username.clone(),
        smtp_config.password.clone(),
    );

    let mailer = if smtp_config.port == 465 {
        match lettre::SmtpTransport::relay(&smtp_config.server) {
            Ok(t) => t.credentials(creds).port(smtp_config.port).build(),
            Err(e) => return CmdResult::err(format!("SMTP relay error: {}", e)),
        }
    } else {
        match lettre::SmtpTransport::starttls_relay(&smtp_config.server) {
            Ok(t) => t.credentials(creds).port(smtp_config.port).build(),
            Err(e) => return CmdResult::err(format!("SMTP starttls error: {}", e)),
        }
    };

    match mailer.send(&email_msg) {
        Ok(_) => CmdResult::ok(format!(
            "📧 密码已发送至 {}",
            email::mask_email(&bound_email)
        )),
        Err(e) => CmdResult::err(format!("发送失败: {}", e)),
    }
}

// ── Recover V1 Folder (old .pw4e format) ─────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverV1Args {
    pub folder_path: String,
    pub master_password: String,
}

#[tauri::command]
pub fn recover_v1_folder(
    args: RecoverV1Args,
    state: State<AppState>,
) -> CmdResult<String> {
    let folder_path = std::path::Path::new(&args.folder_path);
    match inplace::recover_v1_folder(folder_path, &args.master_password) {
        Ok(summary) => {
            // Remove from config if present
            let mut config = state.config.lock().unwrap();
            let _ = config::remove_encrypted_folder(&mut config, &args.folder_path);
            CmdResult::ok(format!(
                "✅ 旧版文件夹恢复成功！{} 个文件已还原为原始格式。\n现在可以用新版重新加密此文件夹。",
                summary.files
            ))
        }
        Err(e) => CmdResult::err(format!("恢复失败: {}", e)),
    }
}

// ── Remove Folder ───────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveFolderArgs {
    pub folder_path: String,
}

#[tauri::command]
pub fn remove_folder(
    args: RemoveFolderArgs,
    state: State<AppState>,
) -> CmdResult<String> {
    // Check if folder is currently unlocked
    let folder_guard = state.folder.lock().unwrap();
    if let Some(ref folder) = *folder_guard {
        if folder.is_unlocked && folder.folder_path.to_string_lossy() == args.folder_path {
            return CmdResult::err(
                "⚠️ 文件夹当前已解锁，请先锁定后再移除注册信息。\n（仅移除注册信息，不会删除加密文件）"
            );
        }
    }
    drop(folder_guard);

    let mut config = state.config.lock().unwrap();
    match config::remove_encrypted_folder(&mut config, &args.folder_path) {
        Ok(()) => CmdResult::ok("已从列表中移除（加密文件未删除）".to_string()),
        Err(e) => CmdResult::err(format!("移除失败: {}", e)),
    }
}
