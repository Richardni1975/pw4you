//! System tray — background icon with context menu.
//!
//! Uses winapi to create a tray icon in a background thread.
//! Communicates with the main app via channels.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Commands from tray to app.
#[derive(Debug, Clone)]
pub enum TrayCommand {
    Show,
    Hide,
    LockAll,
    Exit,
}

/// Commands from app to tray.
#[derive(Debug, Clone)]
pub enum TrayState {
    Visible,
    Hidden,
}

/// Start the system tray in a background thread.
///
/// Returns a `Receiver<TrayCommand>` that the app should poll in its update loop.
pub fn start_tray() -> Receiver<TrayCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<TrayCommand>();
    let (state_tx, state_rx) = mpsc::channel::<TrayState>();

    thread::spawn(move || {
        run_tray_loop(cmd_tx, state_rx);
    });

    // Set initial state
    let _ = state_tx.send(TrayState::Visible);

    cmd_rx
}

/// The tray event loop running in a background thread.
fn run_tray_loop(cmd_tx: Sender<TrayCommand>, state_rx: Receiver<TrayState>) {
    unsafe {
        use winapi::um::winuser::*;
        use winapi::um::shellapi::*;

        // Register a window class for the tray message window
        let class_name: Vec<u16> = "pw4you_tray_wnd\0".encode_utf16().collect();
        let hinstance = winapi::um::libloaderapi::GetModuleHandleW(std::ptr::null());

        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(tray_wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: (16 + 1) as _, // COLOR_BTNFACE + 1
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wc);

        // Create a hidden message-only window
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0, 0, 0, 0,
            HWND_MESSAGE, // Message-only window
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            return;
        }

        // Create tray icon
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_APP;

        let tip: Vec<u16> = "pw4you - \u{6587}\u{4ef6}\u{5939}\u{4fdd}\u{9669}\u{7bb1}\0".encode_utf16().collect();
        let len = tip.len().min(128);
        nid.szTip[..len].copy_from_slice(&tip[..len]);

        Shell_NotifyIconW(NIM_ADD, &mut nid);

        // Message loop
        let mut msg: MSG = std::mem::zeroed();
        loop {
            // Check for state updates (non-blocking)
            if let Ok(state) = state_rx.try_recv() {
                // Update tray tooltip based on state
                let _ = state; // Could update tooltip here
            }

            // Pump Windows messages with a timeout
            let has_msg = PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0;
            if has_msg {
                if msg.message == WM_QUIT {
                    break;
                }

                match msg.message {
                    // Tray icon messages
                    m if m == WM_APP => {
                        let lparam = msg.lParam as u32;
                        let wparam = msg.wParam;

                        match lparam {
                            WM_RBUTTONUP => {
                                show_tray_menu(hwnd, &cmd_tx);
                            }
                            WM_LBUTTONUP => {
                                let _ = cmd_tx.send(TrayCommand::Show);
                            }
                            _ => {}
                        }

                        // Handle menu commands (sent as WM_COMMAND to our window)
                        if lparam == WM_COMMAND {
                            let cmd_id = (wparam & 0xFFFF) as u32;
                            match cmd_id {
                                1 => { let _ = cmd_tx.send(TrayCommand::Show); }
                                2 => { let _ = cmd_tx.send(TrayCommand::Hide); }
                                3 => { let _ = cmd_tx.send(TrayCommand::LockAll); }
                                4 => { let _ = cmd_tx.send(TrayCommand::Exit); }
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            } else {
                // No message — sleep briefly to avoid busy-waiting
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // Cleanup
        Shell_NotifyIconW(NIM_DELETE, &mut nid);
        DestroyWindow(hwnd);
    }
}

/// Show tray context menu.
unsafe fn show_tray_menu(hwnd: winapi::shared::windef::HWND, _cmd_tx: &Sender<TrayCommand>) {
    use winapi::um::winuser::*;

    let mut pt = std::mem::zeroed();
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);

    let menu = CreatePopupMenu();

    let show_txt: Vec<u16> = "\u{663e}\u{793a}\u{7a97}\u{53e3}\0".encode_utf16().collect();
    let hide_txt: Vec<u16> = "\u{9690}\u{85cf}\u{7a97}\u{53e3}\0".encode_utf16().collect();
    let lock_txt: Vec<u16> = "\u{9501}\u{5b9a}\u{5168}\u{90e8}\0".encode_utf16().collect();
    let exit_txt: Vec<u16> = "\u{9000}\u{51fa}\0".encode_utf16().collect();

    AppendMenuW(menu, MF_STRING, 1, show_txt.as_ptr());
    AppendMenuW(menu, MF_STRING, 2, hide_txt.as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(menu, MF_STRING, 3, lock_txt.as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(menu, MF_STRING, 4, exit_txt.as_ptr());

    TrackPopupMenu(
        menu,
        TPM_BOTTOMALIGN | TPM_LEFTALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        std::ptr::null(),
    );

    DestroyMenu(menu);
}

/// Window procedure for the tray message window.
unsafe extern "system" fn tray_wndproc(
    hwnd: winapi::shared::windef::HWND,
    msg: u32,
    wparam: winapi::shared::minwindef::WPARAM,
    lparam: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::LRESULT {
    use winapi::um::winuser::*;
    // Forward WM_COMMAND from TrackPopupMenu to ourselves
    if msg == WM_COMMAND {
        // Re-post to be picked up in the message loop
        PostMessageW(hwnd, WM_APP, wparam, WM_COMMAND as isize);
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
