pub mod adopt;
pub mod browser;
pub mod claudetrust;
pub mod commands;
pub mod core;
pub mod dashauth;
pub mod dashboard;
pub mod error;
pub mod gitexclude;
pub mod land;
pub mod planusage;
pub mod agentusage;
pub mod state;
pub mod review;
pub mod tailfile;
pub mod toast;
pub mod transcripts;
pub mod worktree;

use crate::state::AppState;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

/// The app's name on screen. A dev build says so, so it can't be mistaken for
/// the installed app open beside it (src/appname.ts has the same split).
const APP_NAME: &str = if cfg!(debug_assertions) { "ByteHive Dev" } else { "ByteHive" };

/// Bring the main window back to the foreground (used by the tray icon + menu).
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Build the system-tray icon with a Show/Quit menu. Created hidden — the
/// frontend turns it on via `set_tray_visible` when "Hide to tray" is enabled.
fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", format!("Show {APP_NAME}"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(APP_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => {
                // Surface the window so the frontend's kill-all confirm is visible,
                // then let the frontend run its existing quit flow.
                show_main_window(app);
                let _ = app.emit("tray-quit", ());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    // Built visible by default; hide it until the frontend enables "Hide to tray".
    let tray = builder.build(app)?;
    tray.set_visible(false)?;
    Ok(())
}

/// Chromium switches that keep a hidden window working. The window is hidden to
/// WebView2 when minimized (see on_window_event below), and Chromium then runs a
/// page's timers once a second, and after five minutes once a minute. xterm
/// parses what it is given from a timer, so a minimized ByteHive with agents
/// printing stopped parsing, its backlog grew in the renderer (measured: +260 MB
/// in a minute) and, with output flow control, held every program back.
/// Frames still stop when hidden; only the timers keep their pace.
///
/// They go in through the environment rather than a window's own config because
/// every window that shares the profile (detached tabs, previews) must start
/// WebView2 with the same arguments, and the ones made from JS cannot set them.
/// The `--disable-features` list repeats wry's own defaults, since the last such
/// switch wins.
const WEBVIEW_KEEP_AWAKE: &str = "--disable-background-timer-throttling --disable-renderer-backgrounding --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,IntensiveWakeUpThrottling";

/// The value for WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS given what is there already
/// (a developer may have set it, for the remote debugging port).
pub fn webview_args(existing: Option<&str>) -> String {
    match existing.map(str::trim).filter(|s| !s.is_empty()) {
        Some(e) if e.contains(WEBVIEW_KEEP_AWAKE) => e.to_string(),
        Some(e) => format!("{e} {WEBVIEW_KEEP_AWAKE}"),
        None => WEBVIEW_KEEP_AWAKE.to_string(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    {
        // Before any window: single-threaded here, and every webview reads it when it starts.
        let key = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
        std::env::set_var(key, webview_args(std::env::var(key).ok().as_deref()));
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .manage(browser::BrowserState::default())
        // WebView2 keeps compositing at full rate while its window is minimized unless the
        // host says the view is hidden (Microsoft's guidance: IsVisible = false when
        // minimized). Measured 2026-10-10: a minimized ByteHive still kept the 3D engine
        // ~45% busy. Hidden, the page gets document.hidden, requestAnimationFrame stops
        // and xterm stops repainting; power.ts repaints on the way back.
        .on_window_event(|window, event| {
            #[cfg(windows)]
            if let tauri::WindowEvent::Resized(_) = event {
                if let Some(wv) = window.app_handle().get_webview_window(window.label()) {
                    let visible = !wv.is_minimized().unwrap_or(false);
                    let _ = wv.with_webview(move |pw| unsafe {
                        let _ = pw.controller().SetIsVisible(visible);
                    });
                }
            }
            #[cfg(not(windows))]
            let _ = (window, event);
        })
        .setup(|app| {
            adopt::adopt_old_home();
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_title(APP_NAME);
            }
            build_tray(app)?;
            // the dashboard password's hash lives with the app's config
            if let Ok(dir) = app.path().app_config_dir() {
                app.state::<AppState>().dashboard.auth.load_from(dir);
            }
            browser::start(app.handle(), &app.state::<browser::BrowserState>());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            adopt::adopt_old_folder,
            commands::pty_spawn,
            commands::pty_attach,
            commands::pty_ack,
            commands::pty_input,
            commands::pty_resize,
            commands::pty_kill,
            commands::pty_kill_all,
            commands::set_tray_visible,
            commands::set_tray_tooltip,
            toast::notify_open,
            commands::programs_on_path,
            commands::app_pid,
            commands::process_alive,
            commands::record_start,
            commands::record_stop,
            commands::record_read,
            commands::claude_usage,
            commands::claude_session_usage,
            planusage::claude_plan_usage,
            planusage::codex_plan_usage,
            agentusage::agent_usage,
            commands::claude_transcript,
            transcripts::codex::codex_transcript,
            transcripts::opencode::opencode_transcript,
            tailfile::fs_read_lines_from,
            tailfile::fs_truncate_if_size,
            commands::save_pasted_image,
            commands::save_pasted_file,
            commands::run_capture,
            commands::run_capture_status,
            commands::claude_session_exists,
            commands::claude_live_sessions,
            commands::claude_sessions,
            commands::claude_sessions_everywhere,
            commands::claude_sessions_in,
            claudetrust::claude_trust_folder,
            gitexclude::git_exclude_bytehive,
            worktree::git_repo_root,
            worktree::worktree_add,
            worktree::worktree_remove,
            worktree::worktree_leave,
            worktree::workspace_files,
            review::git_repos_under,
            review::repo_diff,
            review::git_changed_files,
            review::review_repo_info,
            review::review_commit,
            review::review_merge,
            review::review_discard,
            review::review_remove_worktree,
            land::review_land,
            land::review_discard_branch,
            land::review_branch_diff,
            core::fs::fs_read_dir,
            core::fs::fs_read_file,
            core::fs::fs_stat,
            core::fs::fs_write_file,
            core::fs::fs_read_data_url,
            core::fs::fs_create_file,
            core::fs::fs_create_dir,
            core::fs::fs_rename,
            core::fs::fs_delete,
            core::fs::fs_copy,
            core::fs::fs_move,
            core::fs::fs_trash,
            core::fs::fs_reveal,
            core::fs::fs_open_external,
            core::watch::watch_start,
            core::watch::watch_stop,
            core::screenshot::capture_window,
            dashboard::dashboard_status,
            dashboard::dashboard_start,
            dashboard::dashboard_stop,
            dashboard::dashboard_push,
            dashboard::dashboard_password_status,
            dashboard::dashboard_password_set,
            dashboard::dashboard_password_clear,
            browser::browser_status,
            browser::browser_profiles,
            browser::browser_open_profile,
            browser::browser_activity,
            browser::browser_pause,
            browser::browser_peek,
            browser::browser_asks,
            browser::browser_answer,
            browser::browser_set_ask,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod webview_args_tests {
    use super::{webview_args, WEBVIEW_KEEP_AWAKE};

    #[test]
    fn the_switches_go_in_when_nothing_was_set() {
        assert_eq!(webview_args(None), WEBVIEW_KEEP_AWAKE);
        assert_eq!(webview_args(Some("  ")), WEBVIEW_KEEP_AWAKE);
    }

    #[test]
    fn a_developers_own_arguments_are_kept_in_front() {
        let a = webview_args(Some("--remote-debugging-port=9333"));
        assert_eq!(a, format!("--remote-debugging-port=9333 {WEBVIEW_KEEP_AWAKE}"));
    }

    #[test]
    fn running_it_twice_does_not_repeat_the_switches() {
        let once = webview_args(None);
        assert_eq!(webview_args(Some(&once)), once);
    }

    #[test]
    fn timers_keep_running_hidden_and_wrys_own_defaults_stay() {
        for must in ["--disable-background-timer-throttling", "--disable-renderer-backgrounding", "IntensiveWakeUpThrottling", "msWebOOUI", "msPdfOOUI", "msSmartScreenProtection"] {
            assert!(WEBVIEW_KEEP_AWAKE.contains(must), "{must}");
        }
        assert_eq!(WEBVIEW_KEEP_AWAKE.matches("--disable-features=").count(), 1, "the last such switch wins, so there must be one");
    }
}
