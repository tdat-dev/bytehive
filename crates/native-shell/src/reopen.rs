//! The agents that were running come back when ByteHive starts again, each
//! carrying on its own conversation: closing the app, an update or a crash
//! no longer loses them. Every few seconds the running agents are written to
//! `~/.bytehive/open-agents.json` (a build run from target\ keeps its own
//! file, so trying a change never touches the real list); at start each one
//! is resumed in the folder it worked in.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::app::ShellApp;
use crate::sessions_ui::{fresh_spec, resume_spec, Cli};
use crate::term_pane::{Pane, WorktreeInfo};

/// How often the list is written (only when it changed).
const EVERY: Duration = Duration::from_secs(3);

pub struct Reopen {
    last: String,
    at: Option<Instant>,
}

impl Default for Reopen {
    fn default() -> Self {
        Reopen { last: String::new(), at: None }
    }
}

fn path() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let dev = std::env::current_exe().ok().is_some_and(|p| p.components().any(|c| c.as_os_str() == "target"));
    Some(PathBuf::from(home).join(".bytehive").join(if dev { "open-agents-dev.json" } else { "open-agents.json" }))
}

/// A running agent as the file keeps it; shells are not kept.
pub fn saved(p: &Pane) -> Option<Value> {
    let cli = p.cli?;
    if p.exit_code.is_some() {
        return None;
    }
    let wt = p.worktree.as_ref().map(|w| json!({ "repo": w.repo, "branch": w.branch, "base": w.base, "path": w.path }));
    Some(json!({
        "name": p.name,
        "cli": cli.name(),
        "session": p.session_id,
        "cwd": p.spec.cwd.clone().unwrap_or_default(),
        "extra": p.extra_args,
        "task": p.task,
        "worktree": wt,
    }))
}

fn cli_named(name: &str) -> Option<Cli> {
    Cli::all().into_iter().find(|c| c.name() == name)
}

impl ShellApp {
    /// Write the running agents down when they changed (logic pass).
    pub(crate) fn save_open_agents(&mut self) {
        if cfg!(test) || self.extras.reopen.at.is_some_and(|t| t.elapsed() < EVERY) {
            return;
        }
        self.extras.reopen.at = Some(Instant::now());
        let list: Vec<Value> = self.panes.iter().filter_map(saved).collect();
        let text = serde_json::to_string_pretty(&list).unwrap_or_default();
        if text == self.extras.reopen.last {
            return;
        }
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if std::fs::write(&p, &text).is_ok() {
                self.extras.reopen.last = text;
            }
        }
    }

    /// Bring last run's agents back, each resuming its own session.
    pub(crate) fn reopen_agents(&mut self) {
        let Some(p) = path() else { return };
        let Ok(text) = std::fs::read_to_string(&p) else { return };
        let list: Vec<Value> = serde_json::from_str(&text).unwrap_or_default();
        for a in &list {
            if let Err(e) = self.reopen_one(a) {
                self.notice = Some(e);
            }
        }
        // What is open now is what the file says, so no rewrite is needed.
        self.extras.reopen.last = text;
    }

    fn reopen_one(&mut self, a: &Value) -> Result<(), String> {
        let s = |k: &str| a[k].as_str().unwrap_or_default().to_string();
        let (name, cwd) = (s("name"), s("cwd"));
        let cli = cli_named(&s("cli")).ok_or_else(|| format!("{name}: unknown CLI {}", s("cli")))?;
        if cwd.is_empty() || !std::path::Path::new(&cwd).is_dir() {
            return Err(format!("{name} was not reopened: {cwd} is gone."));
        }
        let extra: Vec<String> = a["extra"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let worktree = a["worktree"].as_object().map(|w| {
            let g = |k: &str| w.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            WorktreeInfo { repo: g("repo"), branch: g("branch"), base: g("base"), path: g("path") }
        });
        let id = match a["session"].as_str().filter(|s| !s.is_empty()) {
            Some(sid) => {
                // Claude saves a session only after its first message: one
                // that never got one boots fresh under the same id instead.
                let (spec, kept) = if cli == Cli::Claude && !agent_core::claude::session_exists(&cwd, sid) {
                    fresh_spec(cli, &cwd, &extra, || sid.to_string())
                } else {
                    let item = agent_core::transcript::SessionItem { id: sid.to_string(), title: String::new(), cwd: cwd.clone(), modified_ms: 0 };
                    resume_spec(cli, &item, &extra)
                };
                let id = self.spawn_named(name.clone(), spec, Some(cli), kept).map_err(|e| format!("{name} was not reopened: {e}"))?;
                if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
                    p.extra_args = extra;
                    p.worktree = worktree;
                }
                id
            }
            // Its session was never learned: a fresh agent in the same place.
            None => self.boot_agent(cli, &cwd, name, extra, worktree)?,
        };
        if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
            p.task = a["task"].as_str().map(String::from);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_running_agents_are_kept() {
        let mut app = ShellApp::new().unwrap();
        // The test app boots one plain shell: never kept.
        assert!(app.panes.iter().all(|p| saved(p).is_none()));
        let p = &mut app.panes[0];
        p.cli = Some(Cli::Claude);
        p.session_id = Some("abc".into());
        p.name = "Ana".into();
        let v = saved(p).expect("a running agent is kept");
        assert_eq!(v["name"], "Ana");
        assert_eq!(v["cli"], Cli::Claude.name());
        assert_eq!(v["session"], "abc");
        assert_eq!(cli_named(v["cli"].as_str().unwrap()), Some(Cli::Claude));
        p.exit_code = Some(0);
        assert!(saved(p).is_none(), "a stopped agent is not reopened");
    }

    #[test]
    fn a_gone_folder_is_reported_not_spawned() {
        let mut app = ShellApp::new().unwrap();
        let before = app.panes.len();
        let a = json!({ "name": "Bob", "cli": Cli::Claude.name(), "session": "x", "cwd": "D:/no-such-folder-bytehive" });
        assert!(app.reopen_one(&a).unwrap_err().contains("is gone"));
        assert_eq!(app.panes.len(), before);
    }
}
