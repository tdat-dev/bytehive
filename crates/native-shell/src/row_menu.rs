//! An agent's actions, the same from its row's ⋯ and a right-click on the
//! row (as the old app's agent menu): open it, rename it, its changes and
//! history, its project's memory, its branch, hand it over, and restart,
//! stop or remove it. Also what each one does to the app.

use eframe::egui::{self, RichText};

use crate::app::ShellApp;
use crate::term_pane::Pane;
use crate::theme::{DEL, TEXT};

/// What a row asks of the app, by pane id.
pub enum RowAct {
    Pick(String),
    Rename(String),
    Changes(String),
    History(String),
    Memory(String),
    CopyBranch(String),
    Handoff(String),
    Restart(String),
    Stop(String),
    Remove(String),
}

/// One line of the menu; true when picked.
fn item(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(RichText::new(label).color(TEXT)).frame_when_inactive(false)).clicked()
}

/// A menu's lines close together, as the list's menus all are.
pub fn tight(ui: &mut egui::Ui) {
    ui.set_min_width(210.0);
    let sp = ui.spacing_mut();
    sp.item_spacing.y = 1.0;
    sp.interact_size.y = 28.0;
    sp.button_padding = egui::vec2(10.0, 5.0);
}

/// The menu's lines for `p`; what was picked.
pub fn agent_menu(ui: &mut egui::Ui, p: &Pane) -> Option<RowAct> {
    tight(ui);
    let id = || p.id.clone();
    let stopped = p.exit_code.is_some();
    let mut act = None;
    if item(ui, "Open", true) {
        act = Some(RowAct::Pick(id()));
    }
    if item(ui, "Rename\u{2026}", true) {
        act = Some(RowAct::Rename(id()));
    }
    ui.separator();
    if item(ui, "Changes", true) {
        act = Some(RowAct::Changes(id()));
    }
    if item(ui, "History", true) {
        act = Some(RowAct::History(id()));
    }
    if item(ui, "Project memory", true) {
        act = Some(RowAct::Memory(id()));
    }
    if let Some(w) = &p.worktree {
        if item(ui, "Copy branch name", true) {
            act = Some(RowAct::CopyBranch(w.branch.clone()));
        }
    }
    // A plain shell has no task to hand over.
    if p.cli.is_some() && item(ui, "Hand over to another CLI\u{2026}", true) {
        act = Some(RowAct::Handoff(id()));
    }
    ui.separator();
    if item(ui, if stopped { "Resume" } else { "Restart" }, true) {
        act = Some(RowAct::Restart(id()));
    }
    if item(ui, "Stop", !stopped) {
        act = Some(RowAct::Stop(id()));
    }
    if ui.add(egui::Button::new(RichText::new("Remove agent").color(DEL)).frame_when_inactive(false)).on_hover_text("It stops and its terminal closes. Its branch and changes stay in git.").clicked() {
        act = Some(RowAct::Remove(id()));
    }
    act
}

impl ShellApp {
    /// Put an agent on the stage, the list showing it picked.
    fn put_on_stage(&mut self, id: &str) {
        self.stage = Some(id.to_string());
        self.focus_pane = Some(id.to_string());
        self.board.open = false;
        self.extras.home = false;
    }

    /// Do what a row asked.
    pub(crate) fn row_act(&mut self, act: RowAct, ctx: &egui::Context) {
        // The stage was drawn before the list asked: show the change now.
        ctx.request_repaint();
        match act {
            RowAct::Pick(id) => self.put_on_stage(&id),
            RowAct::Rename(id) => {
                self.put_on_stage(&id);
                let now = self.panes.iter().find(|p| p.id == id).and_then(|p| p.task.clone()).unwrap_or_default();
                self.extras.stage.renaming = Some(now);
            }
            RowAct::Changes(id) => {
                self.put_on_stage(&id);
                self.extras.stage.history = false;
                self.run_command("changes");
            }
            RowAct::History(id) => {
                self.put_on_stage(&id);
                self.extras.stage.history = true;
                self.changes.open = false;
            }
            RowAct::Memory(id) => match self.group_of(&id) {
                Some(dir) => self.extras.memory.open_on(&dir),
                None => self.notice = Some("This agent's folder is no project: memory belongs to a project.".into()),
            },
            RowAct::CopyBranch(branch) => ctx.copy_text(branch),
            RowAct::Handoff(id) => {
                self.put_on_stage(&id);
                self.open_handoff();
            }
            RowAct::Restart(id) => {
                if let Some(i) = self.panes.iter().position(|p| p.id == id) {
                    self.restart_pane(i);
                }
            }
            RowAct::Stop(id) => {
                drop(agent_core::lock(&self.registry).take(&id));
                if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
                    p.exit_code.get_or_insert(0);
                }
            }
            RowAct::Remove(id) => self.remove_panes(&[id]),
        }
    }

    /// Stop these panes and take them out of the list.
    pub(crate) fn remove_panes(&mut self, ids: &[String]) {
        for id in ids {
            // Torn down outside the lock: a dashboard thread may want it.
            let gone = agent_core::lock(&self.registry).take(id);
            drop(gone);
        }
        self.panes.retain(|p| !ids.contains(&p.id));
    }
}
