//! Create vault wizard UI.

use egui::{Color32, RichText, Ui};

/// State for the create vault wizard.
#[derive(Default)]
pub struct CreateVaultState {
    /// Vault display name.
    pub name: String,
    /// File path for the .pw4 container.
    pub path: String,
    /// Fixed password.
    pub password: String,
    /// Confirm password.
    pub confirm_password: String,
    /// Mount point (drive letter, e.g., "P:").
    pub mount_point: String,
    /// Whether to show confirm dialog.
    pub show_confirm: bool,
    /// Whether to proceed with creation (set by confirm dialog).
    pub pending_create: bool,
    /// Error message for validation.
    pub validation_error: Option<String>,
}

impl CreateVaultState {
    pub fn reset(&mut self) {
        self.name.clear();
        self.path.clear();
        self.password.clear();
        self.confirm_password.clear();
        self.mount_point = "P:".to_string();
        self.show_confirm = false;
        self.pending_create = false;
        self.validation_error = None;
    }
}

/// Render the create vault form.
pub fn render(ui: &mut Ui, state: &mut CreateVaultState) {
    ui.vertical_centered(|ui| {
        ui.heading("➕ 创建新保险箱");
    });

    ui.add_space(16.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgb(35, 38, 45))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(20.0)
                .show(ui, |ui| {
                    // Vault name
                    ui.label(RichText::new("保险箱名称").strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut state.name)
                            .hint_text("例如：我的文档")
                            .desired_width(400.0),
                    );
                    ui.add_space(12.0);

                    // Container path
                    ui.label(RichText::new("保存位置").strong());
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.path)
                                .hint_text("例如：D:\\我的保险箱.pw4")
                                .desired_width(340.0),
                        );
                        if ui.button("📂 浏览").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("新保险箱.pw4")
                                .add_filter("pw4you 保险箱", &["pw4"])
                                .save_file()
                            {
                                state.path = path.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.add_space(12.0);

                    // Mount point
                    ui.label(RichText::new("挂载盘符").strong());
                    egui::ComboBox::from_label("")
                        .selected_text(&state.mount_point)
                        .show_ui(ui, |ui| {
                            for letter in 'P'..='Z' {
                                let drive = format!("{}:", letter);
                                ui.selectable_value(
                                    &mut state.mount_point,
                                    drive.clone(),
                                    &drive,
                                );
                            }
                        });
                    ui.add_space(12.0);

                    // Password
                    ui.label(RichText::new("固定密码").strong());
                    ui.label(
                        RichText::new("至少6位，无需包含日期")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut state.password)
                            .password(true)
                            .hint_text("至少6位字符")
                            .desired_width(300.0),
                    );
                    ui.add_space(8.0);

                    // Confirm password
                    ui.label(RichText::new("确认密码").strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut state.confirm_password)
                            .password(true)
                            .hint_text("再次输入密码")
                            .desired_width(300.0),
                    );
                    ui.add_space(16.0);

                    // Password explanation
                    ui.label(
                        RichText::new(
                            "💡 实际密码 = 当天日期(两位) + 您的固定密码\n   例如：19号时，密码为 19您的固定密码",
                        )
                        .size(12.0)
                        .color(Color32::from_rgb(180, 180, 100)),
                    );
                    ui.add_space(12.0);

                    // Validation errors
                    if let Some(ref err) = state.validation_error {
                        ui.label(
                            RichText::new(format!("⚠ {}", err))
                                .color(Color32::from_rgb(255, 80, 80)),
                        );
                    }
                    ui.add_space(8.0);

                    // Create button
                    let can_create = ui
                        .add_sized(
                            [200.0, 40.0],
                            egui::Button::new(
                                RichText::new("🔒 创建加密保险箱").size(16.0),
                            ),
                        )
                        .clicked();

                    if can_create {
                        // Validate
                        state.validation_error = None;
                        if state.name.trim().is_empty() {
                            state.validation_error = Some("请输入保险箱名称".to_string());
                        } else if state.path.trim().is_empty() {
                            state.validation_error = Some("请选择保存位置".to_string());
                        } else if state.password.len() < 6 {
                            state.validation_error = Some("密码至少需要6位".to_string());
                        } else if state.password != state.confirm_password {
                            state.validation_error = Some("两次输入的密码不一致".to_string());
                        } else {
                            state.show_confirm = true;
                        }
                    }
                });
        });
}
