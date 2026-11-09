//! The fleet: every agent the app knows, running or stopped, saved to
//! ~/.bytehive/native/session.json. Like today's app, a restart brings every
//! agent back stopped; Resume carries on its own conversation.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_core::{claude, codex, launch, opencode};
use gpui_kit::*;
use serde::{Deserialize, Serialize};

use supervise::task_state::{TaskFacts, TaskState, TaskStatus, derive_task_state};

use crate::follow;
use crate::terminal::{SCROLLBACK, SCROLLBACK_HIDDEN, Terminal};

const NAMES: [&str; 12] = ["Ana", "Bob", "Cid", "Dee", "Eve", "Fay", "Gus", "Hal", "Ivy", "Jon", "Kai", "Lou"];

/// What it takes to start or resume an agent.
#[derive(Clone, Serialize, Deserialize)]
pub struct Spec {
    pub id: String,
    pub name: String,
    /// The program as typed: claude, codex, opencode, powershell.exe.
    pub cli: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: String,
    /// The CLI's own conversation id, for resuming it.
    #[serde(default)]
    pub session_id: Option<String>,
    pub started_ms: u64,
    /// Its job's first line, the queue's and the header's headline.
    #[serde(default)]
    pub title: Option<String>,
    /// Two or three agents on the same job: which one, of how many.
    #[serde(default)]
    pub race: Option<Race>,
    /// Its own worktree, where it runs, when it has one; `cwd` stays the project.
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

impl Spec {
    /// The folder its CLI runs in: its worktree, else the project.
    pub fn run_dir(&self) -> &str {
        self.worktree.as_deref().unwrap_or(&self.cwd)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Race {
    pub id: String,
    pub n: usize,
    pub of: usize,
}

pub struct Seat {
    pub spec: Spec,
    pub terminal: Option<Entity<Terminal>>,
    /// The git branch of its folder, read when it starts; none outside a repo.
    pub branch: Option<String>,
    /// The Claude process this run is, once found (follow.rs).
    pub pid: Option<u32>,
    /// Its worktree is being made; it starts once that is done.
    pub preparing: bool,
}

impl Seat {
    /// What the agent is doing, read from facts the way today's app does
    /// (supervise::task_state): asking you, blocked, working, idle, stopped.
    pub fn status(&self, cx: &App) -> TaskStatus {
        let facts = match &self.terminal {
            None if self.preparing => TaskFacts { running: true, last_output_at: now_ms(), ..TaskFacts::default() },
            Some(t) if t.read(cx).exit_code().is_none() => {
                let t = t.read(cx);
                TaskFacts { running: true, last_output_at: t.last_output_ms(), screen: t.screen_text(), ..TaskFacts::default() }
            }
            _ => TaskFacts::default(),
        };
        derive_task_state(&facts, now_ms())
    }

    pub fn state(&self, cx: &App) -> TaskState {
        self.status(cx).state
    }

    /// The folder's last part: the project the queue groups it under.
    /// Program and args as typed, for working out its logo.
    pub fn command(&self) -> String {
        std::iter::once(self.spec.cli.as_str()).chain(self.spec.args.iter().map(String::as_str)).collect::<Vec<_>>().join(" ")
    }

    pub fn project(&self) -> String {
        project_of(&self.spec.cwd)
    }
}

pub fn project_of(cwd: &str) -> String {
    cwd.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or(cwd).to_string()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Where the native app keeps its files: ~/.bytehive/native, or
/// BYTEHIVE_NATIVE_DIR so a test run never touches the owner's fleet.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("BYTEHIVE_NATIVE_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var("USERPROFILE").ok()?;
    Some(PathBuf::from(home).join(".bytehive").join("native"))
}

fn store() -> Option<PathBuf> {
    Some(data_dir()?.join("session.json"))
}

#[derive(Default)]
pub struct Fleet {
    pub seats: Vec<Seat>,
    pub focused: Option<usize>,
    /// Conversation ids learned after boot: (agent id, session id).
    learned: Arc<Mutex<Vec<(String, String)>>>,
}

impl Fleet {
    /// The saved fleet, every agent stopped.
    pub fn load() -> Self {
        let specs: Vec<Spec> = store().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let seats: Vec<Seat> = specs.into_iter().map(|spec| Seat { branch: agent_core::git::current_branch(std::path::Path::new(spec.run_dir())), spec, terminal: None, pid: None, preparing: false }).collect();
        let focused = if seats.is_empty() { None } else { Some(0) };
        Self { seats, focused, learned: Arc::default() }
    }

    pub fn save(&self) {
        let Some(path) = store() else { return };
        let specs: Vec<&Spec> = self.seats.iter().map(|s| &s.spec).collect();
        if let (Some(dir), Ok(json)) = (path.parent(), serde_json::to_vec_pretty(&specs)) {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(path, json);
        }
    }

    pub fn focused_seat(&self) -> Option<&Seat> {
        self.focused.and_then(|i| self.seats.get(i))
    }

    /// A name no other agent here carries.
    pub fn free_name(&self) -> String {
        NAMES.iter().find(|n| !self.seats.iter().any(|s| s.spec.name == **n)).map(|n| n.to_string()).unwrap_or_else(|| format!("Agent {}", self.seats.len() + 1))
    }

    /// Add a new agent and start it.
    pub fn add(&mut self, cli: &str, args: Vec<String>, cwd: &str, title: Option<String>, race: Option<Race>, cx: &mut App) -> anyhow::Result<usize> {
        let index = self.reserve(cli, args, cwd, title, race);
        self.seats[index].preparing = false;
        self.start(index, cx)?;
        Ok(index)
    }

    /// Add a new agent that starts later, once its worktree is made.
    pub fn reserve(&mut self, cli: &str, args: Vec<String>, cwd: &str, title: Option<String>, race: Option<Race>) -> usize {
        let spec = Spec {
            id: uuid::Uuid::new_v4().to_string(),
            name: self.free_name(),
            cli: cli.to_string(),
            args,
            cwd: cwd.to_string(),
            session_id: None,
            started_ms: now_ms(),
            title,
            race,
            worktree: None,
            branch: None,
        };
        self.seats.push(Seat { spec, terminal: None, branch: None, pid: None, preparing: true });
        self.save();
        self.seats.len() - 1
    }

    /// Start the agent at `index`, carrying on its conversation when the CLI
    /// still has it.
    pub fn start(&mut self, index: usize, cx: &mut App) -> anyhow::Result<()> {
        let seat = &mut self.seats[index];
        let program = seat.spec.cli.to_ascii_lowercase();
        seat.preparing = false;
        let cwd = seat.spec.run_dir().to_string();
        let base = seat.spec.args.clone();
        let known = seat.spec.session_id.clone();
        let (extra, session) = match program.as_str() {
            "claude" => launch::claude_session_args(&base, known, &cwd, claude::session_exists, || uuid::Uuid::new_v4().to_string()),
            "codex" => match codex::store_root() {
                Some(root) => launch::codex_session_args(&base, known, |id| codex::session_exists_in(&root, id)),
                None => (Vec::new(), None),
            },
            "opencode" => match opencode::db_path() {
                Some(db) => launch::opencode_session_args(&base, known, |id| opencode::session_exists_in(&db, id)),
                None => (Vec::new(), None),
            },
            _ => (Vec::new(), None),
        };
        let mut args = base;
        args.extend(extra);
        let mut spec = launch::launch_spec(&seat.spec.cli, &args);
        spec.cwd = Some(cwd.clone());
        let terminal = Terminal::spawn(spec, cx)?;
        seat.terminal = Some(terminal);
        seat.pid = None;
        seat.branch = agent_core::git::current_branch(std::path::Path::new(&cwd));
        seat.spec.session_id = session.or(seat.spec.session_id.take());
        let started = now_ms();
        seat.spec.started_ms = started;
        self.save();
        // Codex and OpenCode choose their own id at boot: learn it from their
        // stores once the session exists, so Resume finds it later.
        if seat_needs_capture(&program, &self.seats[index].spec) {
            let id = self.seats[index].spec.id.clone();
            let learned = self.learned.clone();
            cx.spawn(async move |cx| {
                for wait in [8u64, 20, 45] {
                    cx.background_executor().timer(Duration::from_secs(wait)).await;
                    let found = match program.as_str() {
                        "codex" => codex::sessions_root().and_then(|root| codex::latest_session_in(&root, &cwd, started)),
                        _ => opencode::db_path().and_then(|db| opencode::latest_session_in(&db, &cwd, started)),
                    };
                    if let Some(session) = found {
                        learned.lock().unwrap().push((id, session));
                        break;
                    }
                }
            })
            .detach();
        }
        Ok(())
    }

    /// Keep the conversation ids the CLIs chose since last time. True if any.
    pub fn absorb(&mut self) -> bool {
        let found: Vec<_> = self.learned.lock().unwrap().drain(..).collect();
        for (id, session) in &found {
            if let Some(seat) = self.seats.iter_mut().find(|s| &s.spec.id == id) {
                seat.spec.session_id = Some(session.clone());
            }
        }
        if !found.is_empty() {
            self.save();
        }
        !found.is_empty()
    }

    /// The running Claude agents, by index, ready for follow.rs.
    pub fn claude_runs(&self, cx: &App) -> Vec<usize> {
        let running = |s: &Seat| s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
        (0..self.seats.len()).filter(|&i| self.seats[i].spec.cli.eq_ignore_ascii_case("claude") && running(&self.seats[i])).collect()
    }

    /// Move each running Claude agent's session id to the one its process is
    /// in now, and save. True if any moved.
    pub fn follow(&mut self, live: &[claude::LiveSession], cx: &App) -> bool {
        let at = self.claude_runs(cx);
        let runs: Vec<follow::Run> = at
            .iter()
            .map(|&i| {
                let s = &self.seats[i];
                follow::Run { session: s.spec.session_id.as_deref(), cwd: s.spec.run_dir(), started_ms: s.spec.started_ms, pid: s.pid }
            })
            .collect();
        let found = follow::follow(&runs, live);
        let mut moved = false;
        for (i, hit) in at.into_iter().zip(found) {
            let Some((pid, session)) = hit else { continue };
            let seat = &mut self.seats[i];
            seat.pid = Some(pid);
            if seat.spec.session_id.as_deref() != Some(session.as_str()) {
                seat.spec.session_id = Some(session);
                moved = true;
            }
        }
        if moved {
            self.save();
        }
        moved
    }

    /// Take the agent off the list: it stops, its spec is forgotten. Its
    /// branch and its changes stay in git.
    pub fn remove(&mut self, index: usize) {
        if index < self.seats.len() {
            self.seats.remove(index);
        }
        self.focused = match self.focused {
            _ if self.seats.is_empty() => None,
            Some(f) if f > index => Some(f - 1),
            Some(f) => Some(f.min(self.seats.len() - 1)),
            None => None,
        };
        self.save();
    }

    /// Stop the agent: its whole process tree ends, its spec stays.
    pub fn stop(&mut self, index: usize) {
        if let Some(seat) = self.seats.get_mut(index) {
            seat.terminal = None;
        }
        self.save();
    }

    /// Show the agent at `index` on the stage: it takes the keyboard and keeps
    /// full history; the rest keep the short one, as today's app does.
    pub fn focus(&mut self, index: usize, window: &mut Window, cx: &mut App) {
        self.focused = Some(index);
        for (i, seat) in self.seats.iter().enumerate() {
            if let Some(t) = &seat.terminal {
                t.read(cx).keep_lines(if i == index { SCROLLBACK } else { SCROLLBACK_HIDDEN });
            }
        }
        if let Some(t) = &self.seats[index].terminal {
            t.read(cx).claim_keys();
            let handle = t.read(cx).focus_handle();
            handle.focus(window, cx);
        }
    }
}

fn seat_needs_capture(program: &str, spec: &Spec) -> bool {
    matches!(program, "codex" | "opencode") && spec.session_id.is_none()
}
