//! A Windows notification that opens what it is about. The notification
//! plugin's toasts can't say they were clicked on desktop, so these are shown
//! straight through WinRT with an `Activated` handler: a click brings Maestro
//! forward and emits `notification-open` with the agent id, which the frontend
//! turns into showing that agent.

use tauri::{AppHandle, Emitter, Manager};
use tauri_winrt_notification::{Sound, Toast};

use crate::error::CommandError;

/// The Start-menu identity the installed app's toasts are filed under. A dev
/// build (run from target\) has no shortcut with that id, so it borrows
/// PowerShell's, as the notification plugin does.
fn app_id(app: &AppHandle) -> String {
    let dev = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.display().to_string()))
        .map(|d| {
            let sep = std::path::MAIN_SEPARATOR;
            d.ends_with(&format!("{sep}target{sep}debug")) || d.ends_with(&format!("{sep}target{sep}release"))
        })
        .unwrap_or(false);
    if dev {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    }
}

#[tauri::command]
pub fn notify_open(app: AppHandle, title: String, body: String, target: Option<String>) -> Result<(), CommandError> {
    let id = app_id(&app);
    // WinRT calls block for a moment; keep them off the main thread.
    std::thread::spawn(move || {
        let handle = app.clone();
        let toast = Toast::new(&id)
            .title(&title)
            .text1(&body)
            .sound(Some(Sound::Default))
            .on_activated(move |_| {
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                let _ = handle.emit("notification-open", target.clone());
                Ok(())
            });
        if let Err(e) = toast.show() {
            eprintln!("notification failed: {e}");
        }
    });
    Ok(())
}
