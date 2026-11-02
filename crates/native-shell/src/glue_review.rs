//! Wiring for Phase B's review surfaces: the Changes panel follows the Review
//! tab's pane and diff, and the Files drawer lists a project or an agent's
//! worktree and opens files in your editor.

use eframe::egui;

use crate::app::ShellApp;
use crate::changes_ui::ChangesView;
use crate::commands::SideTab;
use crate::files_ui::FileSource;
use crate::land_ui::LandOut;
use crate::projects::project_name;

impl ShellApp {
    /// Folders the Files drawer can show: every open project, then every
    /// agent working in its own worktree.
    pub(crate) fn file_sources(&self) -> Vec<FileSource> {
        let mut v: Vec<FileSource> = self.projects.dirs.iter().map(|d| FileSource { label: project_name(d), dir: d.clone() }).collect();
        for p in &self.panes {
            if let Some(w) = &p.worktree {
                if !p.merged {
                    v.push(FileSource { label: format!("{} \u{00B7} {}", p.name, w.branch), dir: w.path.clone() });
                }
            }
        }
        v
    }

    /// What Files opens on: the staged agent's worktree, else the project on
    /// show, else the first open one.
    fn default_file_source(&self) -> Option<String> {
        let staged = self.stage.as_ref().and_then(|id| self.panes.iter().find(|p| &p.id == id));
        staged
            .and_then(|p| p.worktree.as_ref().map(|w| w.path.clone()))
            .or_else(|| self.projects.filter.clone())
            .or_else(|| self.projects.dirs.first().cloned())
    }

    pub(crate) fn toggle_files(&mut self) {
        let src = self.default_file_source();
        self.files.toggle(src);
    }

    /// Open Changes on the staged (or first shown) agent, at its first file.
    pub(crate) fn open_changes(&mut self) {
        let hidden = self.hidden_panes();
        let target = self.stage.clone().or_else(|| self.panes.iter().find(|p| !hidden.contains(&p.id)).map(|p| p.id.clone()));
        if let Some(id) = target {
            self.review.select(&id);
        }
        // Review reads the diff; keep its tab up so the snapshot stays fresh,
        // and hand the agents back once Changes closes.
        if self.side_tab != SideTab::Review {
            self.side_tab = SideTab::Review;
            self.extras.review_by_changes = true;
        }
        self.changes.open_at(0);
    }

    /// The Changes panel, docked right before the grid takes the rest.
    pub(crate) fn side_panels(&mut self, ui: &mut egui::Ui) {
        if let Some(i) = self.review.open_changes.take() {
            self.changes.open_at(i);
        }
        match self.review.land_out.take() {
            Some(LandOut::Tell(id, text)) => self.tell(id, text),
            Some(LandOut::Discarded(id)) => {
                // Its folder is gone: the agent goes with it.
                let name = self.panes.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
                drop(agent_core::lock(&self.registry).take(&id));
                self.panes.retain(|p| p.id != id);
                self.notice = Some(format!("Discarded {name}'s branch."));
            }
            _ => {}
        }
        if !self.changes.open {
            if std::mem::take(&mut self.extras.review_by_changes) && self.side_tab == SideTab::Review {
                self.side_tab = SideTab::Queue;
            }
            return;
        }
        let (id, snap) = self.review.current();
        let Some(id) = id.map(str::to_string) else {
            self.changes.open = false;
            return;
        };
        let pane = self.panes.iter().find(|p| p.id == id);
        let name = pane.map(|p| p.name.clone()).unwrap_or_default();
        let base = pane.and_then(|p| p.worktree.as_ref().map(|w| w.base.clone()));
        let branch = snap.and_then(|s| s.branch.clone());
        let files = snap.map(|s| s.files.as_slice());
        let view = ChangesView { pane_id: &id, pane_name: &name, branch: branch.as_deref(), base: base.as_deref(), files };
        self.changes.show(ui, view);
    }

    /// The Files drawer floats on top; a picked file opens in your editor.
    pub(crate) fn drawers(&mut self, ctx: &egui::Context) {
        let sources = self.file_sources();
        if let Some(path) = self.files.show(ctx, &sources) {
            if let Err(e) = crate::editor::open_in_editor(&path) {
                self.notice = Some(e);
            }
        }
    }
}
