//! Maestro native shell: egui UI over the headless agent-core.

mod app;
mod color;
mod history_ui;
mod keys;
mod polls;
mod queue_ui;
mod review_ui;
mod sessions_ui;
mod spawn;
mod states;
mod term_pane;
mod theme;

use app::ShellApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Maestro Native (prototype)",
        options,
        Box::new(|_cc| Ok(Box::new(ShellApp::new().expect("spawn the shell")))),
    )
}
