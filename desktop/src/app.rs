//! Main application state and update loop.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;

use egui::Context;
use log::{error, info};

use crate::config::{AppConfig, VaultEntry};
use crate::gui;
use crate::mount::MountPoint;
use crate::tray::TrayCommand;

/// Main application state.
pub struct Pw4YouApp {
    /// Application configuration.
    config: AppConfig,
    /// Currently selected tab.
    active_tab: gui::Tab,
    /// State for the create vault wizard.
    create_state: gui::create_vault::CreateVaultState,
    /// State for the password dialog.
    password_state: gui::password_dialog::PasswordDialogState,
    /// State for the main vault list.
    vault_list_state: gui::vault_list::VaultListState,
    /// Status message to display.
    status_message: Option<String>,
    /// Error message to display.
    error_message: Option<String>,
    /// Currently mounted vaults (name → MountPoint).
    mounts: HashMap<String, MountPoint>,
    /// Tray command receiver.
    tray_rx: Receiver<TrayCommand>,
    /// Whether window is minimized to tray.
    is_minimized_to_tray: bool,
}

impl Pw4YouApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, tray_rx: Receiver<TrayCommand>) -> Self {
        info!("Initializing pw4you application");

        // Load configuration
        let config = AppConfig::load();
        info!("Loaded {} vault(s) from config", config.vaults.len());

        Self {
            config,
            active_tab: gui::Tab::Vaults,
            create_state: gui::create_vault::CreateVaultState::default(),
            password_state: gui::password_dialog::PasswordDialogState::default(),
            vault_list_state: gui::vault_list::VaultListState::default(),
            status_message: Some("欢迎使用 pw4you 文件夹保险箱".to_string()),
            error_message: None,
            mounts: HashMap::new(),
            tray_rx,
            is_minimized_to_tray: false,
        }
    }

    /// Show a status message in the bottom bar.
    fn set_status(&mut self, msg: &str) {
        self.status_message = Some(msg.to_string());
        self.error_message = None;
    }

    /// Show an error message in the bottom bar.
    fn set_error(&mut self, msg: &str) {
        self.error_message = Some(msg.to_string());
        self.status_message = None;
    }

    /// Attempt to create a new vault.
    fn create_vault(&mut self, name: String, path: String, password: String, mount_point: String) {
        info!("Creating vault '{}' at {}", name, path);

        let container_path = std::path::PathBuf::from(&path);
        if container_path.exists() {
            self.set_error(&format!("文件已存在: {}", path));
            return;
        }

        match pw4you_core::Vault::create(&container_path, &password) {
            Ok(mut vault) => {
                let password_hash = vault.password_hash().to_string();
                vault.close().ok();

                let entry = VaultEntry {
                    name: name.clone(),
                    path,
                    password_hash,
                    mount_point,
                };

                self.config.vaults.push(entry);
                if let Err(e) = self.config.save() {
                    self.set_error(&format!("配置保存失败: {}", e));
                } else {
                    self.set_status(&format!("保险箱 '{}' 创建成功！", name));
                    self.create_state.reset();
                    self.active_tab = gui::Tab::Vaults;
                }
            }
            Err(e) => {
                error!("Failed to create vault: {}", e);
                self.set_error(&format!("创建失败: {}", e));
            }
        }
    }

    /// Attempt to unlock a vault.
    fn unlock_vault(&mut self, vault_index: usize, password: &str) {
        if vault_index >= self.config.vaults.len() {
            self.set_error("保险箱不存在");
            return;
        }

        let entry_name = self.config.vaults[vault_index].name.clone();
        let entry_path = self.config.vaults[vault_index].path.clone();
        let entry_hash = self.config.vaults[vault_index].password_hash.clone();
        info!("Unlocking vault '{}'", entry_name);

        let container_path = std::path::PathBuf::from(&entry_path);
        if !container_path.exists() {
            self.set_error(&format!("文件不存在: {}", entry_path));
            return;
        }

        match pw4you_core::Vault::open(&container_path, &entry_hash) {
            Ok(mut vault) => {
                match vault.unlock(password) {
                    Ok(result) => match result {
                        pw4you_core::UnlockResult::Success => {
                            self.set_status(&format!("'{}' 已解锁！", entry_name));
                            // Mount the vault (extract to temp + open explorer)
                            match MountPoint::mount(&entry_name, &vault) {
                                Ok(mount) => {
                                    self.mounts.insert(entry_name.clone(), mount);
                                    self.vault_list_state.unlock(vault_index);
                                }
                                Err(e) => {
                                    self.set_error(&format!("挂载失败: {}", e));
                                }
                            }
                            self.password_state.close();
                        }
                        pw4you_core::UnlockResult::WrongPassword {
                            attempt_count,
                            lock_days,
                            ..
                        } => {
                            self.set_error(&format!(
                                "密码错误！(第{}次)\n此保险箱已被锁定 {} 天",
                                attempt_count, lock_days
                            ));
                            self.password_state.close();
                        }
                        pw4you_core::UnlockResult::LockedOut {
                            remaining_days, ..
                        } => {
                            self.set_error(&format!(
                                "保险箱已被锁定，还需等待 {} 天",
                                remaining_days
                            ));
                            self.password_state.close();
                        }
                        pw4you_core::UnlockResult::MaxAttempts {
                            attempt_count,
                            lock_days,
                        } => {
                            self.set_error(&format!(
                                "我没有猜错的话，你是小偷吧？\n\n连续错误 {} 次，锁定 {} 天",
                                attempt_count, lock_days
                            ));
                            self.password_state.close();
                        }
                        pw4you_core::UnlockResult::ClockRollback {
                            penalty_seconds,
                        } => {
                            self.set_error(&format!(
                                "⚠ 检测到系统时钟回拨！\n锁定时间延长 {} 秒作为惩罚。",
                                penalty_seconds
                            ));
                            self.password_state.close();
                        }
                    },
                    Err(e) => {
                        self.set_error(&format!("解锁失败: {}", e));
                        self.password_state.close();
                    }
                }
            }
            Err(e) => {
                self.set_error(&format!("无法打开保险箱: {}", e));
            }
        }
    }

    /// Lock a vault (unmount and secure-delete temp files).
    fn lock_vault(&mut self, vault_index: usize) {
        if vault_index >= self.config.vaults.len() {
            return;
        }
        let name = self.config.vaults[vault_index].name.clone();
        info!("Locking vault '{}'", name);

        if let Some(mut mount) = self.mounts.remove(&name) {
            if let Err(e) = mount.unmount() {
                error!("Failed to unmount '{}': {}", name, e);
            }
        }

        self.vault_list_state.lock(vault_index);
        self.set_status(&format!("'{}' 已锁定", name));
    }
}

impl Pw4YouApp {
    /// Poll tray commands and handle them.
    fn process_tray_commands(&mut self) {
        while let Ok(cmd) = self.tray_rx.try_recv() {
            match cmd {
                TrayCommand::Show => {
                    self.is_minimized_to_tray = false;
                }
                TrayCommand::Hide => {
                    self.is_minimized_to_tray = true;
                }
                TrayCommand::LockAll => {
                    // Lock all mounted vaults
                    let indices: Vec<usize> = self.config.vaults.iter()
                        .enumerate()
                        .filter(|(i, _)| self.vault_list_state.is_unlocked(*i))
                        .map(|(i, _)| i)
                        .collect();
                    for i in indices {
                        self.lock_vault(i);
                    }
                    self.set_status("所有保险箱已锁定");
                }
                TrayCommand::Exit => {
                    info!("Exit requested from tray");
                    std::process::exit(0);
                }
            }
        }
    }
}

impl eframe::App for Pw4YouApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Process tray commands
        self.process_tray_commands();

        // Handle minimize to tray
        if self.is_minimized_to_tray {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            // Show a notification in the tray (via status)
            self.is_minimized_to_tray = false; // Reset after minimizing
        }

        // Top panel — tab bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🔒 pw4you");
                ui.separator();

                ui.selectable_value(&mut self.active_tab, gui::Tab::Vaults, "📁 保险箱");
                ui.selectable_value(&mut self.active_tab, gui::Tab::Create, "➕ 新建");
                ui.selectable_value(&mut self.active_tab, gui::Tab::Settings, "⚙ 设置");
                ui.selectable_value(&mut self.active_tab, gui::Tab::About, "ℹ 关于");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mounted_count = self.mounts.len();
                    if mounted_count > 0 {
                        let names: Vec<&str> = self.mounts.keys().map(|s| s.as_str()).collect();
                        ui.label(format!("🟢 {}", names.join(", ")));
                        if ui.button("🔒 锁定全部").clicked() {
                            let indices: Vec<usize> = self.config.vaults.iter()
                                .enumerate()
                                .filter(|(_i, v)| self.mounts.contains_key(&v.name))
                                .map(|(i, _)| i)
                                .collect();
                            for i in indices {
                                self.lock_vault(i);
                            }
                        }
                    } else {
                        ui.label("🔴 全部锁定");
                    }
                });
            });
        });

        // Central panel — content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                gui::Tab::Vaults => {
                    gui::vault_list::render(
                        ui,
                        &mut self.vault_list_state,
                        &self.config,
                        &mut self.password_state,
                    );
                }
                gui::Tab::Create => {
                    gui::create_vault::render(ui, &mut self.create_state);
                }
                gui::Tab::Settings => {
                    gui::settings::render(ui, &mut self.config);
                }
                gui::Tab::About => {
                    gui::about::render(ui);
                }
            }
        });

        // Bottom panel — status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            if let Some(ref err) = self.error_message {
                ui.colored_label(egui::Color32::from_rgb(255, 80, 80), format!("❌ {}", err));
            } else if let Some(ref status) = self.status_message {
                ui.colored_label(egui::Color32::from_rgb(100, 200, 100), format!("✅ {}", status));
            }
        });

        // Password dialog (modal)
        if self.password_state.is_open {
            let mut should_unlock = false;
            let mut unlock_index = 0;
            let mut unlock_password = String::new();

            egui::Window::new("🔐 输入密码")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    let vault_name = self.password_state.vault_name.clone();
                    gui::password_dialog::render(ui, &mut self.password_state, &vault_name);

                    if self.password_state.unlock_requested {
                        should_unlock = true;
                        unlock_index = self.password_state.vault_index;
                        unlock_password = self.password_state.password.clone();
                    }
                });

            // Process unlock request outside the window closure
            if should_unlock {
                self.unlock_vault(unlock_index, &unlock_password);
            }
        }

        // Create vault confirmation dialog
        if self.create_state.show_confirm {
            let (name, path, password, mount) = (
                self.create_state.name.clone(),
                self.create_state.path.clone(),
                self.create_state.password.clone(),
                self.create_state.mount_point.clone(),
            );
            let mut should_create = false;
            let mut dialog_closed = false;

            egui::Window::new("✅ 确认创建")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("名称: {}", name));
                    ui.label(format!("路径: {}", path));
                    ui.label(format!("挂载点: {}", mount));
                    ui.separator();
                    ui.label("⚠ 请务必记住您的固定密码！");
                    ui.label("密码 = 日期(两位) + 固定密码");
                    ui.label("例如：今天是19号，密码就是 19固定密码");
                    ui.horizontal(|ui| {
                        if ui.button("取消").clicked() {
                            dialog_closed = true;
                        }
                        if ui.button("确认创建").clicked() {
                            should_create = true;
                            dialog_closed = true;
                        }
                    });
                });

            if dialog_closed {
                self.create_state.show_confirm = false;
                if should_create {
                    self.create_vault(name, path, password, mount);
                }
            }
        }

        // Request repaint every second for lock timer display
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        info!("pw4you shutting down");
        if let Err(e) = self.config.save() {
            error!("Failed to save config on exit: {}", e);
        }
    }
}
