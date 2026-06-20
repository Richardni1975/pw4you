//! Virtual disk mounting — simplified implementation.
//!
//! Current approach: extracts vault contents to a temp folder,
//! opens it in Windows Explorer, and securely cleans on lock.
//!
//! Future enhancement: WinFsp-based virtual drive with on-the-fly
//! encryption/decryption (requires WinFsp to be installed).

use std::path::PathBuf;
use log::{info, error};

/// Mount state for a vault.
pub struct MountPoint {
    /// Display name of the vault.
    pub vault_name: String,
    /// Temp directory where files are extracted.
    pub temp_dir: PathBuf,
    /// Whether the mount is active.
    pub active: bool,
}

impl MountPoint {
    /// Create a new mount by extracting vault contents to a temp directory.
    pub fn mount(vault_name: &str, container: &pw4you_core::Vault) -> Result<Self, String> {
        let temp_dir = std::env::temp_dir()
            .join("pw4you")
            .join(sanitize_name(vault_name));

        // Clean any previous extraction
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create mount dir: {}", e))?;

        info!("Mounting '{}' to {}", vault_name, temp_dir.display());

        // Extract all files from the vault
        let summary = pw4you_core::protect::decrypt_container(
            container.container(),
            &temp_dir,
        )
        .map_err(|e| format!("Failed to decrypt vault: {}", e))?;

        info!(
            "Mounted {} files, {} dirs ({} bytes)",
            summary.files, summary.directories, summary.total_bytes
        );

        // Open the folder in Explorer
        let _ = open::that(&temp_dir);

        Ok(Self {
            vault_name: vault_name.to_string(),
            temp_dir,
            active: true,
        })
    }

    /// Unmount: securely delete the temp folder and its contents.
    pub fn unmount(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }

        info!("Unmounting '{}' — securely deleting temp files", self.vault_name);

        pw4you_core::protect::secure_delete_folder(&self.temp_dir)
            .map_err(|e| format!("Failed to securely delete mount: {}", e))?;

        self.active = false;
        Ok(())
    }
}

impl Drop for MountPoint {
    fn drop(&mut self) {
        if self.active {
            if let Err(e) = self.unmount() {
                error!("Failed to unmount on drop: {}", e);
            }
        }
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}
