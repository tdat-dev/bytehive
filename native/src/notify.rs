//! Windows notifications that open what they are about (src-tauri toast.rs):
//! shown straight through WinRT with an Activated handler. A click puts the
//! agent's id in a queue the Inbox reads each tick, and it brings ByteHive
//! forward and shows that agent.

use std::sync::Mutex;

use tauri_winrt_notification::{Sound, Toast};

/// The installed app's identity: the installer registers it under
/// HKCU\Software\Classes\AppUserModelId with ByteHive's name and icon.
const APP_ID: &str = "com.bytehive.app";

static CLICKED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A build run from target\ has no registered identity, so it borrows
/// PowerShell's, as the Tauri app's dev build does.
fn app_id() -> &'static str {
    if crate::updater::dev_build() { Toast::POWERSHELL_APP_ID } else { APP_ID }
}

/// Show a notification; a click opens the agent `open`.
pub fn show(title: String, body: String, open: String) {
    // WinRT calls block for a moment; keep them off the UI thread.
    std::thread::spawn(move || {
        let toast = Toast::new(app_id()).title(&title).text1(&body).sound(Some(Sound::Default)).on_activated(move |_| {
            if let Ok(mut q) = CLICKED.lock() {
                q.push(open.clone());
            }
            Ok(())
        });
        if let Err(e) = toast.show() {
            eprintln!("notification failed: {e}");
        }
    });
}

/// The agent whose notification was clicked last, once.
pub fn take_clicked() -> Option<String> {
    CLICKED.lock().ok().and_then(|mut q| q.drain(..).last())
}

/// ByteHive's window is the one in front (document.hasFocus in the Tauri app).
pub fn in_front() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    let mut pid = 0u32;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    pid == std::process::id()
}
