//! The top bar: shell + New task, the project chips, launch progress and
//! notices, and the density zoom readout. Also what a project chip does to
//! the app (filter, open, close), kept here so app.rs stays a dispatcher.

use std::collections::HashSet;

use agent_core::launch::launch_spec;
use eframe::egui;

use crate::app::ShellApp;
use crate::projects::pane_home;
use crate::projects_ui::{show_chips, ProjectAction};
use crate::queue_ui::is_attention;
use crate::states::pane_status;
use crate::theme::{DEL, DIM, PRIMARY, PRIMARY_INK};

impl ShellApp {
    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui) {
        let homes: Vec<(String, bool)> = self
            .panes
            .iter()
            .map(|p| (pane_home(p), is_attention(pane_status(p, crate::app::now_ms(), None).state)))
            .collect();
        let mut action = None;
        ui.horizontal(|ui| {
            if ui.button("+ Shell").clicked() {
                let mut spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
                spec.cwd = self.projects.filter.clone();
                let _ = self.spawn_pane("shell", spec, None, None);
            }
            let new = egui::Button::new(egui::RichText::new("New task").strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0);
            if ui.add(new).on_hover_text("Start agents on a task (Ctrl+N). Spawn a bare CLI from Ctrl+K.").clicked() {
                self.run_command("new");
            }
            ui.add_space(10.0);
            action = show_chips(ui, &self.projects, &homes);
            if !self.launching.is_empty() {
                ui.label(egui::RichText::new(format!("Making worktrees for {}\u{2026}", self.launching.join(", "))).small().color(DIM));
            }
            if let Some(n) = self.notice.clone() {
                if ui.add(egui::Label::new(egui::RichText::new(n).small().color(DEL)).sense(egui::Sense::click())).on_hover_text("Click to dismiss").clicked() {
                    self.notice = None;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(egui::RichText::new("Settings").small().color(DIM)).frame(false)).on_hover_text("Settings (Ctrl+,)").clicked() {
                    self.run_command("settings");
                }
                ui.label(egui::RichText::new(format!("{}%", (self.zoom * 100.0) as u32)).small().color(DIM))
                    .on_hover_text("Density zoom: Ctrl+= bigger, Ctrl+- smaller, Ctrl+0 reset");
            });
        });
        if let Some(a) = action {
            self.apply_project(a);
        }
    }

    pub(crate) fn apply_project(&mut self, action: ProjectAction) {
        self.stash_stage();
        match action {
            ProjectAction::Filter(f) => self.projects.filter = f,
            ProjectAction::Open(dir) => {
                let d = self.projects.open(&dir);
                self.projects.filter = Some(d);
            }
            ProjectAction::Close(dir) => self.projects.close(&dir),
        }
        // Sessions and new tasks follow the project on show.
        if let Some(f) = self.projects.filter.clone() {
            self.sessions.set_dir(&f);
        }
        self.restore_stage();
        self.save_projects();
    }

    pub(crate) fn save_projects(&self) {
        if let Some(path) = &self.projects_path {
            self.projects.save(path);
        }
    }

    /// Panes the project filter hides from the grid and the Queue.
    pub(crate) fn hidden_panes(&self) -> HashSet<String> {
        self.panes.iter().filter(|p| !self.projects.shows(&pane_home(p))).map(|p| p.id.clone()).collect()
    }
}
