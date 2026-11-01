//! Hand an agent's task to an agent of another CLI: when one stops at its
//! plan's usage limit (or you'd rather another CLI carried on), a new agent
//! starts in the same folder (and worktree) with a brief written from the
//! first one's conversation: what it was asked, its plan, the files it
//! changed, and where it got to. The brief is built off the UI thread
//! (transcript + git), then the new agent boots and is typed the brief.

use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui::{self, Color32, RichText, Stroke};
use supervise::digest::{handoff_brief, HandoffFrom};

use crate::app::{now_ms, ShellApp};
use crate::deliver::Delivery;
use crate::history_ui::{parse_history, read_history};
use crate::newtask::persona;
use crate::sessions_ui::Cli;
use crate::states::pane_cwd;
use crate::term_pane::WorktreeInfo;
use crate::theme::{INK3, PILL, PRIMARY, PRIMARY_INK, R_XL, SHEET, TEXT};

/// Why, when the agent hit its limit.
pub const WHY_LIMIT: &str = "it hit its plan's usage limit";
/// Why, when you chose to.
pub const WHY_CHOSEN: &str = "the user would rather another CLI carried on";

/// A hand-off whose brief is being written.
pub struct HandoffJob {
    pub from_name: String,
    pub name: String,
    pub to: Cli,
    pub dir: String,
    pub worktree: Option<WorktreeInfo>,
    pub task: Option<String>,
}

pub struct Handoffs {
    tx: Sender<(HandoffJob, String)>,
    rx: Receiver<(HandoffJob, String)>,
    /// Names being prepared, so a second hand-off picks another persona.
    pub pending: Vec<String>,
    /// Panes already handed over for the limit they hit (pane id, reset).
    pub handed: Vec<(String, i64)>,
    pub sheet: HandoffSheet,
}

impl Default for Handoffs {
    fn default() -> Self {
        let (tx, rx) = channel();
        Handoffs { tx, rx, pending: Vec::new(), handed: Vec::new(), sheet: HandoffSheet::default() }
    }
}

/// "Hand Ana's task to [Codex]".
#[derive(Default)]
pub struct HandoffSheet {
    pub open: bool,
    pub from: Option<String>,
    pub to: Option<Cli>,
}

impl ShellApp {
    /// Start a hand-off of pane `from_id` to a new `to` agent.
    pub(crate) fn hand_off(&mut self, from_id: &str, to: Cli, why: &str) -> Result<String, String> {
        let p = self.panes.iter().find(|p| p.id == from_id).ok_or("That agent is gone.")?;
        let cli = p.cli.ok_or("Only an agent can hand its task over, not a shell.")?;
        let mut taken: Vec<String> = self.panes.iter().map(|p| p.name.clone()).collect();
        taken.extend(self.extras.handoffs.pending.iter().cloned());
        let name = persona(&taken);
        let job = HandoffJob { from_name: p.name.clone(), name: name.clone(), to, dir: pane_cwd(p), worktree: p.worktree.clone(), task: p.task.clone() };
        let from = HandoffFrom { name: p.name.clone(), cli: cli.name().to_string(), why: why.to_string(), title: p.task.clone(), first_asks: Vec::new(), git_files: Vec::new() };
        let session = p.session_id.clone();
        let tx = self.extras.handoffs.tx.clone();
        self.extras.handoffs.pending.push(name.clone());
        std::thread::spawn(move || {
            let brief = write_brief(cli, &job.dir, session.as_deref(), from);
            let _ = tx.send((job, brief));
        });
        Ok(name)
    }

    /// Boot the agents whose brief is written, and type it in.
    pub(crate) fn poll_handoffs(&mut self) {
        while let Ok((job, brief)) = self.extras.handoffs.rx.try_recv() {
            self.extras.handoffs.pending.retain(|n| n != &job.name);
            let extra = self.prefs.perm.args(job.to);
            match self.boot_agent(job.to, &job.dir, job.name.clone(), extra, job.worktree) {
                Ok(id) => {
                    if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
                        p.task = job.task;
                    }
                    self.deliveries.push(Delivery::new(id.clone(), brief, now_ms(), self.prefs.job_delay_ms()));
                    self.notice = Some(format!("{} ({}) took over from {}.", job.name, job.to.name(), job.from_name));
                    self.focus_pane = Some(id);
                }
                Err(e) => self.notice = Some(format!("{} could not take over from {}: {e}", job.name, job.from_name)),
            }
        }
    }

    /// With a hand-over CLI set, an agent stopped by its limit hands its
    /// task over once per limit.
    pub(crate) fn handover_on_limit(&mut self) {
        let Some(to) = self.prefs.handover_to else { return };
        let hits: Vec<(String, i64)> = self
            .panes
            .iter()
            .filter(|p| p.exit_code.is_none() && p.cli.is_some_and(|c| c != to))
            .filter_map(|p| p.limit.as_ref().map(|l| (p.id.clone(), l.resets_at.map_or(0, |t| t.timestamp()))))
            .collect();
        for hit in hits {
            if self.extras.handoffs.handed.contains(&hit) {
                continue;
            }
            self.extras.handoffs.handed.push(hit.clone());
            if let Err(e) = self.hand_off(&hit.0, to, WHY_LIMIT) {
                self.notice = Some(e);
            }
        }
    }

    pub(crate) fn open_handoff(&mut self) {
        let from = self.stage.clone().or_else(|| self.focus_pane.clone()).filter(|id| self.panes.iter().any(|p| &p.id == id && p.cli.is_some()));
        let from = from.or_else(|| self.panes.iter().find(|p| p.cli.is_some()).map(|p| p.id.clone()));
        match from {
            None => self.notice = Some("No agent to hand over: start one first.".into()),
            Some(id) => {
                let own = self.panes.iter().find(|p| p.id == id).and_then(|p| p.cli);
                let to = Cli::all().into_iter().find(|c| Some(*c) != own);
                self.extras.handoffs.sheet = HandoffSheet { open: true, from: Some(id), to };
            }
        }
    }

    pub(crate) fn handoff_sheet(&mut self, ctx: &egui::Context) {
        let sheet = &mut self.extras.handoffs.sheet;
        if !sheet.open {
            return;
        }
        let agents: Vec<(String, String, Cli)> = self.panes.iter().filter_map(|p| p.cli.map(|c| (p.id.clone(), p.name.clone(), c))).collect();
        let frame = egui::Frame::new().fill(SHEET).corner_radius(R_XL).stroke(Stroke::new(1.0, Color32::from_white_alpha(20))).inner_margin(egui::Margin::same(20));
        let mut go = false;
        let modal = egui::Modal::new(egui::Id::unique("handoff-sheet")).frame(frame).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(RichText::new("Hand over a task").size(16.0).color(TEXT));
            ui.label(RichText::new("A new agent starts in the same folder with a brief of the conversation: what was asked, the plan, the files changed and where it got to.").size(12.5).color(INK3));
            ui.add_space(10.0);
            let from_name = agents.iter().find(|a| Some(&a.0) == sheet.from.as_ref()).map(|a| format!("{} ({})", a.1, a.2.name())).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(RichText::new("From").size(12.5).color(INK3));
                egui::ComboBox::from_id_salt("ho-from").selected_text(from_name).show_ui(ui, |ui| {
                    for (id, name, cli) in &agents {
                        ui.selectable_value(&mut sheet.from, Some(id.clone()), format!("{name} ({})", cli.name()));
                    }
                });
            });
            let own = agents.iter().find(|a| Some(&a.0) == sheet.from.as_ref()).map(|a| a.2);
            ui.horizontal(|ui| {
                ui.label(RichText::new("To").size(12.5).color(INK3));
                for c in Cli::all().into_iter().filter(|c| Some(*c) != own) {
                    let on = sheet.to == Some(c);
                    let b = egui::Button::new(RichText::new(c.name()).size(12.5).color(if on { PRIMARY_INK } else { TEXT })).fill(if on { PRIMARY } else { PILL }).corner_radius(999.0);
                    if ui.add(b).clicked() {
                        sheet.to = Some(c);
                    }
                }
            });
            ui.add_space(12.0);
            let mut close = false;
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ok = sheet.from.is_some() && sheet.to.is_some_and(|t| Some(t) != own);
                    let b = egui::Button::new(RichText::new("Hand over").strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0).min_size(egui::vec2(0.0, 32.0));
                    if ui.add_enabled(ok, b).clicked() {
                        go = true;
                    }
                    if ui.add(egui::Button::new(RichText::new("Cancel").size(12.5).color(INK3)).frame(false)).clicked() {
                        close = true;
                    }
                });
            });
            close
        });
        if modal.inner || modal.should_close() {
            sheet.open = false;
        }
        if go {
            sheet.open = false;
            let (from, to) = (sheet.from.clone().unwrap_or_default(), sheet.to.unwrap_or(Cli::Claude));
            if let Err(e) = self.hand_off(&from, to, WHY_CHOSEN) {
                self.notice = Some(e);
            }
        }
    }
}

/// The brief, from the agent's transcript and what git says changed.
fn write_brief(cli: Cli, dir: &str, session: Option<&str>, mut from: HandoffFrom) -> String {
    from.git_files = agent_core::git::changed_files(std::path::Path::new(dir));
    let chunk = read_history(cli, dir, session);
    if chunk.text.trim().is_empty() {
        return handoff_brief(&from, None);
    }
    let (items, meta) = parse_history(cli, &chunk.text);
    handoff_brief(&from, Some((&items, &meta)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_cannot_hand_over_and_an_agent_gets_a_new_persona() {
        let mut app = ShellApp::new().unwrap();
        let id = app.panes[0].id.clone();
        assert!(app.hand_off(&id, Cli::Codex, WHY_CHOSEN).unwrap_err().contains("not a shell"));
        app.panes[0].cli = Some(Cli::Claude);
        app.panes[0].name = "Ana".into();
        let name = app.hand_off(&id, Cli::Codex, WHY_CHOSEN).unwrap();
        assert_eq!(name, "Bob");
        assert_eq!(app.extras.handoffs.pending, vec!["Bob".to_string()]);
    }

    #[test]
    fn the_brief_without_a_conversation_says_to_look_at_the_folder() {
        let from = HandoffFrom { name: "Ana".into(), cli: "Claude".into(), why: WHY_LIMIT.into(), title: Some("Fix login".into()), first_asks: vec![], git_files: vec![] };
        let b = write_brief(Cli::Claude, r"Z:\nowhere", Some("00000000-0000-0000-0000-000000000000"), from);
        assert!(b.contains("from Ana (Claude)") && b.contains("Its task: Fix login") && b.contains("look at the folder first"), "{b}");
    }

    #[test]
    fn a_limit_hands_over_once() {
        let mut app = ShellApp::new().unwrap();
        app.prefs.handover_to = Some(Cli::Codex);
        app.panes[0].cli = Some(Cli::Claude);
        app.panes[0].limit = Some(supervise::limits::LimitHit { what: "limit".into(), resets_at: None });
        app.handover_on_limit();
        app.handover_on_limit();
        assert_eq!(app.extras.handoffs.pending.len(), 1);
    }
}
