//! Maestro native shell: egui UI over the headless agent-core.

mod app;
mod color;
mod flow_ui;
mod grid;
mod history_ui;
mod keys;
mod layout;
mod polls;
mod queue_ui;
mod review_ui;
mod send_ui;
mod sessions_ui;
mod spawn;
mod states;
mod term_pane;
mod theme;

use app::ShellApp;
use theme::install_fonts;

fn main() -> eframe::Result<()> {
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
