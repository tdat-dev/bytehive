//! The agents in the list come back when ByteHive starts again: closing the
//! app, an update or a crash no longer loses them. Every few seconds they are
//! written to `~/.bytehive/open-agents.json` (a build run from target\ keeps
//! its own file, so trying a change never touches the real list). At start
//! each one is back in the list, stopped: nothing runs until you press
//! Resume, which carries its own conversation on in its folder.

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

/// An agent in the list as the file keeps it, running or stopped (a stopped
/// one keeps its Resume); shells are not kept.
pub fn saved(p: &Pane) -> Option<Value> {
    if p.merged || (p.cli.is_none() && p.custom.is_none()) {
        return None; // landed and done, or a plain shell
    }
    let wt = p.worktree.as_ref().map(|w| json!({ "repo": w.repo, "branch": w.branch, "base": w.base, "path": w.path }));
    Some(json!({
        "name": p.name,
        "cli": p.cli.map(|c| c.name()),
        "custom": p.custom,
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

    /// Put one agent back in the list, stopped: nothing runs until Resume
    /// (the same as an agent you stopped), which carries its session on.
    fn reopen_one(&mut self, a: &Value) -> Result<(), String> {
        let s = |k: &str| a[k].as_str().unwrap_or_default().to_string();
        let (name, cwd) = (s("name"), s("cwd"));
        let custom = a["custom"].as_str().filter(|c| !c.trim().is_empty()).map(String::from);
        let cli = cli_named(&s("cli"));
        if cli.is_none() && custom.is_none() {
            return Err(format!("{name}: unknown CLI {}", s("cli")));
        }
        if cwd.is_empty() || !std::path::Path::new(&cwd).is_dir() {
            return Err(format!("{name} was not reopened: {cwd} is gone."));
        }
        let extra: Vec<String> = a["extra"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let worktree = a["worktree"].as_object().map(|w| {
            let g = |k: &str| w.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            WorktreeInfo { repo: g("repo"), branch: g("branch"), base: g("base"), path: g("path") }
        });
        let session = a["session"].as_str().filter(|s| !s.is_empty()).map(String::from);
        // Resume rebuilds the launch from the session; with none known it
        // starts the CLI fresh in the same place.
        let (spec, kept) = match (cli, &session) {
            (None, _) => (crate::custom::spec(custom.as_deref().unwrap_or_default(), &cwd).ok_or("empty command")?, None),
            (Some(cli), Some(sid)) => {
                let item = agent_core::transcript::SessionItem { id: sid.clone(), title: String::new(), cwd: cwd.clone(), modified_ms: 0 };
                resume_spec(cli, &item, &extra)
            }
            (Some(cli), None) => fresh_spec(cli, &cwd, &extra, || uuid::Uuid::new_v4().to_string()),
        };
        let id = format!("pane-{}", self.next_id);
        self.next_id += 1;
        let mut pane = Pane::new(id, name.clone(), crate::spawn::agent_spec(spec, &name, cli), 80, 24);
        pane.cli = cli;
        pane.custom = custom;
        pane.session_id = kept;
        pane.extra_args = extra;
        pane.worktree = worktree;
        pane.task = a["task"].as_str().map(String::from);
        pane.exit_code = Some(0);
        let note = format!("\r\n  {name} was open when ByteHive last closed.\r\n  Resume carries on its conversation.\r\n");
        let _ = pane.sender().send(crate::term_pane::Msg::Output(note.into_bytes()));
        self.panes.push(pane);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_are_kept_shells_and_merged_ones_are_not() {
        let mut app = ShellApp::new().unwrap();
        // The test app boots one plain shell: never kept.
        assert!(app.panes.iter().all(|p| saved(p).is_none()));
        let p = &mut app.panes[0];
        p.cli = Some(Cli::Claude);
        p.session_id = Some("abc".into());
        p.name = "Ana".into();
        let v = saved(p).expect("an agent is kept");
        assert_eq!(v["name"], "Ana");
        assert_eq!(v["cli"], Cli::Claude.name());
        assert_eq!(v["session"], "abc");
        assert_eq!(cli_named(v["cli"].as_str().unwrap()), Some(Cli::Claude));
        p.exit_code = Some(0);
        assert!(saved(p).is_some(), "a stopped agent keeps its Resume");
        p.merged = true;
        assert!(saved(p).is_none(), "a merged one is done");
    }

    #[test]
    fn a_reopened_agent_waits_for_resume() {
        let mut app = ShellApp::new().unwrap();
        let before = app.panes.len();
        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        let a = json!({ "name": "Cid", "cli": Cli::Codex.name(), "session": "s-1", "cwd": dir, "task": "Fix it" });
        app.reopen_one(&a).unwrap();
        assert_eq!(app.panes.len(), before + 1);
        let p = app.panes.last().unwrap();
        assert_eq!(p.name, "Cid");
        assert_eq!(p.session_id.as_deref(), Some("s-1"));
        assert!(p.exit_code.is_some(), "stopped: nothing runs until Resume");
        assert!(agent_core::lock(&app.registry).take(&p.id).is_none(), "no process was started");
        assert_eq!(p.task.as_deref(), Some("Fix it"));
    }

    #[test]
    fn a_custom_agent_comes_back_with_its_command() {
        let mut app = ShellApp::new().unwrap();
        let p = &mut app.panes[0];
        p.custom = Some("ollama run llama3.1".into());
        p.name = "Eve".into();
        let v = saved(p).expect("a custom agent is kept");
        assert_eq!(v["custom"], "ollama run llama3.1");
        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        let a = json!({ "name": "Eve", "cli": null, "custom": "ollama run llama3.1", "cwd": dir });
        app.reopen_one(&a).unwrap();
        let p = app.panes.last().unwrap();
        assert_eq!(p.cli, None);
        assert_eq!(p.custom.as_deref(), Some("ollama run llama3.1"));
        assert!(p.exit_code.is_some(), "stopped until Resume");
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
