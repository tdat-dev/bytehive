//! Resume panel: pick a CLI, see its past sessions, carry one on.
//!
//! All store access goes through agent-core; this module only shapes it for
//! the UI and builds the spawn specs (fresh boot or resume).

use agent_core::launch::{claude_session_args, codex_session_args, launch_spec, muse_session_args, opencode_session_args};
use agent_core::transcript::SessionItem;
use agent_core::{claude, codex, muse, opencode, CommandSpec};
use eframe::egui::{self};

use crate::theme::{DEL, DIM, FAINT};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cli {
    Claude,
    Codex,
    Muse,
    Opencode,
}

impl Cli {
    pub fn all() -> [Cli; 4] {
        [Cli::Claude, Cli::Codex, Cli::Muse, Cli::Opencode]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Cli::Claude => "Claude",
            Cli::Codex => "Codex",
            Cli::Muse => "Muse",
            Cli::Opencode => "Opencode",
        }
    }

    pub fn program(&self) -> &'static str {
        match self {
            Cli::Claude => "claude",
            Cli::Codex => "codex",
            Cli::Muse => "muse",
            Cli::Opencode => "opencode",
        }
    }

    /// Past sessions for the resume list, newest first.
    pub fn list(&self, dir: &str) -> Vec<SessionItem> {
        match self {
            Cli::Claude => {
                let mut out: Vec<SessionItem> = claude::sessions(dir)
                    .into_iter()
                    .map(|s| SessionItem { id: s.id, title: s.title, cwd: s.cwd, modified_ms: s.modified_ms })
                    .collect();
                // Plus ones begun elsewhere, so they can be carried on in
                // their own folder.
                for s in claude::sessions_everywhere(20) {
                    if !out.iter().any(|o| o.id == s.id) {
                        out.push(SessionItem { id: s.id, title: s.title, cwd: s.cwd, modified_ms: s.modified_ms });
                    }
                }
                out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
                out.truncate(40);
                out
            }
            Cli::Codex => codex::sessions_root().map_or(Vec::new(), |r| codex::list_in(&r, 40)),
            Cli::Muse => muse::list(40)
                .into_iter()
                .map(|s| SessionItem { id: s.id, title: s.title, cwd: String::new(), modified_ms: s.modified_ms })
                .collect(),
            Cli::Opencode => opencode::db_path().map_or(Vec::new(), |db| opencode::list_in(&db, 40)),
        }
    }

    /// The newest session touched since `since_ms`, for learning a fresh
    /// run's session id right after spawning it.
    pub fn latest_since(&self, dir: &str, since_ms: u64) -> Option<String> {
        match self {
            Cli::Claude => claude::sessions(dir).into_iter().find(|s| s.modified_ms >= since_ms).map(|s| s.id),
            Cli::Codex => codex::sessions_root().and_then(|r| codex::latest_session_in(&r, dir, since_ms)),
            Cli::Muse => muse::sessions().into_iter().find(|s| s.modified_ms >= since_ms).map(|s| s.id),
            Cli::Opencode => opencode::db_path().and_then(|db| opencode::latest_session_in(&db, dir, since_ms)),
        }
    }
}

/// A fresh boot's spawn spec: Claude gets a named session up front, the rest
/// boot blank and the capture after spawning learns their id.
pub fn fresh_spec(cli: Cli, dir: &str, fresh_id: impl Fn() -> String) -> (CommandSpec, Option<String>) {
    let (args, kept) = match cli {
        Cli::Claude => claude_session_args(&[], None, dir, |_, _| false, fresh_id),
        Cli::Codex => codex_session_args(&[], None, |_| false),
        Cli::Muse => muse_session_args(&[], None, |_| false),
        Cli::Opencode => opencode_session_args(&[], None, |_| false),
    };
    let mut spec = launch_spec(cli.program(), &args);
    spec.cwd = Some(dir.to_string());
    (spec, kept)
}

/// A resume's spawn spec: carry on `session` in the folder it ran in.
pub fn resume_spec(cli: Cli, session: &SessionItem) -> (CommandSpec, Option<String>) {
    let id = Some(session.id.clone());
    let (args, kept) = match cli {
        Cli::Claude => claude_session_args(&[], id, &session.cwd, |_, _| true, || unreachable_id()),
        Cli::Codex => codex_session_args(&[], id, |_| true),
        Cli::Muse => muse_session_args(&[], id, |_| true),
        Cli::Opencode => opencode_session_args(&[], id, |_| true),
    };
    let mut spec = launch_spec(cli.program(), &args);
    if !session.cwd.is_empty() {
        spec.cwd = Some(session.cwd.clone());
    }
    (spec, kept)
}

fn unreachable_id() -> String {
    unreachable!("a session that exists never needs a fresh id")
}

pub struct ResumeRequest {
    pub cli: Cli,
    pub session: SessionItem,
}

/// A fresh agent boot inside its own worktree: repo to branch from, branch to
/// work on, base to merge back into.
pub struct WorktreeRequest {
    pub cli: Cli,
    pub repo: String,
    pub branch: String,
    pub base: String,
}

pub struct SessionsPanel {
    pub cli: Cli,
    pub dir: String,
    items: Vec<SessionItem>,
    loaded: bool,
    wt_branch: String,
    wt_base: String,
    pub wt_error: Option<String>,
}

impl SessionsPanel {
    pub fn new() -> Self {
        let dir = std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "D:\\maestro".to_string());
        SessionsPanel { cli: Cli::Claude, dir, items: Vec::new(), loaded: false, wt_branch: String::new(), wt_base: "HEAD".to_string(), wt_error: None }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<ResumeRequest> {
        let mut out = None;
        ui.heading("Sessions");
        ui.horizontal(|ui| {
            for cli in Cli::all() {
                if ui.selectable_label(self.cli == cli, cli.name()).clicked() {
                    self.cli = cli;
                    self.loaded = false;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Dir");
            if ui.text_edit_singleline(&mut self.dir).changed() {
                self.loaded = false;
            }
            if ui.button("Refresh").clicked() {
                self.loaded = false;
            }
        });
        if !self.loaded {
            self.items = self.cli.list(&self.dir);
            self.loaded = true;
        }
        if self.items.is_empty() {
            ui.label(egui::RichText::new("No sessions yet.").color(FAINT));
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for s in &self.items {
                ui.horizontal(|ui| {
                    let title: String = if s.title.is_empty() { s.id.chars().take(8).collect() } else { s.title.chars().take(32).collect() };
                    ui.label(title).on_hover_text(&s.id);
                    if !s.cwd.is_empty() {
                        ui.label(egui::RichText::new(s.cwd.chars().take(28).collect::<String>()).color(DIM));
                    }
                    if ui.small_button("Resume").clicked() {
                        out = Some(ResumeRequest { cli: self.cli, session: s.clone() });
                    }
                });
            }
        });
        out
    }

    /// A fresh boot in its own worktree, branched off Dir above.
    pub fn show_worktree(&mut self, ui: &mut egui::Ui) -> Option<WorktreeRequest> {
        let mut out = None;
        ui.separator();
        ui.label(egui::RichText::new("New agent in a worktree").strong());
        ui.horizontal(|ui| {
            ui.label("Branch");
            ui.text_edit_singleline(&mut self.wt_branch);
            ui.label("Base");
            ui.text_edit_singleline(&mut self.wt_base);
            if ui.small_button(format!("Spawn {}", self.cli.name())).clicked() {
                let branch = self.wt_branch.trim().to_string();
                let base = self.wt_base.trim().to_string();
                if branch.is_empty() {
                    self.wt_error = Some("Branch name is empty.".to_string());
                } else if base.is_empty() {
                    self.wt_error = Some("Base is empty.".to_string());
                } else {
                    self.wt_error = None;
                    self.wt_branch.clear();
                    out = Some(WorktreeRequest { cli: self.cli, repo: self.dir.clone(), branch, base });
                }
            }
        });
        if let Some(err) = self.wt_error.as_deref() {
            ui.label(egui::RichText::new(err).small().color(DEL));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_specs_carry_the_session_in_its_folder() {
        let s = SessionItem { id: "abc-123".into(), title: "t".into(), cwd: "D:/app".into(), modified_ms: 9 };
        let (spec, kept) = resume_spec(Cli::Claude, &s);
        assert_eq!(spec.program, "cmd.exe");
        assert_eq!(spec.args, ["/c", "claude", "--resume", "abc-123"]);
        assert_eq!(spec.cwd.as_deref(), Some("D:/app"));
        assert_eq!(kept.as_deref(), Some("abc-123"));
        let (spec, _) = resume_spec(Cli::Codex, &s);
        assert_eq!(spec.args[2], "resume");
        let (spec, _) = resume_spec(Cli::Muse, &s);
        assert_eq!(spec.args[2], "resume");
        let (spec, _) = resume_spec(Cli::Opencode, &s);
        assert_eq!(spec.args[2..], ["--session".to_string(), "abc-123".to_string()]);
    }

    #[test]
    fn fresh_specs_boot_named_for_claude_blank_for_the_rest() {
        let (spec, kept) = fresh_spec(Cli::Claude, "D:/app", || "fresh-1".to_string());
        assert_eq!(spec.args[2..], ["--session-id".to_string(), "fresh-1".to_string()]);
        assert_eq!(kept.as_deref(), Some("fresh-1"));
        assert_eq!(spec.cwd.as_deref(), Some("D:/app"));
        let (spec, kept) = fresh_spec(Cli::Codex, "D:/app", || "x".to_string());
        assert_eq!(spec.args, ["/c", "codex"]);
        assert_eq!(kept, None);
    }
}
