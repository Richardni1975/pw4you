//! About page UI.

use egui::{Color32, RichText, Ui};

/// Render the about page.
pub fn render(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);

        // App icon/logo
        ui.label(RichText::new("🔒").size(64.0));

        ui.add_space(8.0);
        ui.label(RichText::new("pw4you").size(32.0).strong());
        ui.label(RichText::new("文件夹保险箱 v0.1.0").size(14.0).color(Color32::GRAY));
        ui.add_space(16.0);
        ui.label("安全的文件夹加密工具");
        ui.add_space(24.0);

        // Features list
        egui::Frame::NONE
            .fill(Color32::from_rgb(35, 38, 45))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_width(400.0);
                ui.label(RichText::new("✨ 功能特性").size(16.0).strong());
                ui.add_space(8.0);
                ui.label("🔐 AES-256-GCM 军用级加密");
                ui.label("📅 动态密码 (日期 + 固定密码)");
                ui.label("🛡 防时钟篡改锁定机制");
                ui.label("🔢 n² 递增锁定时间");
                ui.label("💾 单文件容器，便于备份");
                ui.label("🖥 虚拟磁盘挂载");
                ui.label("📦 轻量级 (~5MB)");
            });

        ui.add_space(24.0);

        // Security notice
        ui.label(
            RichText::new("⚠ 安全提示")
                .size(14.0)
                .strong()
                .color(Color32::from_rgb(255, 200, 80)),
        );
        ui.add_space(4.0);
        ui.label("请务必记住您的固定密码。");
        ui.label("密码无法找回，忘记密码将导致数据永久丢失。");
        ui.label("建议定期备份 .pw4 容器文件到安全位置。");

        ui.add_space(16.0);
        ui.label(
            RichText::new("Built with ❤️ using Rust + egui")
                .size(11.0)
                .color(Color32::GRAY),
        );
    });
}
