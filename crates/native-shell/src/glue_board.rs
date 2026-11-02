//! Wiring for the Board view: agent cards built from the panes on show, the
//! plan board of the project on show, and what a card click does (back to
//! the terminals with that agent on stage).

use std::collections::HashSet;
use std::path::PathBuf;

use eframe::egui;

use crate::app::{now_ms, ShellApp};
use crate::board_ui::AgentCard;
use crate::grid::GridOut;
use crate::plan_board::board_path;
use crate::projects::{pane_home, project_name};
use crate::states::{pane_cwd, pane_status};

impl ShellApp {
    fn agent_cards(&self, hidden: &HashSet<String>) -> Vec<AgentCard> {
        let now = now_ms();
        self.panes
            .iter()
            .filter(|p| !hidden.contains(&p.id))
            .map(|p| {
                let changed = self.git_changed.get(&pane_cwd(p)).map(|f| f.len() as u32);
                let state = if p.merged { supervise::task_state::TaskState::Merged } else { pane_status(p, now, changed).state };
                let home = pane_home(p);
                let project = self.projects.owner(&home).map(project_name).unwrap_or_else(|| project_name(&home));
                let where_ = match &p.worktree {
                    Some(w) => format!("{project} / {}", w.branch),
                    None => project,
                };
                let title = p.task.clone().unwrap_or_else(|| crate::inbox::state_label(state).to_string());
                let cost = p.usage.as_ref().and_then(|u| u.usd).map(supervise::pricing::money).unwrap_or_default();
                AgentCard { pane_id: p.id.clone(), name: p.name.clone(), cli: p.cli.map_or("Shell", |c| c.name()).to_string(), title, where_, state, cost }
            })
            .collect()
    }

    /// The plan board of the project on show (or the only open one).
    fn plan_path(&self) -> Option<(PathBuf, String)> {
        let dir = self.projects.filter.clone().or_else(|| self.projects.dirs.first().cloned())?;
        Some((board_path(&dir), project_name(&dir)))
    }

    /// The Board in place of the terminals.
    pub(crate) fn board_central(&mut self, ui: &mut egui::Ui, hidden: &HashSet<String>) -> GridOut {
        let cards = self.agent_cards(hidden);
        let plan = self.plan_path();
        let mut picked = None;
        // On the same floating panel as the stage, not bare on the ground.
        let frame = egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: crate::style::GUTTER as i8, top: 0, bottom: 0 });
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            crate::inbox::panel(ui, |ui| {
                let (path, name) = match &plan {
                    Some((p, n)) => (Some(p.as_path()), n.as_str()),
                    None => (None, ""),
                };
                picked = self.board.show(ui, &cards, path, name);
            });
        });
        if let Some((id, task)) = self.board.give.take() {
            self.tell(id, task);
        }
        if let Some(id) = picked {
            // Back to the terminals with that agent up front.
            self.board.open = false;
            self.stage = Some(id.clone());
            self.focus_pane = Some(id);
        }
        GridOut::default()
    }
}
