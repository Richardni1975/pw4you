//! pw4you Desktop Application — Entry Point
//!
//! Windows desktop application for managing encrypted vaults.
//! Built with egui/eframe for a native GUI experience.

#![windows_subsystem = "windows"]

mod app;
mod gui;
mod config;
mod tray;
mod mount;
mod shell_ext;

use log::info;

fn main() {
    // ── Crash guard: show panic errors in a message box ──
    #[cfg(windows)]
    {
        std::panic::set_hook(Box::new(|info| {
            let msg = format!("pw4you 发生了错误:\n\n{}\n\n请将此信息反馈给开发者。", info);
            unsafe {
                use winapi::um::winuser::MessageBoxW;
                let title: Vec<u16> = "pw4you 错误\0".encode_utf16().collect();
                let body: Vec<u16> = msg.encode_utf16().collect();
                MessageBoxW(
                    std::ptr::null_mut(),
                    body.as_ptr(),
                    title.as_ptr(),
                    0x10, // MB_ICONERROR
                );
            }
        }));
    }

    // Initialize logging to temp file so we can debug startup issues
    let log_path = std::env::temp_dir().join("pw4you.log");
    let _ = std::fs::write(&log_path, ""); // clear old log
    let log_file = Box::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .expect("Failed to create log file")
    );
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .target(env_logger::Target::Pipe(log_file))
        .init();

    info!("pw4you starting... log at {:?}", log_path);

    // Start system tray
    let tray_rx = tray::start_tray();

    // Register shell extension (.pw4 file association)
    let _ = shell_ext::register();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "pw4you - 文件夹保险箱",
        native_options,
        Box::new(move |cc| {
            setup_style(&cc.egui_ctx);
            Ok(Box::new(app::Pw4YouApp::new(cc, tray_rx)))
        }),
    )
    .expect("Failed to start pw4you");
}

fn load_icon() -> egui::IconData {
    let icon_size = 16;
    let mut rgba = vec![0u8; icon_size * icon_size * 4];

    for y in 0..icon_size {
        for x in 0..icon_size {
            let idx = (y * icon_size + x) * 4;
            let cx = 8.0;
            let cy = 5.0;
            let outer_r = 6.0;
            let inner_r = 4.0;
            let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();

            if dist < outer_r {
                rgba[idx] = 30;
                rgba[idx + 1] = 60;
                rgba[idx + 2] = 120;
                rgba[idx + 3] = 255;
            }
            if dist < inner_r && y < 10 {
                rgba[idx] = 80;
                rgba[idx + 1] = 140;
                rgba[idx + 2] = 220;
                rgba[idx + 3] = 255;
            }
            if y >= 8 && y < 14 && x >= 4 && x <= 11 {
                rgba[idx] = 30;
                rgba[idx + 1] = 60;
                rgba[idx + 2] = 120;
                rgba[idx + 3] = 255;
            }
            if y >= 9 && y <= 11 && x >= 6 && x <= 9 {
                rgba[idx] = 200;
                rgba[idx + 1] = 200;
                rgba[idx + 2] = 200;
                rgba[idx + 3] = 255;
            }
        }
    }

    egui::IconData {
        rgba,
        width: icon_size as u32,
        height: icon_size as u32,
    }
}

fn setup_style(ctx: &egui::Context) {
    // ── Load Chinese font ─────────────────────────
    setup_chinese_font(ctx);

    // ── Visual style ──────────────────────────────
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.window_corner_radius = egui::CornerRadius::same(8);
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(40, 44, 52);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(50, 55, 65);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(70, 75, 85);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 65, 140);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(60, 100, 200);
    ctx.set_style(style);
}

/// Load Microsoft YaHei (微软雅黑) as the primary UI font.
/// Falls back gracefully if the font file is not found.
fn setup_chinese_font(ctx: &egui::Context) {
    if let Some(font_bytes) = load_system_chinese_font() {
        // Validate font before passing to egui
        if !is_valid_font(&font_bytes) {
            info!("Font validation failed, skipping");
            return;
        }

        let mut fonts = egui::FontDefinitions::default();

        fonts.font_data.insert(
            "chinese".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(font_bytes)),
        );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "chinese".to_owned());

        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "chinese".to_owned());

        ctx.set_fonts(fonts);
        info!("Chinese font loaded successfully");
    } else {
        info!("Chinese font not found, using default");
    }
}

/// Check if font bytes start with a valid TTF/OTF magic sequence.
fn is_valid_font(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    // TrueType: \x00\x01\x00\x00
    if data[0] == 0x00 && data[1] == 0x01 && data[2] == 0x00 && data[3] == 0x00 {
        return true;
    }
    // OpenType CFF: OTTO
    if &data[0..4] == b"OTTO" {
        return true;
    }
    // TrueType (Apple): true
    if &data[0..4] == b"true" {
        return true;
    }
    // PostScript in SFNT: typ1
    if &data[0..4] == b"typ1" {
        return true;
    }
    false
}

/// Try to load a Chinese-capable font from the Windows system.
///
/// Strategy: list all .ttf/.ttc/.otf files in C:\Windows\Fonts,
/// try each one, return the first that works for CJK.
fn load_system_chinese_font() -> Option<Vec<u8>> {
    let fonts_dir = std::path::Path::new("C:\\Windows\\Fonts");

    if let Ok(entries) = std::fs::read_dir(fonts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            match ext.as_str() {
                "ttf" | "otf" => {
                    // Plain font file — try it directly
                    if let Ok(data) = std::fs::read(&path) {
                        if is_valid_font(&data) {
                            let fname = path.file_name().unwrap().to_string_lossy();
                            info!("Loading font: {} ({} KB)", fname, data.len()/1024);
                            return Some(data);
                        }
                    }
                }
                "ttc" => {
                    // TrueType Collection — extract the first face
                    if let Ok(ttc_data) = std::fs::read(&path) {
                        let fname = path.file_name().unwrap().to_string_lossy();
                        if let Some(font_data) = extract_first_from_ttc(&ttc_data) {
                            info!("Extracted font from {} ({} KB)", fname, font_data.len()/1024);
                            return Some(font_data);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Fallback: try known paths in order
    info!("Font directory scan failed, trying known paths...");
    let fallbacks = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "C:\\Windows\\Fonts\\msjh.ttc",
    ];

    for path_str in &fallbacks {
        let path = std::path::Path::new(path_str);
        if let Ok(data) = std::fs::read(path) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("ttc") {
                if let Some(font_data) = extract_first_from_ttc(&data) {
                    return Some(font_data);
                }
            } else if is_valid_font(&data) {
                return Some(data);
            }
        }
    }

    None
}

/// Extract the first TTF/OTF font from a TrueType Collection (TTC) file.
///
/// TTC format:
/// ```text
/// Offset 0:   tag "ttcf" (4 bytes, big-endian)
/// Offset 4:   version (u32, 0x00010000 or 0x00020000)
/// Offset 8:   numFonts (u32)
/// Offset 12:  offsetTable[numFonts] (u32 array — byte offsets to each embedded font)
/// [v2 only]:  dsigTag + dsigLength + dsigOffset (12 bytes after offsetTable)
/// ```
///
/// Each offset in the table points to a complete, standalone TTF/OTF font file
/// embedded within the TTC. We extract the first one.
fn extract_first_from_ttc(ttc_data: &[u8]) -> Option<Vec<u8>> {
    // Minimum size: tag(4) + version(4) + numFonts(4) + one offset(4) = 16
    if ttc_data.len() < 16 {
        return None;
    }

    // Check TTC magic
    if &ttc_data[0..4] != b"ttcf" {
        return None;
    }

    let num_fonts = read_u32_be(ttc_data, 8)? as usize;
    if num_fonts == 0 || num_fonts > 100 {
        return None;
    }

    // Read the first font offset (starts at byte 12 in both v1 and v2 headers)
    let first_offset = read_u32_be(ttc_data, 12)? as usize;
    if first_offset >= ttc_data.len() || first_offset < 16 {
        return None;
    }

    // Determine where the first font ends
    let end = if num_fonts > 1 {
        // There's a second font — first font ends at second font's offset
        let second_off = read_u32_be(ttc_data, 16)? as usize;
        if second_off > first_offset && second_off <= ttc_data.len() {
            second_off
        } else {
            ttc_data.len()
        }
    } else {
        // Only one font — it goes to end of file
        ttc_data.len()
    };

    let font_bytes = ttc_data[first_offset..end].to_vec();

    // Validate the extracted data
    if is_valid_font(&font_bytes) {
        Some(font_bytes)
    } else {
        // If the first extraction fails, maybe the TTC has a different layout
        // Try: extract from first_offset to end of file
        let font_bytes_full = ttc_data[first_offset..].to_vec();
        if is_valid_font(&font_bytes_full) {
            return Some(font_bytes_full);
        }
        None
    }
}

/// Read a big-endian u32 at the given byte offset.
fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}
