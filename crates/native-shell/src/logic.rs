//! What Maestro does whether or not its window shows: read the agents'
//! output, type tasks in when they are free, run schedules, hand off and
//! resume on limits, flash the taskbar, ask for plan limits, hide to the
//! tray. eframe calls [`ShellApp::tick`] through `App::logic` before every
//! frame and also while the window is minimized or hidden, when `ui` is not
//! called at all; nothing here may paint.

use std::time::Duration;

use agent_core::lock;
use eframe::egui;

use crate::app::ShellApp;

impl ShellApp {
    pub(crate) fn tick(&mut self, ctx: &egui::Context) {
        for pane in &mut self.panes {
            pane.drain(&mut lock(&self.registry));
        }
        for pane in &mut self.panes {
            if let Some(t) = pane.pending_title.take() {
                // Shells report their exe path; show the pane name instead.
                let boring = t.contains('\\') || t.contains('/') || t.ends_with(".exe");
                let title = if boring { pane.name.clone() } else { t.chars().take(48).collect() };
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{title} - Maestro")));
            }
        }
        self.poll_captures();
        self.poll_git();
        self.poll_stuck();
        self.poll_launches();
        self.poll_deliveries();
        self.poll_usage();
        self.poll_handoffs();
        self.handover_on_limit();
        self.watch_attention(ctx);
        self.poll_remote(ctx);
        self.extras.plan.poll(ctx);
        self.poll_tray(ctx);
        self.poll_notify(ctx);
        self.poll_fleet();
        self.poll_session_follow();
        self.poll_updates(ctx);
        self.poll_schedules(ctx);
        if self.panes.iter().any(|p| p.exit_code.is_none()) {
            // Poll the PTYs at ~30fps while shown (eframe slows this down by
            // itself while the window is hidden); try_recv is cheap when idle.
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
}
