//! The app side of the fleet file bridge (the bytehive MCP server is the other
//! side, and only writes files): every 1.5 s, for each open project,
//!  - publish its agents to `.bytehive/fleet.json` (fleet_status, agent_output
//!    and agent_context read it),
//!  - deliver each new line of `.bytehive/outbox.jsonl` (fleet_send) into the
//!    agent it names, or to every agent but the sender,
//!  - boot the agents asked for in `.bytehive/spawn-requests.jsonl` (agent_spawn).
//! The files are read on from where the last read stopped; a file seen for the
//! first time is skipped to its end so a restart doesn't replay old messages.
//! Port of the web app's fleetbridge.ts and its host in bridges.ts.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::projects::{normalize, pane_home, project_name};
use crate::sessions_ui::Cli;
use crate::states::{pane_cwd, pane_status};

const EVERY: Duration = Duration::from_millis(1500);
/// A fully read outbox bigger than this is emptied.
const COMPACT_BYTES: u64 = 1024 * 1024;

/// One fleet_send message.
#[derive(Debug, PartialEq)]
pub struct OutboxLine {
    pub from: Option<String>,
    pub to: Option<String>,
    pub message: String,
}

fn text_of(v: &Value, k: &str) -> Option<String> {
    v[k].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn parse_outbox_line(line: &str) -> Option<OutboxLine> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let message = v["message"].as_str().filter(|m| !m.trim().is_empty())?.to_string();
    Some(OutboxLine { from: text_of(&v, "from"), to: text_of(&v, "to"), message })
}

/// One agent_spawn request.
#[derive(Debug, PartialEq)]
pub struct SpawnLine {
    pub cli: String,
    pub task: Option<String>,
    pub count: usize,
}

pub fn parse_spawn_line(line: &str) -> Option<SpawnLine> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let cli = text_of(&v, "cli")?;
    let count = v["count"].as_f64().map_or(1, |n| (n as i64).clamp(1, 6) as usize);
    Some(SpawnLine { cli, task: text_of(&v, "task"), count })
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Who a message reaches: the agent it names, or, sent to nobody in
/// particular, every running agent but its sender (a Director broadcasting
/// to everyone would otherwise read its own order back). Never a stopped one.
pub fn delivery_targets<'a>(agents: &[(&'a str, &'a str, bool)], from: Option<&str>, to: Option<&str>) -> Vec<&'a str> {
    agents
        .iter()
        .filter(|(_, name, running)| *running && to.map_or_else(|| from.is_none_or(|f| !same_name(name, f)), |t| same_name(name, t)))
        .map(|(id, _, _)| *id)
        .collect()
}

/// The status word the MCP tools use.
pub fn status_word(state: TaskState) -> &'static str {
    match state {
        TaskState::Needs | TaskState::Blocked => "needs",
        TaskState::Working => "active",
        TaskState::Review | TaskState::Idle => "idle",
        TaskState::Stopped | TaskState::Merged => "stopped",
    }
}

/// The new whole lines of an append-only jsonl file since `offset`, and
/// where the next read starts. A missing file starts at 0 (its first line is
/// new); a file seen first skips its backlog; a shrunk file starts over.
pub fn read_new_lines(path: &Path, offset: Option<u64>) -> (Vec<String>, Option<u64>) {
    let Ok(mut f) = std::fs::File::open(path) else { return (Vec::new(), Some(0)) };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let Some(at) = offset else { return (Vec::new(), Some(size)) };
    let at = if at > size { 0 } else { at };
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(at)).is_err() || f.take(4 * 1024 * 1024).read_to_end(&mut buf).is_err() {
        return (Vec::new(), Some(at));
    }
    // A line still being written waits for the next read.
    let whole = buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let lines = String::from_utf8_lossy(&buf[..whole]).lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect();
    let next = at + whole as u64;
    if next == size && size > COMPACT_BYTES {
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(path) {
            if f.metadata().map(|m| m.len()).unwrap_or(0) == size && f.set_len(0).is_ok() {
                return (lines, Some(0));
            }
        }
    }
    (lines, Some(next))
}

#[derive(Default)]
pub struct FleetBridge {
    last: Option<Instant>,
    written: HashMap<String, String>,
    outbox: HashMap<String, Option<u64>>,
    spawns: HashMap<String, Option<u64>>,
}

fn file(dir: &str, name: &str) -> PathBuf {
    Path::new(dir).join(".bytehive").join(name)
}

impl ShellApp {
    /// The agents of project `dir`: (pane index) of those it owns.
    fn fleet_of(&self, dir: &str) -> Vec<usize> {
        let d = normalize(dir);
        (0..self.panes.len()).filter(|&i| self.panes[i].cli.is_some() && self.projects.owner(&pane_home(&self.panes[i])).is_some_and(|o| normalize(o) == d)).collect()
    }

    fn roster(&self, dir: &str) -> String {
        let now = now_ms();
        let agents: Vec<Value> = self
            .fleet_of(dir)
            .into_iter()
            .map(|i| {
                let p = &self.panes[i];
                let changed = self.git_changed.get(&pane_cwd(p)).map(|f| f.len() as u32);
                let screen = p.screen_text();
                let tail: String = screen.chars().rev().take(2000).collect::<Vec<_>>().into_iter().rev().collect();
                let mut a = json!({ "id": p.id, "name": p.name, "status": status_word(pane_status(p, now, changed).state), "workspace": project_name(dir), "screen": tail, "folder": pane_cwd(p), "since": p.started_ms });
                if let Some(cli) = p.cli {
                    a["cli"] = json!(cli.program());
                }
                if let Some(s) = &p.session_id {
                    a["session"] = json!(s);
                }
                a
            })
            .collect();
        json!({ "agents": agents }).to_string()
    }

    /// Publish, deliver, spawn (logic pass).
    pub(crate) fn poll_fleet(&mut self) {
        if self.extras.fleet.last.is_some_and(|t| t.elapsed() < EVERY) || cfg!(test) {
            return;
        }
        self.extras.fleet.last = Some(Instant::now());
        for dir in self.projects.dirs.clone() {
            self.fleet_tick(&dir);
        }
    }

    pub(crate) fn fleet_tick(&mut self, dir: &str) {
        let roster = self.roster(dir);
        if self.extras.fleet.written.get(dir) != Some(&roster) {
            let path = file(dir, "fleet.json");
            let ok = path.parent().is_some_and(|p| std::fs::create_dir_all(p).is_ok()) && std::fs::write(&path, &roster).is_ok();
            if ok {
                self.extras.fleet.written.insert(dir.to_string(), roster);
            }
        }
        let at = self.extras.fleet.outbox.get(dir).copied().flatten();
        let seen = self.extras.fleet.outbox.contains_key(dir);
        let (lines, next) = read_new_lines(&file(dir, "outbox.jsonl"), if seen { Some(at.unwrap_or(0)) } else { None });
        self.extras.fleet.outbox.insert(dir.to_string(), next);
        for m in lines.iter().filter_map(|l| parse_outbox_line(l)) {
            let agents: Vec<(String, String, bool)> = self.fleet_of(dir).into_iter().map(|i| (self.panes[i].id.clone(), self.panes[i].name.clone(), self.panes[i].exit_code.is_none())).collect();
            let view: Vec<(&str, &str, bool)> = agents.iter().map(|(a, b, c)| (a.as_str(), b.as_str(), *c)).collect();
            for id in delivery_targets(&view, m.from.as_deref(), m.to.as_deref()) {
                self.tell(id.to_string(), m.message.clone());
            }
        }
        let at = self.extras.fleet.spawns.get(dir).copied().flatten();
        let seen = self.extras.fleet.spawns.contains_key(dir);
        let (lines, next) = read_new_lines(&file(dir, "spawn-requests.jsonl"), if seen { Some(at.unwrap_or(0)) } else { None });
        self.extras.fleet.spawns.insert(dir.to_string(), next);
        for req in lines.iter().filter_map(|l| parse_spawn_line(l)) {
            self.spawn_requested(dir, req);
        }
    }

    /// An agent asked for more agents (agent_spawn): boot them in its project
    /// under persona names and give them the task, if any.
    fn spawn_requested(&mut self, dir: &str, req: SpawnLine) {
        let Some(cli) = Cli::all().into_iter().find(|c| same_name(c.program(), &req.cli) || same_name(c.name(), &req.cli)) else {
            self.notice = Some(format!("An agent asked for {}, which ByteHive can't start.", req.cli));
            return;
        };
        for _ in 0..req.count {
            let taken: Vec<String> = self.panes.iter().map(|p| p.name.clone()).collect();
            let name = crate::newtask::persona(&taken);
            match self.boot_agent(cli, dir, name, self.prefs.perm.args(cli), None) {
                Ok(id) => {
                    if let Some(task) = &req.task {
                        if let Some(p) = self.panes.iter_mut().find(|p| p.id == id) {
                            p.task = task.lines().next().map(|l| l.chars().take(80).collect());
                        }
                        self.deliveries.push(crate::deliver::Delivery::new(id, task.clone(), now_ms(), self.prefs.job_delay_ms()));
                    }
                }
                Err(e) => self.notice = Some(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_messages_and_spawn_requests() {
        assert_eq!(parse_outbox_line(r#"{"from":"Ana","to":" ","message":"go"}"#), Some(OutboxLine { from: Some("Ana".into()), to: None, message: "go".into() }));
        assert_eq!(parse_outbox_line(r#"{"message":"  "}"#), None);
        assert_eq!(parse_outbox_line("not json"), None);
        assert_eq!(parse_spawn_line(r#"{"cli":"codex","count":9,"task":"x"}"#), Some(SpawnLine { cli: "codex".into(), task: Some("x".into()), count: 6 }));
        assert_eq!(parse_spawn_line(r#"{"cli":"claude"}"#).map(|s| s.count), Some(1));
    }

    #[test]
    fn broadcasts_skip_the_sender_and_the_stopped() {
        let a = [("1", "Director", true), ("2", "Ana", true), ("3", "Bob", false)];
        assert_eq!(delivery_targets(&a, Some("director"), None), ["2"]);
        assert_eq!(delivery_targets(&a, None, Some(" ana ")), ["2"]);
        assert!(delivery_targets(&a, None, Some("Bob")).is_empty(), "a stopped agent gets nothing");
        assert_eq!(delivery_targets(&a, None, None), ["1", "2"]);
    }

    #[test]
    fn reads_only_new_whole_lines() {
        let dir = std::env::temp_dir().join(format!("bytehive-fleet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("outbox.jsonl");
        assert_eq!(read_new_lines(&p, None), (vec![], Some(0)), "not there yet: starts at 0");
        std::fs::write(&p, "old\n").unwrap();
        assert_eq!(read_new_lines(&p, None), (vec![], Some(4)), "first sight skips the backlog");
        std::fs::write(&p, "old\nnew\npart").unwrap();
        assert_eq!(read_new_lines(&p, Some(4)), (vec!["new".to_string()], Some(8)), "a half-written line waits");
        std::fs::write(&p, "x\n").unwrap();
        assert_eq!(read_new_lines(&p, Some(8)), (vec!["x".to_string()], Some(2)), "a shrunk file starts over");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn publishes_and_delivers_for_a_project() {
        let dir = std::env::temp_dir().join(format!("bytehive-fleet-app-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".bytehive")).unwrap();
        let d = dir.to_string_lossy().to_string();
        let mut app = ShellApp::new().unwrap();
        app.projects = Default::default();
        app.projects.open(&d);
        app.panes[0].cli = Some(Cli::Claude);
        app.panes[0].name = "Ana".into();
        app.panes[0].spec.cwd = Some(d.clone());
        app.fleet_tick(&d);
        let roster: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".bytehive/fleet.json")).unwrap()).unwrap();
        assert_eq!(roster["agents"][0]["name"], "Ana");
        assert_eq!(roster["agents"][0]["cli"], "claude");
        std::fs::write(dir.join(".bytehive/outbox.jsonl"), "{\"to\":\"ana\",\"message\":\"run the tests\"}\n").unwrap();
        app.fleet_tick(&d);
        assert_eq!(app.deliveries.len(), 1, "the message is on its way to Ana");
        std::fs::remove_dir_all(&dir).ok();
    }
}
