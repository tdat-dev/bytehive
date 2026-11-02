//! ByteHive native shell: egui UI over the headless agent-core.
// A windowed app: no console window opens beside it. Debug builds keep the
// console for their logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod board_ui;
mod browser_live;
mod browser_prefs;
mod browser_ui;
mod changes_ui;
mod chrome;
mod color;
mod commands;
mod face;
mod icons;
mod deliver;
mod diffview;
mod dock;
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
mod inbox;
mod history_ui;
mod keys;
mod land_ui;
mod laws;
mod logo;
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
mod plan_edit;
mod planmeter;
mod polls;
mod prefs;
mod projects;
mod projects_ui;
mod queue_ui;
mod remote;
mod rail;
mod reopen;
mod single;
mod tabs;
mod replay;
mod replay_ui;
mod remote_ui;
mod review_ui;
mod schedule;
mod settings_ui;
mod session_follow;
mod sessions_ui;
mod spawn;
mod start_ui;
mod stage_ui;
mod states;
mod style;
mod system;
mod taskopts;
mod tasks;
mod term_paint;
mod term_pane;
mod term_search;
mod term_select;
mod theme;
mod topbar;
mod touched;
mod tray;
mod updater;
mod usage_watch;

use app::ShellApp;

/// The app's Windows identity: toasts and the Start-menu shortcut share it
/// (the Tauri app's, so its shortcut and notification settings carry over).
pub const APP_ID: &str = "com.bytehive.app";
use theme::install_fonts;

fn main() -> eframe::Result<()> {
    // Started by Chrome as the extension's native messaging host: relay, no window.
    if browser_hub::host::wanted() {
        browser_hub::host::run();
    }
    // One ByteHive at a time: a second start shows the first and quits.
    // Debug builds skip it, so trial runs never get in each other's way.
    if !cfg!(debug_assertions) && single::another_is_running() {
        return Ok(());
    }
    projects::adopt_old_home();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]).with_min_inner_size([900.0, 560.0]).with_decorations(false).with_icon(tray::window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "ByteHive",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            style::install_style(&cc.egui_ctx);
            let app = ShellApp::new().expect("spawn the shell");
            app.extras.tray.set_window(cc);
            Ok(Box::new(app))
        }),
    )
}
