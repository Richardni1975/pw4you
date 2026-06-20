//! Vault list UI — displays registered vaults and their status.

use egui::{Color32, RichText, Ui};
use crate::config::AppConfig;
use super::password_dialog::PasswordDialogState;

/// State for the vault list.
#[derive(Default)]
pub struct VaultListState {
    /// Whether each vault is unlocked (by index).
    unlocked: std::collections::HashSet<usize>,
}

impl VaultListState {
    pub fn unlock(&mut self, index: usize) {
        self.unlocked.insert(index);
    }

    pub fn lock(&mut self, index: usize) {
        self.unlocked.remove(&index);
    }

    pub fn is_unlocked(&self, index: usize) -> bool {
        self.unlocked.contains(&index)
    }
}

/// Render the vault list.
pub fn render(
    ui: &mut Ui,
    state: &mut VaultListState,
    config: &AppConfig,
    password_state: &mut PasswordDialogState,
) {
    ui.vertical_centered(|ui| {
        ui.heading("📁 我的保险箱");
    });

    ui.add_space(16.0);

    if config.vaults.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("📭 还没有保险箱").size(18.0).color(Color32::GRAY));
            ui.add_space(8.0);
            ui.label("点击「➕ 新建」标签页创建您的第一个加密保险箱");
        });
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, vault) in config.vaults.iter().enumerate() {
                let is_unlocked = state.is_unlocked(i);

                egui::Frame::NONE
                    .fill(if is_unlocked {
                        Color32::from_rgb(30, 60, 30)
                    } else {
                        Color32::from_rgb(35, 38, 45)
                    })
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Status indicator
                            if is_unlocked {
                                ui.label(RichText::new("🟢").size(20.0));
                            } else {
                                ui.label(RichText::new("🔒").size(20.0));
                            }

                            ui.vertical(|ui| {
                                ui.label(RichText::new(&vault.name).size(15.0).strong());
                                ui.label(
                                    RichText::new(&vault.path)
                                        .size(11.0)
                                        .color(Color32::GRAY),
                                );
                                if is_unlocked {
                                    ui.label(
                                        RichText::new(format!("挂载点: {}", vault.mount_point))
                                            .size(11.0)
                                            .color(Color32::GREEN),
                                    );
                                }
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if is_unlocked {
                                        if ui.button("🔒 锁定").clicked() {
                                            state.lock(i);
                                        }
                                        if ui.button("📂 打开").clicked() {
                                            // Open the mount point in file explorer
                                            let _ = open::that(&vault.mount_point);
                                        }
                                    } else {
                                        if ui.button("🔓 解锁").clicked() {
                                            password_state.open(i, &vault.name);
                                        }
                                    }
                                },
                            );
                        });
                    });

                ui.add_space(8.0);
            }
        });
}
