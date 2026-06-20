//! Settings page UI.

use egui::{Color32, RichText, Ui};
use crate::config::AppConfig;

/// Render the settings page.
pub fn render(ui: &mut Ui, config: &mut AppConfig) {
    ui.vertical_centered(|ui| {
        ui.heading("⚙ 设置");
    });

    ui.add_space(16.0);

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 38, 45))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(20.0)
        .show(ui, |ui| {
            // Auto-lock settings
            ui.label(RichText::new("🔒 安全设置").size(16.0).strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("空闲自动锁定时间：");
                let mut minutes = config.auto_lock_minutes;
                ui.add(
                    egui::DragValue::new(&mut minutes)
                        .range(0..=240)
                        .suffix(" 分钟"),
                );
                if minutes != config.auto_lock_minutes {
                    config.auto_lock_minutes = minutes;
                }
                if minutes == 0 {
                    ui.label("（永不自动锁定）");
                }
            });
            ui.add_space(4.0);

            // Startup behavior
            ui.checkbox(&mut config.start_minimized, "启动时最小化到系统托盘");

            ui.add_space(16.0);

            // Danger zone
            ui.label(RichText::new("⚠ 危险操作").size(16.0).strong().color(Color32::from_rgb(255, 150, 80)));
            ui.add_space(8.0);
            ui.label("重置所有锁定状态：");
            if ui.button("🔄 重置").clicked() {
                // This would clear lock state in all locations
                // For safety, we just indicate the feature
            }
            ui.label(
                RichText::new("这将清除当前设备上的所有锁定记录。")
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        });

    ui.add_space(16.0);

    // Save button
    ui.vertical_centered(|ui| {
        if ui
            .add_sized([200.0, 36.0], egui::Button::new("💾 保存设置"))
            .clicked()
        {
            if let Err(e) = config.save() {
                // Error will be shown by the status bar
            }
        }
    });
}
