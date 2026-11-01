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
        self.settings.update_status = self.extras.updates.status.clone();
        if std::mem::take(&mut self.settings.want_update_check) {
            self.extras.updates.check_now(ctx);
        }
        if self.settings.show(ctx, &mut self.prefs, &store, &mut self.extras.remote.form, &mut self.extras.browser.form) {
            self.save_prefs();
        }
        self.new_task.room = self.extras.plan.room_note(self.new_task.form.cli, crate::app::now_ms() as i64);
        let Some(out) = self.new_task.show(ctx, &self.projects.dirs, &self.prefs.crews) else { return };
        match out {
            SheetOut::Schedule(s) => {
                self.notice = Some(format!("Scheduled: {}.", s.when()));
                self.prefs.schedules.push(s);
                self.save_prefs();
                self.new_task.started();
            }
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

    /// Start every scheduled task that is due, and wake up for the next one.
    pub(crate) fn poll_schedules(&mut self, ctx: &egui::Context) {
        if !self.prefs.schedules.iter().any(|s| s.enabled) {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(20));
        let now = chrono::Local::now();
        let due: Vec<usize> = (0..self.prefs.schedules.len()).filter(|&i| self.prefs.schedules[i].is_due(now)).collect();
        if due.is_empty() {
            return;
        }
        for i in due {
            let task = self.prefs.schedules[i].task.clone();
            self.prefs.schedules[i].fired(now);
            let title: String = task.prompt.lines().next().unwrap_or("").chars().take(60).collect();
            self.notice = Some(match self.start_task(&task) {
                Ok(()) => format!("Started the scheduled task: {title}"),
                Err(e) => format!("The scheduled task {title} could not start: {e}"),
            });
            self.projects.open(&task.dir);
        }
        self.save_projects();
        self.save_prefs();
    }

    /// The grid, or the start screen while no project is open.
    pub(crate) fn central(&mut self, ui: &mut egui::Ui, hidden: &std::collections::HashSet<String>, order: &[String]) -> GridOut {
        if !self.projects.dirs.is_empty() || !self.panes.is_empty() {
            if self.board.open {
                return self.board_central(ui, hidden);
            }
            if self.extras.view == crate::dock::View::Queue {
                return self.stage_central(ui, order);
            }
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
    fn a_due_schedule_fires_once_and_says_why_it_failed() {
        let mut app = ShellApp::new().unwrap();
        app.prefs_path = None; // never touch the real settings file
        app.projects_path = None;
        let mut task = crate::newtask::NewTask::new(r"Z:\no-such-repo".to_string());
        task.prompt = "Nightly checks".to_string();
        let now = chrono::Local::now();
        let time = format!("{:02}:{:02}", now.format("%H"), now.format("%M"));
        app.prefs.schedules = vec![crate::schedule::Schedule { id: "s".into(), task, time, daily: false, enabled: true, last_fired: 0 }];
        let ctx = egui::Context::default();
        app.poll_schedules(&ctx);
        let note = app.notice.clone().unwrap_or_default();
        assert!(note.contains("could not start") && note.contains("git repo"), "{note}");
        assert!(!app.prefs.schedules[0].enabled, "a once schedule is spent");
        let before = app.notice.take();
        app.poll_schedules(&ctx);
        assert!(app.notice.is_none() && before.is_some(), "it does not fire twice");
    }

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
