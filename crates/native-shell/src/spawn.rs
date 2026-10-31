//! Spawning: shells, fresh agent boots, resumes, restarts, and the session
//! capture that learns a fresh boot's id from the CLI store.

use std::path::Path;

use agent_core::git::{create_worktree, worktree_path_for};
use agent_core::CommandSpec;
use portable_pty::PtySize;
use std::time::{Duration, Instant};

use crate::app::{now_ms, ShellApp};
use crate::sessions_ui::{fresh_spec, resume_spec, Cli, ResumeRequest, WorktreeRequest};
use crate::term_pane::{Msg, Pane, WorktreeInfo};

/// A fresh agent boot whose session id is still unknown: polled until the
/// CLI's store shows it, then remembered on the pane.
pub(crate) struct PendingCapture {
    pub(crate) pane_id: String,
    pub(crate) cli: Cli,
    dir: String,
    since_ms: u64,
    tries: u8,
}

impl ShellApp {
    /// A pane named `<name> <n>`, the shell and quick-spawn style.
    pub(crate) fn spawn_pane(&mut self, name: &str, spec: CommandSpec, cli: Option<Cli>, session_id: Option<String>) -> anyhow::Result<String> {
        let full = format!("{name} {}", self.panes.len() + 1);
        self.spawn_named(full, spec, cli, session_id)
    }

    /// A pane under exactly this name (a persona like Ana).
    pub(crate) fn spawn_named(&mut self, name: String, spec: CommandSpec, cli: Option<Cli>, session_id: Option<String>) -> anyhow::Result<String> {
        let id = format!("pane-{}", self.next_id);
        self.next_id += 1;
        let mut pane = Pane::new(id.clone(), name, spec.clone(), 80, 24);
        pane.cli = cli;
        pane.session_id = session_id;
        let tx = pane.sender();
        let tx_exit = pane.sender();
        self.registry.spawn(
            id.clone(),
            &spec,
            PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 },
            move |bytes| {
                let _ = tx.send(Msg::Output(bytes.to_vec()));
            },
            move |code| {
                let _ = tx_exit.send(Msg::Exit(code));
            },
        )?;
        self.panes.push(pane);
        Ok(id)
    }

    /// A fresh agent boot: Claude gets a named session up front, the rest are
    /// captured after spawning.
    pub(crate) fn spawn_fresh(&mut self, cli: Cli) {
        let dir = self.sessions.dir.clone();
        let name = format!("{} {}", cli.name().to_lowercase(), self.panes.len() + 1);
        if let Err(e) = self.boot_agent(cli, &dir, name, Vec::new(), None) {
            eprintln!("spawn {} failed: {e}", cli.name());
        }
    }

    /// Boot a fresh agent in `dir` under `name`, with `extra` launch flags,
    /// learning its session id afterwards when the CLI can't be told one.
    pub(crate) fn boot_agent(&mut self, cli: Cli, dir: &str, name: String, extra: Vec<String>, worktree: Option<WorktreeInfo>) -> Result<String, String> {
        let (spec, kept) = fresh_spec(cli, dir, &extra, || uuid::Uuid::new_v4().to_string());
        let since = now_ms();
        let id = self.spawn_named(name, spec, Some(cli), kept).map_err(|e| format!("Spawn failed: {e:?}"))?;
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == id) {
            pane.extra_args = extra;
            pane.worktree = worktree;
            if pane.session_id.is_none() {
                self.captures.push(PendingCapture { pane_id: id.clone(), cli, dir: dir.to_string(), since_ms: since, tries: 0 });
            }
        }
        Ok(id)
    }

    /// Carry on a past session in the folder it ran in.
    pub(crate) fn spawn_resume(&mut self, req: ResumeRequest) {
        let name = req.cli.name().to_lowercase();
        let (spec, kept) = resume_spec(req.cli, &req.session, &[]);
        if self.spawn_pane(&name, spec, Some(req.cli), kept).is_err() {
            eprintln!("resume {} failed", req.session.id);
        }
    }

    /// A fresh boot in its own worktree: branch off the repo, work there, so
    /// review can later merge the branch back. Fails before spawning when git
    /// refuses (not a repo, branch taken, base unknown).
    pub(crate) fn spawn_worktree(&mut self, req: WorktreeRequest) -> Result<String, String> {
        if req.branch.trim().is_empty() {
            return Err("Branch name is empty.".to_string());
        }
        let path = worktree_path_for(&req.repo, &req.branch);
        let created = create_worktree(Path::new(&req.repo), &path, &req.branch, &req.base);
        if created.is_none() {
            return Err(format!(
                "Could not branch {} off {} in {}. Is it a git repo, is the branch taken, does the base exist?",
                req.branch, req.base, req.repo
            ));
        }
        let dir = path.to_string_lossy().into_owned();
        let info = WorktreeInfo { repo: req.repo.clone(), branch: req.branch.clone(), base: req.base.clone(), path: dir.clone() };
        let name = format!("{} {}", req.cli.name().to_lowercase(), self.panes.len() + 1);
        self.boot_agent(req.cli, &dir, name, Vec::new(), Some(info))
    }

    pub(crate) fn restart_pane(&mut self, index: usize) {
        let Some(pane) = self.panes.get_mut(index) else { return };
        self.registry.kill(&pane.id);
        pane.reset();
        // Restart carries the conversation on when one is known.
        if let (Some(cli), Some(sid)) = (pane.cli, pane.session_id.clone()) {
            let cwd = pane.spec.cwd.clone().unwrap_or_default();
            let item = agent_core::transcript::SessionItem { id: sid, title: String::new(), cwd, modified_ms: 0 };
            let (spec, kept) = resume_spec(cli, &item, &pane.extra_args);
            pane.spec = spec;
            pane.session_id = kept;
        }
        let spec = pane.spec.clone();
        let id = pane.id.clone();
        let tx = pane.sender();
        let tx_exit = pane.sender();
        let size = PtySize { rows: pane.rows as u16, cols: pane.cols as u16, pixel_width: 0, pixel_height: 0 };
        let _ = self.registry.spawn(
            id,
            &spec,
            size,
            move |bytes| {
                let _ = tx.send(Msg::Output(bytes.to_vec()));
            },
            move |code| {
                let _ = tx_exit.send(Msg::Exit(code));
            },
        );
    }

    /// Poll pending captures about once a second; give up after ~15 tries.
    pub(crate) fn poll_captures(&mut self) {
        if self.last_poll.elapsed() < Duration::from_secs(1) || self.captures.is_empty() {
            return;
        }
        self.last_poll = Instant::now();
        let mut done = Vec::new();
        for (i, cap) in self.captures.iter_mut().enumerate() {
            cap.tries += 1;
            if let Some(id) = cap.cli.latest_since(&cap.dir, cap.since_ms) {
                if let Some(pane) = self.panes.iter_mut().find(|p| p.id == cap.pane_id) {
                    pane.session_id = Some(id);
                }
                done.push(i);
            } else if cap.tries >= 15 {
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            self.captures.remove(i);
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions_ui::WorktreeRequest;

    #[test]
    fn worktree_spawn_fails_clean_on_a_bad_repo() {
        let mut app = ShellApp::new().unwrap();
        let before = app.panes.len();
        let req = WorktreeRequest {
            cli: Cli::Claude,
            repo: "D:/no-such-repo-maestro".to_string(),
            branch: "feat/x".to_string(),
            base: "HEAD".to_string(),
        };
        assert!(app.spawn_worktree(req).is_err());
        assert_eq!(app.panes.len(), before);
    }
}