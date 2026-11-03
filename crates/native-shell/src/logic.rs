//! What ByteHive does whether or not its window shows: read the agents'
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
        // The window title stays "ByteHive". It used to follow the agents'
        // terminal titles, and Claude spins one ten times a second while it
        // works: every rename made Explorer re-list the window, which put
        // ByteHive back into an Alt+Tab filtered to another monitor
        // (Windhawk's Alt+Tab per monitor) and kept the taskbar redrawing.
        for pane in &mut self.panes {
            pane.pending_title = None;
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
        self.save_open_agents();
        if self.panes.iter().any(|p| p.exit_code.is_none()) {
            // Poll the PTYs at ~30fps while shown (eframe slows this down by
            // itself while the window is hidden); try_recv is cheap when idle.
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
}
