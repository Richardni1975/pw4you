//! Application configuration management.
//!
//! Reads and writes `config.json` in `%APPDATA%/pw4you/`.
//! Tracks registered encrypted folders, bound email, SMTP settings, and preferences.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::Pw4Result;

/// Current config file version.
const CONFIG_VERSION: u32 = 2;

/// Application configuration stored in `%APPDATA%/pw4you/config.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    /// Config format version.
    pub version: u32,
    /// List of registered encrypted folders.
    pub encrypted_folders: Vec<EncryptedFolderEntry>,
    /// Security email bound for password recovery.
    pub bound_email: Option<String>,
    /// SMTP server configuration for sending verification codes.
    pub smtp_config: Option<SmtpConfig>,
    /// User preferences.
    pub settings: AppSettings,
}

/// Entry for one encrypted folder in the config registry.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedFolderEntry {
    /// User-assigned display name.
    pub name: String,
    /// Absolute filesystem path to the folder.
    pub path: String,
    /// Argon2id hash of the password (for verification before unlock).
    pub password_hash: String,
}

/// SMTP server settings for email sending.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SmtpConfig {
    /// SMTP server hostname, e.g. "smtp.gmail.com".
    pub server: String,
    /// SMTP server port, e.g. 587 for STARTTLS.
    pub port: u16,
    /// Sender email address (also used for SMTP auth).
    pub username: String,
    /// SMTP auth password (App Password, not account password).
    /// ⚠️ WARNING: Stored in plain text in config.json.
    /// This is acceptable because config.json is in the user's private AppData directory.
    pub password: String,
}

/// Application preferences.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    /// Auto-lock after N minutes of inactivity (0 = never).
    pub auto_lock_minutes: u32,
    /// UI language.
    pub language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            encrypted_folders: Vec::new(),
            bound_email: None,
            smtp_config: None,
            settings: AppSettings::default(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 30,
            language: "zh-CN".to_string(),
        }
    }
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            server: String::new(),
            port: 587,
            username: String::new(),
            password: String::new(),
        }
    }
}

/// Get the config directory path (`%APPDATA%/pw4you/`).
/// Creates the directory if it doesn't exist.
pub fn config_dir() -> Pw4Result<PathBuf> {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(&appdata).join("pw4you");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Get the config file path (`%APPDATA%/pw4you/config.json`).
pub fn config_path() -> Pw4Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

/// Load application configuration from disk.
///
/// Returns default config if the file doesn't exist or is corrupted.
pub fn load_config() -> Pw4Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        let default_config = AppConfig::default();
        save_config(&default_config)?;
        return Ok(default_config);
    }

    let data = std::fs::read_to_string(&path)?;
    match serde_json::from_str::<AppConfig>(&data) {
        Ok(mut config) => {
            // Migrate old config versions if needed
            if config.version < CONFIG_VERSION {
                config.version = CONFIG_VERSION;
                save_config(&config)?;
            }
            Ok(config)
        }
        Err(e) => {
            log::warn!("Failed to parse config.json, using defaults: {}", e);
            let default_config = AppConfig::default();
            save_config(&default_config)?;
            Ok(default_config)
        }
    }
}

/// Save application configuration to disk.
pub fn save_config(config: &AppConfig) -> Pw4Result<()> {
    let path = config_path()?;
    let data = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, &data)?;
    Ok(())
}

/// Add an encrypted folder entry to config and save.
pub fn add_encrypted_folder(config: &mut AppConfig, entry: EncryptedFolderEntry) -> Pw4Result<()> {
    // Remove existing entry for the same path if any
    config.encrypted_folders.retain(|e| e.path != entry.path);
    config.encrypted_folders.push(entry);
    save_config(config)
}

/// Remove an encrypted folder entry from config and save.
pub fn remove_encrypted_folder(config: &mut AppConfig, path: &str) -> Pw4Result<()> {
    config.encrypted_folders.retain(|e| e.path != path);
    save_config(config)
}

/// Update the SMTP configuration and save.
pub fn update_smtp_config(config: &mut AppConfig, smtp: SmtpConfig) -> Pw4Result<()> {
    config.smtp_config = Some(smtp);
    save_config(config)
}

/// Set the bound security email and save.
pub fn set_bound_email(config: &mut AppConfig, email: &str) -> Pw4Result<()> {
    config.bound_email = Some(email.to_string());
    save_config(config)
}

/// Check if email recovery is fully configured.
pub fn is_email_configured(config: &AppConfig) -> bool {
    config.bound_email.is_some() && config.smtp_config.is_some()
        && !config.smtp_config.as_ref().map(|s| s.server.is_empty()).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_roundtrip() {
        let mut config = AppConfig::default();

        config.encrypted_folders.push(EncryptedFolderEntry {
            name: "Test Folder".to_string(),
            path: "C:\\test".to_string(),
            password_hash: "test_hash".to_string(),
        });

        config.bound_email = Some("test@example.com".to_string());
        config.smtp_config = Some(SmtpConfig {
            server: "smtp.gmail.com".to_string(),
            port: 587,
            username: "test@gmail.com".to_string(),
            password: "app_password".to_string(),
        });

        // Serialize and deserialize
        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.version, CONFIG_VERSION);
        assert_eq!(restored.encrypted_folders.len(), 1);
        assert_eq!(restored.encrypted_folders[0].name, "Test Folder");
        assert_eq!(restored.bound_email, Some("test@example.com".to_string()));
        assert!(restored.smtp_config.is_some());
    }

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.version, CONFIG_VERSION);
        assert!(config.encrypted_folders.is_empty());
        assert!(config.bound_email.is_none());
        assert!(config.smtp_config.is_none());
        assert_eq!(config.settings.auto_lock_minutes, 30);
    }

    #[test]
    fn test_is_email_configured() {
        let mut config = AppConfig::default();
        assert!(!is_email_configured(&config));

        config.bound_email = Some("test@test.com".to_string());
        assert!(!is_email_configured(&config));

        config.smtp_config = Some(SmtpConfig {
            server: "smtp.gmail.com".to_string(),
            port: 587,
            username: "test@gmail.com".to_string(),
            password: "pass".to_string(),
        });
        assert!(is_email_configured(&config));
    }
}
