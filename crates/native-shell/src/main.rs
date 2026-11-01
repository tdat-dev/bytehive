//! Maestro native shell: egui UI over the headless agent-core.

mod app;
mod board_ui;
mod browser_live;
mod browser_prefs;
mod browser_ui;
mod changes_ui;
mod color;
mod commands;
mod deliver;
mod diffview;
mod editor;
mod files_ui;
mod fleet;
mod filetree;
mod flow_ui;
mod glue;
mod glue_board;
mod glue_extras;
mod glue_review;
mod grid;
mod handoff;
mod history_task;
mod history_ui;
mod keys;
mod land_ui;
mod laws;
mod layout;
mod logic;
mod memory;
mod memory_ui;
mod notify;
mod newtask;
mod newtask_props;
mod newtask_ui;
mod palette;
mod paste;
mod plan_board;
mod planmeter;
mod polls;
mod prefs;
mod projects;
mod projects_ui;
mod queue_ui;
mod remote;
mod replay;
mod replay_ui;
mod remote_ui;
mod review_ui;
mod schedule;
mod send_ui;
mod settings_ui;
mod session_follow;
mod sessions_ui;
mod spawn;
mod start_ui;
mod states;
mod system;
mod taskopts;
mod tasks;
mod term_pane;
mod term_search;
mod term_select;
mod theme;
mod topbar;
mod tray;
mod usage_watch;

use app::ShellApp;

/// The app's Windows identity: toasts and the Start-menu shortcut share it
/// (the Tauri app's, so its shortcut and notification settings carry over).
pub const APP_ID: &str = "com.maestro.app";
use theme::install_fonts;

fn main() -> eframe::Result<()> {
    // Started by Chrome as the extension's native messaging host: relay, no window.
    if browser_hub::host::wanted() {
        browser_hub::host::run();
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]).with_icon(tray::window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Maestro Native (prototype)",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            let app = ShellApp::new().expect("spawn the shell");
            app.extras.tray.set_window(cc);
            Ok(Box::new(app))
        }),
    )
}
