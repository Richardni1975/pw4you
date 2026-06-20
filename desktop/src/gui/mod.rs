//! GUI module — organizes all UI components.

pub mod about;
pub mod create_vault;
pub mod password_dialog;
pub mod settings;
pub mod vault_list;

/// Application tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Vaults,
    Create,
    Settings,
    About,
}
