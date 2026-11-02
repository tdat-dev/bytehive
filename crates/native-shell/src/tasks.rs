//! Starting a New task: name the agents, give each its own worktree (off the
//! UI thread, since `git worktree add` takes seconds on a big repo), boot
//! them, then type the task into each once it reaches its prompt.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use agent_core::git::{create_worktree, current_branch, repo_root, worktree_path_for};
use alacritty_terminal::term::TermMode;

use crate::app::{now_ms, ShellApp};
use crate::deliver::{Delivery, Tick};
use crate::newtask::{Launch, NewTask};
use crate::term_pane::WorktreeInfo;

/// A worktree job's answer, sent back to the UI thread.
pub(crate) enum LaunchMsg {
    Ready { launch: Launch, worktree: WorktreeInfo },
    Failed { name: String, error: String },
}

/// Branch off `base` into a fresh worktree. A branch name already taken (the
/// same task run before) gets `-2`, `-3`... rather than failing the agent.
fn make_worktree(repo: &str, base: &str, mut launch: Launch, path_for: impl Fn(&str, &str) -> PathBuf) -> LaunchMsg {
    let want = launch.branch.clone().unwrap_or_default();
    for n in 1..10 {
        let branch = if n == 1 { want.clone() } else { format!("{want}-{n}") };
        let path = path_for(repo, &branch);
        if path.exists() {
            continue;
        }
        if create_worktree(Path::new(repo), &path, &branch, base).is_some() {
            launch.branch = Some(branch.clone());
            let worktree = WorktreeInfo { repo: repo.to_string(), branch, base: base.to_string(), path: path.to_string_lossy().into_owned() };
            return LaunchMsg::Ready { launch, worktree };
        }
    }
    LaunchMsg::Failed { name: launch.name, error: format!("Could not branch {want} off {base} in {repo}.") }
}

fn spawn_worktrees(tx: Sender<LaunchMsg>, repo: String, base: String, launches: Vec<Launch>) {
    std::thread::spawn(move || {
        for l in launches {
            if tx.send(make_worktree(&repo, &base, l, worktree_path_for)).is_err() {
                return;
            }
        }
    });
}

impl ShellApp {
    /// Start the sheet's task. An error keeps the sheet open with the reason.
    pub(crate) fn start_task(&mut self, form: &NewTask) -> Result<(), String> {
        if let Some(problem) = form.problem() {
            return Err(problem);
        }
        let mut taken: Vec<String> = self.panes.iter().map(|p| p.name.clone()).collect();
        taken.extend(self.launching.iter().cloned());
        let launches = form.launches(&taken);
        if form.worktree {
            let Some(repo) = repo_root(Path::new(&form.dir)) else {
                return Err(format!("Own worktree needs a git repo, and {} is not in one. Turn it off to work in the folder itself.", form.dir));
            };
            let base = current_branch(Path::new(&repo)).unwrap_or_else(|| "HEAD".to_string());
            self.launching.extend(launches.iter().map(|l| l.name.clone()));
            spawn_worktrees(self.launch_tx.clone(), repo, base, launches);
        } else {
            let dir = form.dir.clone();
            for (i, l) in launches.into_iter().enumerate() {
                self.boot_launch(l, &dir, None, i == 0)?;
            }
        }
        Ok(())
    }

    fn boot_launch(&mut self, l: Launch, dir: &str, worktree: Option<WorktreeInfo>, reveal: bool) -> Result<(), String> {
        let id = self.boot_agent(l.cli, dir, l.name, l.extra_args, worktree)?;
        let title: String = l.task.lines().map(str::trim).find(|s| !s.is_empty()).unwrap_or("").chars().take(80).collect();
        if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
            p.task = Some(title).filter(|t| !t.is_empty());
        }
        // No job: the agent opens and waits for you.
        if !l.task.trim().is_empty() {
            self.deliveries.push(Delivery::new(id.clone(), l.task, now_ms(), self.prefs.job_delay_ms()));
        }
        if reveal {
            // The agent just started is the one on screen.
            self.focus_pane = Some(id);
        }
        Ok(())
    }

    /// Boot agents whose worktree is ready.
    pub(crate) fn poll_launches(&mut self) {
        while let Ok(msg) = self.launch_rx.try_recv() {
            match msg {
                LaunchMsg::Ready { launch, worktree } => {
                    let first = self.launching.first() == Some(&launch.name);
                    self.launching.retain(|n| n != &launch.name);
                    let dir = worktree.path.clone();
                    if let Err(e) = self.boot_launch(launch, &dir, Some(worktree), first) {
                        self.notice = Some(e);
                    }
                }
                LaunchMsg::Failed { name, error } => {
                    self.launching.retain(|n| n != &name);
                    self.notice = Some(error);
                }
            }
        }
    }

    /// Type each waiting task once its agent is ready for it.
    pub(crate) fn poll_deliveries(&mut self) {
        if self.deliveries.is_empty() {
            return;
        }
        let now = now_ms();
        let mut i = 0;
        while i < self.deliveries.len() {
            let pane_id = self.deliveries[i].pane_id.clone();
            let tick = match self.panes.iter().find(|p| p.id == pane_id) {
                None => Tick::Finished,
                Some(p) => {
                    let paste = p.term.mode().contains(TermMode::BRACKETED_PASTE);
                    self.deliveries[i].tick(now, p.exit_code.is_none(), &p.screen_text(), paste)
                }
            };
            match tick {
                Tick::Finished => {
                    self.deliveries.remove(i);
                    continue;
                }
                Tick::Write(bytes) => {
                    let _ = agent_core::lock(&self.registry).write_input(&pane_id, &bytes);
                }
                Tick::Wait => {}
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions_ui::Cli;

    #[test]
    fn a_race_outside_a_repo_is_refused_before_anything_starts() {
        let mut app = ShellApp::new().unwrap();
        let before = app.panes.len();
        let mut t = NewTask::new("D:/no-such-repo-bytehive".to_string());
        t.prompt = "Fix it".to_string();
        t.set_count(2);
        t.worktree = true;
        t.cli = Cli::Claude;
        let err = app.start_task(&t).unwrap_err();
        assert!(err.contains("git repo"), "{err}");
        assert_eq!(app.panes.len(), before);
        assert!(app.launching.is_empty());
    }

    fn git(dir: &Path, args: &[&str]) -> bool {
        std::process::Command::new("git").arg("-C").arg(dir).args(args).output().is_ok_and(|o| o.status.success())
    }

    #[test]
    fn a_taken_branch_gets_a_number() {
        let dir = std::env::temp_dir().join(format!("bytehive-task-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = git(&dir, &["init"])
            && git(&dir, &["config", "user.email", "t@bytehive.local"])
            && git(&dir, &["config", "user.name", "t"])
            && git(&dir, &["commit", "--allow-empty", "-m", "first"]);
        if !ok {
            return; // no git here
        }
        let repo = dir.to_string_lossy().into_owned();
        let wts = dir.join("wts");
        let path_for = |_: &str, b: &str| wts.join(b.replace('/', "_"));
        let launch = |name: &str| Launch { name: name.into(), cli: Cli::Claude, branch: Some("ana/fix".into()), extra_args: vec![], task: "fix".into() };
        let LaunchMsg::Ready { worktree: first, .. } = make_worktree(&repo, "HEAD", launch("Ana"), path_for) else { panic!("first worktree") };
        let LaunchMsg::Ready { launch: l, worktree: second } = make_worktree(&repo, "HEAD", launch("Bob"), path_for) else { panic!("second worktree") };
        assert_eq!(first.branch, "ana/fix");
        assert_eq!(second.branch, "ana/fix-2");
        assert_eq!(l.branch.as_deref(), Some("ana/fix-2"));
        assert!(Path::new(&second.path).is_dir());
        for wt in [&first.path, &second.path] {
            git(&dir, &["worktree", "remove", "--force", wt]);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_task_is_typed_into_its_pane_once_it_is_up() {
        let mut app = ShellApp::new().unwrap();
        let id = app.panes[0].id.clone();
        app.deliveries.push(Delivery::new(id.clone(), "echo task-landed-42".to_string(), now_ms(), crate::deliver::JOB_DELAY_MS));
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut seen = false;
        while std::time::Instant::now() < end && !seen {
            let ShellApp { panes, registry, .. } = &mut app;
            panes[0].drain(&mut agent_core::lock(registry));
            app.poll_deliveries();
            // The echo's output sits on a line of its own, not the typed one.
            seen = app.panes[0].screen_text().lines().any(|l| l.trim() == "task-landed-42");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(seen, "task ran: {}", app.panes[0].screen_text());
    }

    #[test]
    fn a_folder_is_still_needed() {
        let mut app = ShellApp::new().unwrap();
        let t = NewTask::new(String::new());
        assert!(app.start_task(&t).is_err());
    }
}
