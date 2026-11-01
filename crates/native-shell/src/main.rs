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
mod filetree;
mod flow_ui;
mod glue;
mod glue_board;
mod glue_extras;
mod glue_review;
mod grid;
mod handoff;
mod history_ui;
mod keys;
mod layout;
mod memory;
mod memory_ui;
mod newtask;
mod newtask_props;
mod newtask_ui;
mod palette;
mod plan_board;
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
mod sessions_ui;
mod spawn;
mod start_ui;
mod states;
mod system;
mod taskopts;
mod tasks;
mod term_pane;
mod theme;
mod topbar;
mod usage_watch;

use app::ShellApp;
use theme::install_fonts;

fn main() -> eframe::Result<()> {
    // Started by Chrome as the extension's native messaging host: relay, no window.
    if browser_hub::host::wanted() {
        browser_hub::host::run();
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Maestro Native (prototype)",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(ShellApp::new().expect("spawn the shell")))
        }),
    )
}
