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

/// An agent's launch with what ByteHive gives every agent it starts: its name
/// and folder for the bytehive MCP server (`BYTEHIVE_AGENT`, `BYTEHIVE_WORKSPACE`:
/// who moved a card, which project's fleet it is in), and for Claude the laws
/// as a system prompt (the Director's own for the Director). Plain shells
/// are left alone.
pub(crate) fn agent_spec(mut spec: CommandSpec, name: &str, cli: Option<Cli>) -> CommandSpec {
    let Some(cli) = cli else { return spec };
    spec.env.retain(|(k, _)| k != "BYTEHIVE_AGENT" && k != "BYTEHIVE_WORKSPACE");
    spec.env.push(("BYTEHIVE_AGENT".into(), name.to_string()));
    spec.env.push(("BYTEHIVE_WORKSPACE".into(), spec.cwd.clone().unwrap_or_default()));
    if cli == Cli::Claude && !spec.args.iter().any(|a| a == "--append-system-prompt") {
        let laws = if name == crate::laws::DIRECTOR { crate::laws::DIRECTOR_LAWS } else { crate::laws::BYTEHIVE_LAWS };
        spec.args.push("--append-system-prompt".into());
        spec.args.push(laws.into());
    }
    spec
}

impl ShellApp {
    /// A pane named `<name> <n>`, the shell and quick-spawn style.
    pub(crate) fn spawn_pane(&mut self, name: &str, spec: CommandSpec, cli: Option<Cli>, session_id: Option<String>) -> anyhow::Result<String> {
        let full = format!("{name} {}", self.panes.len() + 1);
        self.spawn_named(full, spec, cli, session_id)
    }

    /// A pane under exactly this name (a persona like Ana).
    pub(crate) fn spawn_named(&mut self, name: String, spec: CommandSpec, cli: Option<Cli>, session_id: Option<String>) -> anyhow::Result<String> {
        let spec = agent_spec(spec, &name, cli);
        let id = format!("pane-{}", self.next_id);
        self.next_id += 1;
        let mut pane = Pane::new(id.clone(), name, spec.clone(), 80, 24);
        pane.cli = cli;
        pane.session_id = session_id;
        let tx = pane.sender();
        let tx_exit = pane.sender();
        agent_core::lock(&self.registry).spawn(
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

    /// Boot a custom command in `dir` under `name`.
    pub(crate) fn boot_custom(&mut self, cmd: &str, dir: &str, name: String, worktree: Option<WorktreeInfo>) -> Result<String, String> {
        let spec = crate::custom::spec(cmd, dir).ok_or("Type the command to run.")?;
        let id = self.spawn_named(name, spec, None, None).map_err(|e| format!("Could not start {cmd}: {e:?}"))?;
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == id) {
            pane.custom = Some(cmd.to_string());
            pane.worktree = worktree;
        }
        Ok(id)
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
        let gone = agent_core::lock(&self.registry).take(&pane.id);
        drop(gone);
        pane.reset();
        // Restart carries the conversation on when one is known.
        if let (Some(cli), Some(sid)) = (pane.cli, pane.session_id.clone()) {
            let cwd = pane.spec.cwd.clone().unwrap_or_default();
            // Stopped before its first message, Claude never saved the session:
            // --resume would fail, so boot fresh under the same id instead.
            let (spec, kept) = if cli == Cli::Claude && !agent_core::claude::session_exists(&cwd, &sid) {
                fresh_spec(cli, &cwd, &pane.extra_args, || sid.clone())
            } else {
                let item = agent_core::transcript::SessionItem { id: sid, title: String::new(), cwd, modified_ms: 0 };
                resume_spec(cli, &item, &pane.extra_args)
            };
            pane.spec = agent_spec(spec, &pane.name, Some(cli));
            pane.session_id = kept;
        }
        let spec = pane.spec.clone();
        let id = pane.id.clone();
        let tx = pane.sender();
        let tx_exit = pane.sender();
        let size = PtySize { rows: pane.rows as u16, cols: pane.cols as u16, pixel_width: 0, pixel_height: 0 };
        let _ = agent_core::lock(&self.registry).spawn(
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

    /// Stop a real Claude agent the way the Stop button does, then Resume it.
    /// Spends a little quota: run by hand with --ignored.
    #[test]
    #[ignore]
    fn a_stopped_claude_agent_resumes() {
        use crate::deliver::Delivery;
        let mut app = ShellApp::new().unwrap();
        let dir = std::env::current_dir().unwrap().display().to_string();
        let id = app.boot_agent(Cli::Claude, &dir, "Tess".into(), Vec::new(), None).unwrap();
        let i = app.panes.iter().position(|p| p.id == id).unwrap();
        app.deliveries.push(Delivery::new(id.clone(), "reply with only the word plum and use no tools".into(), crate::app::now_ms(), 3000));
        let pump = |app: &mut ShellApp, secs: u64, until: &dyn Fn(&ShellApp) -> bool| {
            let end = Instant::now() + Duration::from_secs(secs);
            while Instant::now() < end && !until(app) {
                let ShellApp { panes, registry, .. } = &mut *app;
                for p in panes.iter_mut() {
                    p.drain(&mut agent_core::lock(registry));
                }
                app.poll_deliveries();
                app.poll_captures();
                std::thread::sleep(Duration::from_millis(100));
            }
        };
        pump(&mut app, 120, &|a| a.panes[i].screen_text().lines().any(|l| l.contains("plum") && !l.contains("reply with")));
        let before = app.panes[i].screen_text();
        assert!(before.lines().any(|l| l.contains("plum") && !l.contains("reply with")), "never answered:\n{before}");
        let sid = app.panes[i].session_id.clone();
        eprintln!("session before stop: {sid:?}");
        // Stop, as the stage's Stop does.
        drop(agent_core::lock(&app.registry).take(&id));
        app.panes[i].exit_code.get_or_insert(0);
        pump(&mut app, 2, &|_| false);
        // Resume.
        app.restart_pane(i);
        eprintln!("resume args: {:?}", app.panes[i].spec.args);
        pump(&mut app, 60, &|a| a.panes[i].screen_text().contains("plum"));
        let after = app.panes[i].screen_text();
        eprintln!("exit after resume: {:?}\n---screen---\n{after}", app.panes[i].exit_code);
        assert!(app.panes[i].exit_code.is_none(), "it stopped again:\n{after}");
        assert!(after.contains("plum"), "the conversation did not come back:\n{after}");
    }

    #[test]
    fn worktree_spawn_fails_clean_on_a_bad_repo() {
        let mut app = ShellApp::new().unwrap();
        let before = app.panes.len();
        let req = WorktreeRequest {
            cli: Cli::Claude,
            repo: "D:/no-such-repo-bytehive".to_string(),
            branch: "feat/x".to_string(),
            base: "HEAD".to_string(),
        };
        assert!(app.spawn_worktree(req).is_err());
        assert_eq!(app.panes.len(), before);
    }
}