//! Windows Shell Extension — registers .pw4 file association.
//!
//! On first run, registers:
//! - `.pw4` file extension with "pw4you" progid
//! - Double-click association to open with this app

use log::info;
use std::ffi::CString;

/// Register .pw4 file extension and shell integration.
pub fn register() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Cannot get exe path: {}", e))?;
    let exe_path_str = exe_path.to_string_lossy();

    unsafe {
        // Helper: create a registry key
        macro_rules! reg_key {
            ($path:expr, $write:expr) => {{
                let mut key: winapi::shared::minwindef::HKEY = std::ptr::null_mut();
                let path_c = CString::new($path).unwrap();
                let ret = winapi::um::winreg::RegCreateKeyExA(
                    winapi::um::winreg::HKEY_CURRENT_USER,
                    path_c.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    if $write { 0x2001F } else { 0x20019 }, // KEY_ALL_ACCESS or KEY_READ
                    std::ptr::null_mut(),
                    &mut key,
                    std::ptr::null_mut(),
                );
                if ret == 0 { Some(key) } else { None }
            }};
        }

        // Helper: set a string value
        macro_rules! set_value {
            ($key:expr, $name:expr, $value:expr) => {{
                let value = format!("{}\0", $value);
                winapi::um::winreg::RegSetValueExA(
                    $key,
                    if $name.is_empty() { std::ptr::null() } else { CString::new($name).unwrap().as_ptr() },
                    0,
                    winapi::um::winnt::REG_SZ,
                    value.as_ptr() as *const u8,
                    value.len() as u32,
                )
            }};
        }

        // 1. .pw4 → pw4you.vault
        if let Some(key) = reg_key!("Software\\Classes\\.pw4", true) {
            set_value!(key, "", "pw4you.vault");
            winapi::um::winreg::RegCloseKey(key);
        }

        // 2. pw4you.vault description
        if let Some(key) = reg_key!("Software\\Classes\\pw4you.vault", true) {
            set_value!(key, "", "pw4you 保险箱");
            winapi::um::winreg::RegCloseKey(key);
        }

        // 3. DefaultIcon
        if let Some(key) = reg_key!("Software\\Classes\\pw4you.vault\\DefaultIcon", true) {
            set_value!(key, "", format!("{},0", exe_path_str));
            winapi::um::winreg::RegCloseKey(key);
        }

        // 4. shell\open\command
        if let Some(key) = reg_key!("Software\\Classes\\pw4you.vault\\shell\\open\\command", true) {
            set_value!(key, "", format!("\"{}\" \"%1\"", exe_path_str));
            winapi::um::winreg::RegCloseKey(key);
        }

        info!("Shell extension registered for .pw4 files");
    }

    Ok(())
}

/// Unregister the shell extension.
#[allow(dead_code)]
pub fn unregister() -> Result<(), String> {
    unsafe {
        // Delete the progid tree
        let path = CString::new("Software\\Classes\\pw4you.vault").unwrap();
        winapi::um::winreg::RegDeleteTreeA(
            winapi::um::winreg::HKEY_CURRENT_USER,
            path.as_ptr(),
        );

        // Clear .pw4 default value
        let pw4_path = CString::new("Software\\Classes\\.pw4").unwrap();
        let mut key: winapi::shared::minwindef::HKEY = std::ptr::null_mut();
        if winapi::um::winreg::RegOpenKeyExA(
            winapi::um::winreg::HKEY_CURRENT_USER,
            pw4_path.as_ptr(),
            0,
            0x2001F,
            &mut key,
        ) == 0
        {
            winapi::um::winreg::RegDeleteValueA(key, std::ptr::null());
            winapi::um::winreg::RegCloseKey(key);
        }
    }

    Ok(())
}
