//! What the floating sheets (Settings, New task) and the start screen do to
//! the app, and the stage remembered per project. Kept out of app.rs, which
//! stays a dispatcher under the length gate.

use eframe::egui;

use crate::app::ShellApp;
use crate::grid::GridOut;
use crate::newtask_ui::SheetOut;
use crate::start_ui::{show_start, StartAction};

/// The key a project filter's stage is remembered under ("*" for all).
fn stage_key(filter: &Option<String>) -> String {
    filter.clone().unwrap_or_else(|| "*".to_string())
}

impl ShellApp {
    pub(crate) fn save_prefs(&self) {
        if let Some(path) = &self.prefs_path {
            self.prefs.save(path);
        }
    }

    /// Settings, then New task, floating over everything.
    pub(crate) fn sheets(&mut self, ctx: &egui::Context) {
        let store = self.prefs_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "not saved (no home folder)".to_string());
        if self.settings.show(ctx, &mut self.prefs, &store) {
            self.save_prefs();
        }
        let Some(out) = self.new_task.show(ctx, &self.projects.dirs, &self.prefs.crews) else { return };
        match out {
            SheetOut::SaveCrew(crew) => {
                self.notice = Some(format!("Saved crew {}.", crew.name));
                self.prefs.save_crew(crew);
                self.save_prefs();
            }
            SheetOut::Start(form) => match self.start_task(&form) {
                Ok(()) => {
                    self.projects.open(&form.dir);
                    self.save_projects();
                    self.prefs.last_cli = form.cli;
                    self.save_prefs();
                    if self.new_task.more {
                        // Keep typing in the sheet, not in the new pane.
                        self.focus_pane = None;
                    }
                    self.new_task.started();
                }
                Err(e) => self.new_task.error = Some(e),
            },
        }
    }

    /// The grid, or the start screen while no project is open.
    pub(crate) fn central(&mut self, ui: &mut egui::Ui, hidden: &std::collections::HashSet<String>) -> GridOut {
        if !self.projects.dirs.is_empty() {
            return crate::grid::show_grid(self, ui, hidden);
        }
        let mut act = None;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(crate::theme::BG)).show(ui, |ui| {
            act = show_start(ui, &self.projects.recent, &self.prefs.crews);
        });
        match act {
            Some(StartAction::Open(dir)) => self.apply_project(crate::projects_ui::ProjectAction::Open(dir)),
            Some(StartAction::Crew(dir, crew)) => {
                self.apply_project(crate::projects_ui::ProjectAction::Open(dir.clone()));
                self.new_task.open_with(&dir, Some(crew.count), &self.prefs);
                let f = &mut self.new_task.form;
                f.cli = crew.cli;
                f.worktree = crew.worktree;
                f.perm = crew.perm;
                f.flow = crew.flow;
            }
            None => {}
        }
        GridOut::default()
    }

    /// Before the project filter changes: remember what was on stage.
    pub(crate) fn stash_stage(&mut self) {
        let key = stage_key(&self.projects.filter);
        match self.stage.clone() {
            Some(id) => {
                self.stages.insert(key, id);
            }
            None => {
                self.stages.remove(&key);
            }
        }
    }

    /// After it changed: put back that project's stage, if its pane lives.
    pub(crate) fn restore_stage(&mut self) {
        let key = stage_key(&self.projects.filter);
        let hidden = self.hidden_panes();
        self.stage = self.stages.get(&key).filter(|id| self.panes.iter().any(|p| &p.id == *id) && !hidden.contains(*id)).cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_project_keeps_its_own_stage() {
        let mut app = ShellApp::new().unwrap();
        app.projects = Default::default();
        app.projects.open(r"D:\maestro");
        let id = app.panes[0].id.clone();
        app.stage = Some(id.clone());
        // Switch to a project that hides the shell: no stage there.
        app.stash_stage();
        app.projects.open(r"Z:\elsewhere");
        app.projects.filter = Some(r"Z:\elsewhere".to_string());
        app.restore_stage();
        assert_eq!(app.stage, None);
        // Back to all projects: the shell is on stage again.
        app.stash_stage();
        app.projects.filter = None;
        app.restore_stage();
        assert_eq!(app.stage, Some(id));
    }
}
