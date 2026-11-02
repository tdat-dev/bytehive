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
                    match &form.custom {
                        Some(cmd) => self.prefs.remember_custom(cmd),
                        None => self.prefs.last_cli = form.cli,
                    }
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
        if !self.extras.home && (!self.projects.dirs.is_empty() || !self.panes.is_empty()) {
            if self.board.open {
                return self.board_central(ui, hidden);
            }
            if self.extras.view == crate::dock::View::Queue {
                return self.stage_central(ui, order);
            }
            return crate::grid::show_grid(self, ui, hidden);
        }
        self.home_central(ui);
        GridOut::default()
    }

    /// The start screen: projects, who waits on you, the composer.
    fn home_central(&mut self, ui: &mut egui::Ui) {
        use crate::start_ui::{HomeView, ProjectCard};
        use supervise::task_state::TaskState;
        let now = crate::app::now_ms();
        let mut dirs: Vec<String> = self.projects.dirs.clone();
        for d in &self.projects.recent {
            if !dirs.iter().any(|x| x.eq_ignore_ascii_case(d)) {
                dirs.push(d.clone());
            }
        }
        let states: Vec<(String, TaskState)> = self.panes.iter().filter(|p| p.cli.is_some()).map(|p| (crate::projects::pane_home(p), crate::states::pane_status(p, now, None).state)).collect();
        let projects: Vec<ProjectCard> = dirs
            .into_iter()
            .map(|dir| {
                let mine: Vec<&TaskState> = states.iter().filter(|(h, _)| self.projects.owner(h).is_some_and(|o| o.eq_ignore_ascii_case(&dir))).map(|(_, s)| s).collect();
                let needs = mine.iter().filter(|s| matches!(s, TaskState::Needs | TaskState::Blocked)).count();
                ProjectCard { agents: mine.len(), needs, dir }
            })
            .collect();
        let waiting: Vec<crate::start_ui::Waiting> = self
            .panes
            .iter()
            .filter_map(|p| {
                let st = crate::states::pane_status(p, now, None);
                (p.cli.is_some() && st.state == TaskState::Needs).then(|| {
                    let home = crate::projects::pane_home(p);
                    let proj = crate::projects::project_name(self.projects.owner(&home).unwrap_or(&home));
                    let what = st.ask.map(|a| a.prompt).unwrap_or_else(|| "Needs you".into());
                    (p.id.clone(), p.name.clone(), proj, what)
                })
            })
            .collect();
        let count = |s: TaskState| states.iter().filter(|(_, x)| *x == s).count();
        let installed = self.extras.clis.get_or_insert_with(crate::newtask_ui::installed_clis).clone();
        let room = self.extras.plan.room_note(self.extras.start.cli, now as i64);
        let hour = chrono::Timelike::hour(&chrono::Local::now());
        let recent = self.prefs.custom_cmds.clone();
        let view = HomeView { projects: &projects, waiting: &waiting, working: count(TaskState::Working), review: count(TaskState::Review), installed: &installed, recent: &recent, room, hour };
        let mut act = None;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            act = show_start(ui, &mut self.extras.start, &view);
        });
        let Some(act) = act else { return };
        self.extras.home = false;
        match act {
            StartAction::Open(dir) => self.apply_project(crate::projects_ui::ProjectAction::Open(dir)),
            StartAction::Shell => self.run_command("shell"),
            StartAction::Show(id) => {
                self.stage = Some(id.clone());
                self.focus_pane = Some(id);
            }
            StartAction::Start(task) => match self.start_task(&task) {
                Ok(()) => {
                    self.projects.open(&task.dir);
                    self.save_projects();
                    if let Some(cmd) = &task.custom {
                        self.prefs.remember_custom(cmd);
                        self.save_prefs();
                    }
                }
                Err(e) => {
                    self.extras.home = true;
                    self.notice = Some(e);
                }
            },
        }
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
        task.worktree = true;
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
