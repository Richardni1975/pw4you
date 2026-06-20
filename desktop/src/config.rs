//! Application configuration — stores vault list and user preferences.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration stored in %APPDATA%\pw4you\config.json.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    /// List of vaults the user has registered.
    pub vaults: Vec<VaultEntry>,
    /// Auto-lock idle time in minutes (0 = never).
    pub auto_lock_minutes: u32,
    /// Whether to start minimized to tray.
    pub start_minimized: bool,
    /// UI language (future use).
    pub language: String,
}

/// A registered vault entry.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VaultEntry {
    /// Display name for this vault.
    pub name: String,
    /// Path to the .pw4 container file.
    pub path: String,
    /// Argon2 hash of the fixed password (for verification only).
    pub password_hash: String,
    /// Mount point (e.g., "P:") when unlocked.
    pub mount_point: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            vaults: Vec::new(),
            auto_lock_minutes: 30,
            start_minimized: false,
            language: "zh-CN".to_string(),
        }
    }
}

impl AppConfig {
    /// Load config from disk, or return default if not found.
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str(&data) {
                    return config;
                }
            }
        }
        Self::default()
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
        }
        let data = serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize config: {}", e))?;
        std::fs::write(&path, data).map_err(|e| format!("Failed to write config: {}", e))?;
        Ok(())
    }
}

/// Path to the config file: %APPDATA%\pw4you\config.json
fn config_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(appdata).join("pw4you").join("config.json")
}
