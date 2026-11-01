//! The window and the OS: closing ByteHive while agents work asks first (they
//! stop with it; their sessions resume later from Sessions), and an agent
//! that starts needing you while ByteHive is in the background flashes the
//! taskbar button.

use std::collections::HashSet;

use eframe::egui::{self, Color32, RichText, Stroke};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::states::pane_status;
use crate::theme::{DEL, INK3, R_XL, SHEET, TEXT};

#[derive(Default)]
pub struct SystemWatch {
    /// The close was held to ask first.
    pub asking_quit: bool,
    /// You said quit: let the next close through.
    pub quit_ok: bool,
    /// Agents that needed you at the last look.
    needs: HashSet<String>,
}

/// Whether closing should ask first: only while agents are alive.
pub fn should_ask(live_agents: usize, quit_ok: bool) -> bool {
    live_agents > 0 && !quit_ok
}

/// The agents that newly need you, given who needed you before.
pub fn newly_needing(before: &HashSet<String>, now: &HashSet<String>) -> usize {
    now.difference(before).count()
}

impl ShellApp {
    fn live_agents(&self) -> usize {
        self.panes.iter().filter(|p| p.cli.is_some() && p.exit_code.is_none()).count()
    }

    /// Hold a close while agents are live and ask first (ui pass).
    pub(crate) fn poll_system(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && should_ask(self.live_agents(), self.extras.system.quit_ok) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.extras.system.asking_quit = true;
        }
        self.quit_sheet(ctx);
    }

    /// Flash the taskbar for an agent that newly needs you while ByteHive is
    /// in the background, minimized included (logic pass).
    pub(crate) fn watch_attention(&mut self, ctx: &egui::Context) {
        let now = now_ms();
        let needs: HashSet<String> = self
            .panes
            .iter()
            .filter(|p| p.cli.is_some())
            .filter(|p| matches!(pane_status(p, now, None).state, TaskState::Needs | TaskState::Blocked))
            .map(|p| p.id.clone())
            .collect();
        if Self::away(ctx) && newly_needing(&self.extras.system.needs, &needs) > 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
            if self.prefs.notify_needs {
                for p in self.panes.iter().filter(|p| needs.contains(&p.id) && !self.extras.system.needs.contains(&p.id)) {
                    let ask = pane_status(p, now, None).ask.map(|a| a.prompt).unwrap_or_default();
                    let project = crate::projects::project_name(&crate::projects::pane_home(p));
                    let body = if ask.is_empty() { format!("In {project}.") } else { ask };
                    self.toast(ctx, format!("{} needs you", p.name), body, Some(p.id.clone()));
                }
            }
        }
        self.extras.system.needs = needs;
    }

    fn quit_sheet(&mut self, ctx: &egui::Context) {
        if !self.extras.system.asking_quit {
            return;
        }
        let n = self.live_agents();
        let frame = egui::Frame::new().fill(SHEET).corner_radius(R_XL).stroke(Stroke::new(1.0, Color32::from_white_alpha(20))).inner_margin(egui::Margin::same(20));
        let mut quit = false;
        let modal = egui::Modal::new(egui::Id::unique("quit-sheet")).frame(frame).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new("Quit ByteHive?").size(16.0).color(TEXT));
            let who = if n == 1 { "1 agent is".to_string() } else { format!("{n} agents are") };
            ui.label(RichText::new(format!("{who} still running. Quitting stops them; their sessions can be resumed later from Sessions.")).size(12.5).color(INK3));
            ui.add_space(12.0);
            let mut keep = false;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let b = egui::Button::new(RichText::new("Quit and stop them").strong().color(TEXT)).fill(DEL.gamma_multiply(0.5)).corner_radius(999.0).min_size(egui::vec2(0.0, 32.0));
                quit = ui.add(b).clicked();
                keep = ui.add(egui::Button::new(RichText::new("Keep working").size(12.5).color(INK3)).frame(false)).clicked();
            });
            keep
        });
        if modal.inner || modal.should_close() {
            self.extras.system.asking_quit = false;
        }
        if quit {
            self.extras.system.asking_quit = false;
            self.extras.system.quit_ok = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_only_while_agents_live() {
        assert!(should_ask(2, false));
        assert!(!should_ask(0, false));
        assert!(!should_ask(3, true), "you already said quit");
    }

    #[test]
    fn counts_only_new_needs() {
        let a: HashSet<String> = ["p1".to_string()].into();
        let b: HashSet<String> = ["p1".to_string(), "p2".to_string()].into();
        assert_eq!(newly_needing(&a, &b), 1);
        assert_eq!(newly_needing(&b, &a), 0);
    }

    #[test]
    fn a_shell_never_holds_the_close() {
        let mut app = ShellApp::new().unwrap();
        assert_eq!(app.live_agents(), 0);
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |ui| app.poll_system(ui.ctx())).drop_without_applying_deltas();
        assert!(!app.extras.system.asking_quit);
    }
}
