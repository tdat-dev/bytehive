//! The Phase E pieces (project memory, hand-offs, system), kept out of
//! app.rs: their state lives in one [`Extras`] field and their palette
//! commands are run by [`ShellApp::run_more`].

use eframe::egui;

use crate::app::{now_ms, ShellApp};
use crate::deliver::Delivery;
use crate::handoff::Handoffs;
use crate::memory_ui::MemorySheet;
use crate::projects::pane_home;

#[derive(Default)]
pub struct Extras {
    pub memory: MemorySheet,
    pub handoffs: Handoffs,
    pub system: crate::system::SystemWatch,
    pub remote: crate::remote::Remote,
    pub browser: crate::browser_ui::BrowserWatch,
    pub replay: crate::replay_ui::ReplaySheet,
    pub plan: crate::planmeter::PlanWatch,
    pub tray: crate::tray::Tray,
}

impl ShellApp {
    /// Palette commands of the Phase E pieces; false when `key` is not one.
    pub(crate) fn run_more(&mut self, key: &str) -> bool {
        match key {
            "memory" => self.open_memory(),
            "handoff" => self.open_handoff(),
            "remote" => self.settings.open_at(2),
            "browser" => self.settings.open_at(3),
            "record" => self.toggle_record(),
            "replays" => self.open_replays(),
            _ => return false,
        }
        true
    }

    /// The project the shown agents belong to: the filter, else the one on
    /// stage, else the first open project.
    pub(crate) fn current_project(&self) -> Option<String> {
        if let Some(f) = &self.projects.filter {
            return Some(f.clone());
        }
        let staged = self.stage.as_ref().and_then(|id| self.panes.iter().find(|p| &p.id == id));
        staged.and_then(|p| self.projects.owner(&pane_home(p)).map(str::to_string)).or_else(|| self.projects.dirs.first().cloned())
    }

    pub(crate) fn open_memory(&mut self) {
        if self.extras.memory.open {
            self.extras.memory.open = false;
        } else if let Some(p) = self.current_project() {
            self.extras.memory.open_on(&p);
        } else {
            self.notice = Some("Open a project first: memory belongs to a project.".into());
        }
    }

    /// The live agents of `project`, (pane id, name), the one on stage first.
    pub(crate) fn project_agents(&self, project: &str) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self
            .panes
            .iter()
            .filter(|p| p.cli.is_some() && p.exit_code.is_none())
            .filter(|p| self.projects.owner(&pane_home(p)).is_some_and(|o| crate::projects::normalize(o) == crate::projects::normalize(project)))
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();
        if let Some(i) = self.stage.as_ref().and_then(|s| v.iter().position(|(id, _)| id == s)) {
            let first = v.remove(i);
            v.insert(0, first);
        }
        v
    }

    /// Type `text` into an agent when it is free to take it.
    pub(crate) fn tell(&mut self, pane_id: String, text: String) {
        self.deliveries.push(Delivery::new(pane_id, text, now_ms(), 0));
    }

    /// The Phase E sheets, floating over everything.
    pub(crate) fn extra_sheets(&mut self, ctx: &egui::Context) {
        self.handoff_sheet(ctx);
        self.poll_system(ctx);
        self.poll_browser(ctx);
        let font = self.font.clone();
        self.extras.replay.show(ctx, &font);
        if self.extras.memory.open {
            let agents = self.project_agents(&self.extras.memory.project.clone());
            if let Some((to, text)) = self.extras.memory.show(ctx, &self.projects.dirs, &agents) {
                self.tell(to, text);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_opens_on_the_current_project_and_says_why_not() {
        let mut app = ShellApp::new().unwrap();
        app.projects = Default::default();
        app.run_command("memory");
        assert!(!app.extras.memory.open);
        assert!(app.notice.as_deref().unwrap_or("").contains("Open a project"));
        let dir = std::env::temp_dir().join(format!("maestro-memui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.to_string_lossy().to_string();
        app.projects.open(&d);
        app.run_command("memory");
        assert!(app.extras.memory.open && app.extras.memory.project == d);
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |ui| app.extra_sheets(ui.ctx())).drop_without_applying_deltas();
        std::fs::remove_dir_all(&dir).ok();
    }
}
