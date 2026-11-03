//! An agent's actions, the same from its row's ⋯ and a right-click on the
//! row (as the old app's agent menu): open it, rename it, its changes and
//! history, its project's memory, its branch, hand it over, restart it or
//! start it on a new conversation, stop it, and remove it (asked first).
//! Also what each one does to the app.

use eframe::egui::{self, Color32, RichText, Stroke};

use crate::app::ShellApp;
use crate::term_pane::Pane;
use crate::theme::{DEL, INK3, R_XL, SHEET, TEXT};

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
    Fresh(String),
    Stop(String),
    /// Remove, once you said yes.
    Remove(String),
    /// Take a merged agent off the list: its work is in.
    Clear(String),
}

/// One line of the menu, with its key when it has one; true when picked.
fn item(ui: &mut egui::Ui, label: &str, key: Option<&str>, enabled: bool) -> bool {
    let mut b = egui::Button::new(RichText::new(label).color(TEXT)).frame_when_inactive(false);
    if let Some(k) = key {
        b = b.shortcut_text(RichText::new(k).color(INK3).size(12.0));
    }
    ui.add_enabled(enabled, b).clicked()
}

/// A menu's lines close together, as the list's menus all are.
pub fn tight(ui: &mut egui::Ui) {
    ui.set_min_width(220.0);
    let sp = ui.spacing_mut();
    sp.item_spacing.y = 1.0;
    sp.interact_size.y = 28.0;
    sp.button_padding = egui::vec2(10.0, 5.0);
}

/// The menu's lines for `p`; what was picked. Alt+R and Alt+H act on the
/// agent on stage, so its menu alone shows them.
pub fn agent_menu(ui: &mut egui::Ui, p: &Pane, on_stage: bool) -> Option<RowAct> {
    tight(ui);
    let id = || p.id.clone();
    let key = |k: &'static str| on_stage.then_some(k);
    let stopped = p.exit_code.is_some();
    let mut act = None;
    if p.merged && item(ui, "Clear from list", None, true) {
        act = Some(RowAct::Clear(id()));
    }
    if item(ui, "Open", None, true) {
        act = Some(RowAct::Pick(id()));
    }
    if item(ui, "Rename\u{2026}", None, true) {
        act = Some(RowAct::Rename(id()));
    }
    ui.separator();
    if item(ui, "Changes", key("Alt+R"), true) {
        act = Some(RowAct::Changes(id()));
    }
    if item(ui, "History", key("Alt+H"), true) {
        act = Some(RowAct::History(id()));
    }
    if item(ui, "Project memory", None, true) {
        act = Some(RowAct::Memory(id()));
    }
    if let Some(w) = &p.worktree {
        if item(ui, "Copy branch name", None, true) {
            act = Some(RowAct::CopyBranch(w.branch.clone()));
        }
    }
    // A plain shell has no task to hand over, nor a conversation.
    if p.cli.is_some() && item(ui, "Hand over to another CLI\u{2026}", None, true) {
        act = Some(RowAct::Handoff(id()));
    }
    ui.separator();
    if item(ui, if stopped { "Resume" } else { "Restart" }, None, true) {
        act = Some(RowAct::Restart(id()));
    }
    if p.cli.is_some() && item(ui, "New conversation", None, true) {
        act = Some(RowAct::Fresh(id()));
    }
    if item(ui, "Stop", None, !stopped) {
        act = Some(RowAct::Stop(id()));
    }
    if ui.add(egui::Button::new(RichText::new("Remove agent\u{2026}").color(DEL)).frame_when_inactive(false)).clicked() {
        act = Some(RowAct::Remove(id()));
    }
    act
}

impl ShellApp {
    /// Put an agent on the stage, the list showing it picked.
    pub(crate) fn put_on_stage(&mut self, id: &str) {
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
            RowAct::Fresh(id) => {
                if let Some(i) = self.panes.iter().position(|p| p.id == id) {
                    self.new_conversation(i);
                }
            }
            RowAct::Stop(id) => {
                drop(agent_core::lock(&self.registry).take(&id));
                if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
                    p.exit_code.get_or_insert(0);
                }
            }
            RowAct::Remove(id) => self.extras.remove_ask = Some(id),
            RowAct::Clear(id) => self.remove_panes(&[id]),
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

    /// "Remove Ana?", while a Remove waits for your OK.
    pub(crate) fn remove_sheet(&mut self, ctx: &egui::Context) {
        let Some(id) = self.extras.remove_ask.clone() else { return };
        let Some(p) = self.panes.iter().find(|p| p.id == id) else {
            self.extras.remove_ask = None;
            return;
        };
        let (name, running) = (p.name.clone(), p.exit_code.is_none());
        let frame = egui::Frame::new().fill(SHEET).corner_radius(R_XL).stroke(Stroke::new(1.0, Color32::from_white_alpha(20))).inner_margin(egui::Margin::same(20));
        let mut yes = false;
        let modal = egui::Modal::new(egui::Id::unique("remove-agent")).frame(frame).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(format!("Remove {name}?")).size(16.0).color(TEXT));
            let why = if running { "It stops and its terminal closes." } else { "Its terminal closes." };
            ui.label(RichText::new(format!("{why} Its branch and the changes on it stay in git, and History can carry its conversation on.")).size(12.5).color(INK3));
            ui.add_space(14.0);
            let mut no = ui.input(|i| i.key_pressed(egui::Key::Escape));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let b = egui::Button::new(RichText::new("Remove").strong().color(Color32::WHITE)).fill(DEL.gamma_multiply(0.85)).corner_radius(999.0).min_size(egui::vec2(0.0, 32.0));
                let r = ui.add(b);
                r.request_focus();
                yes = r.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                no |= ui.add(egui::Button::new(RichText::new("Cancel").size(12.5).color(INK3)).frame(false)).clicked();
            });
            no
        });
        if yes {
            self.remove_panes(&[id]);
        }
        if yes || modal.inner || modal.should_close() {
            self.extras.remove_ask = None;
        }
    }
}
