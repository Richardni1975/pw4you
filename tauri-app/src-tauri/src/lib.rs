//! pw4you Tauri Backend — wraps pw4you-core for the web frontend.

use std::sync::Mutex;

mod commands;

/// Application state shared across Tauri commands.
pub struct AppState {
    /// Currently open encrypted folder (unlocked, with master key loaded).
    pub folder: Mutex<Option<pw4you_core::EncryptedFolder>>,
    /// Application configuration (loaded from %APPDATA%/pw4you/config.json).
    pub config: Mutex<pw4you_core::AppConfig>,
    /// Pending verification code and expiry timestamp for password recovery.
    /// Format: (code, expiry_unix_timestamp)
    pub pending_code: Mutex<Option<(String, i64)>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    // Load config at startup
    let config = pw4you_core::config::load_config().unwrap_or_default();
    log::info!(
        "pw4you v0.2.0 started — {} encrypted folders registered",
        config.encrypted_folders.len()
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            folder: Mutex::new(None),
            config: Mutex::new(config),
            pending_code: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::encrypt_folder,
            commands::unlock_folder,
            commands::decrypt_orphaned_folder,
            commands::decrypt_folder,
            commands::lock_folder,
            commands::get_folder_status,
            commands::list_folders,
            commands::load_config,
            commands::save_config,
            commands::bind_email,
            commands::send_verification_code,
            commands::verify_code_and_reset,
            commands::update_smtp_config,
            commands::send_password_to_email,
            commands::recover_v1_folder,
            commands::remove_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
