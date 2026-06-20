//! Password input dialog for unlocking vaults.

use egui::Ui;

/// State for the password dialog.
#[derive(Default)]
pub struct PasswordDialogState {
    /// Whether the dialog is currently open.
    pub is_open: bool,
    /// Index of the vault being unlocked.
    pub vault_index: usize,
    /// Display name of the vault.
    pub vault_name: String,
    /// Password input buffer.
    pub password: String,
    /// Whether to show the password.
    pub show_password: bool,
    /// Today's day hint.
    pub day_hint: String,
    /// Set to true when the user clicks "Unlock" — processed by app.rs.
    pub unlock_requested: bool,
}

impl PasswordDialogState {
    /// Open the dialog for a specific vault.
    pub fn open(&mut self, index: usize, name: &str) {
        self.is_open = true;
        self.vault_index = index;
        self.vault_name = name.to_string();
        self.password = String::new();
        self.show_password = false;
        self.unlock_requested = false;

        // Generate day hint
        let now = chrono::Local::now();
        let day = now.format("%d").to_string();
        self.day_hint = format!("今天是 {} 号，请输入您的固定密码", day);
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.is_open = false;
        self.password = String::new();
        self.unlock_requested = false;
    }
}

/// Render the password dialog.
pub fn render(ui: &mut Ui, state: &mut PasswordDialogState, vault_name: &str) {
    ui.label(format!("保险箱: {}", vault_name));
    ui.add_space(8.0);

    ui.label(&state.day_hint);
    ui.label("密码格式: 日期 + 固定密码");
    ui.add_space(8.0);

    // Password input
    ui.horizontal(|ui| {
        let password_response = ui.add_sized(
            [250.0, 32.0],
            egui::TextEdit::singleline(&mut state.password)
                .password(!state.show_password)
                .hint_text("输入固定密码（不含日期）")
                .desired_width(250.0),
        );
        password_response.request_focus();

        // Show/hide password toggle
        ui.checkbox(&mut state.show_password, "👁");
    });

    ui.add_space(12.0);

    ui.horizontal(|ui| {
        if ui.button("取消").clicked() {
            state.close();
        }

        let unlock_clicked = ui
            .add_sized(
                [120.0, 32.0],
                egui::Button::new("🔓 解锁"),
            )
            .clicked();

        if unlock_clicked && !state.password.is_empty() {
            state.unlock_requested = true;
        }
    });

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("💡 提示：密码 = 今天的日期(如19) + 您设置的固定密码")
            .size(11.0)
            .color(egui::Color32::GRAY),
    );
}
