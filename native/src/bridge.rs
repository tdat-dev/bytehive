//! The app's side of the file bridge to bytehive-mcp (fleetbridge.ts). The
//! MCP server never talks to the app, only through files in a project's
//! `.bytehive` folder:
//!  - fleet.json: the roster this app publishes, for fleet_status, agent_output
//!    and agent_context;
//!  - outbox.jsonl: messages fleet_send appends, typed into the agents named;
//!  - spawn-requests.jsonl: agents agent_spawn asks for.
//! The files stay byte compatible with what the Tauri app reads and writes.

use std::collections::HashMap;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// A fully read outbox bigger than this is emptied.
pub const COMPACT_BYTES: u64 = 1024 * 1024;
/// Read at most this much of a file in one tick; the rest waits.
const SLICE_BYTES: u64 = 256 * 1024;

/// One agent as fleet.json lists it.
pub struct BridgeAgent {
    pub id: String,
    pub name: String,
    /// needs, active, idle or stopped.
    pub status: &'static str,
    pub screen: String,
    pub cli: String,
    pub session: Option<String>,
    pub folder: String,
    pub since: Option<u64>,
}

/// The roster JSON the MCP tools read (serializeFleet).
pub fn serialize_fleet(workspace: &str, agents: &[BridgeAgent]) -> String {
    let list: Vec<Value> = agents
        .iter()
        .map(|a| {
            let mut o = json!({ "id": a.id, "name": a.name, "status": a.status, "workspace": workspace, "screen": a.screen });
            let m = o.as_object_mut().expect("object");
            if !a.cli.is_empty() {
                m.insert("cli".into(), json!(a.cli));
            }
            if let Some(s) = a.session.as_ref().filter(|s| !s.is_empty()) {
                m.insert("session".into(), json!(s));
            }
            if !a.folder.is_empty() {
                m.insert("folder".into(), json!(a.folder));
            }
            if let Some(t) = a.since.filter(|t| *t > 0) {
                m.insert("since".into(), json!(t));
            }
            o
        })
        .collect();
    json!({ "agents": list }).to_string()
}

#[derive(Debug, PartialEq)]
pub struct OutboxLine {
    pub from: Option<String>,
    pub to: Option<String>,
    pub message: String,
}

fn text_of(o: &Value, key: &str) -> Option<String> {
    o.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string)
}

/// One outbox line as a message to deliver, or none when it is unusable.
pub fn parse_outbox_line(line: &str) -> Option<OutboxLine> {
    let o: Value = serde_json::from_str(line.trim()).ok()?;
    Some(OutboxLine { message: text_of(&o, "message")?, from: text_of(&o, "from"), to: text_of(&o, "to") })
}

#[derive(Debug, PartialEq)]
pub struct SpawnLine {
    pub cli: String,
    pub task: Option<String>,
    pub count: usize,
}

pub fn parse_spawn_line(line: &str) -> Option<SpawnLine> {
    let o: Value = serde_json::from_str(line.trim()).ok()?;
    let count = o.get("count").and_then(Value::as_f64).map_or(1, |n| n.clamp(1.0, 6.0) as usize);
    Some(SpawnLine { cli: text_of(&o, "cli")?, task: text_of(&o, "task"), count })
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Who a line reaches: the agent named, loosely, or for a broadcast every
/// running agent but the sender (a Director dispatching to everyone would
/// otherwise read its own order back and loop). Stopped agents never.
pub fn delivery_targets(agents: &[(String, bool)], from: Option<&str>, to: Option<&str>) -> Vec<usize> {
    (0..agents.len())
        .filter(|&i| {
            let (name, running) = (&agents[i].0, agents[i].1);
            running
                && match to {
                    Some(to) => same_name(name, to),
                    None => from.is_none_or(|f| !same_name(name, f)),
                }
        })
        .collect()
}

pub fn bytehive_dir(project: &str) -> PathBuf {
    Path::new(project).join(".bytehive")
}

/// What the bridge remembers between ticks: how far each file was read and
/// the roster last written, so an unchanged one is not written again.
#[derive(Default)]
pub struct BridgeFiles {
    offsets: HashMap<PathBuf, u64>,
    last_fleet: HashMap<PathBuf, String>,
    /// Projects whose `.bytehive/` was kept out of git status this run.
    excluded: std::collections::HashSet<String>,
}

impl BridgeFiles {
    /// Write a project's roster when it changed.
    pub fn publish(&mut self, project: &str, content: String) {
        let path = bytehive_dir(project).join("fleet.json");
        if self.last_fleet.get(&path) == Some(&content) {
            return;
        }
        let _ = std::fs::create_dir_all(bytehive_dir(project));
        if !self.excluded.contains(project) && crate::gitexclude::keep_out(project).is_ok() {
            self.excluded.insert(project.to_string());
        }
        if std::fs::write(&path, &content).is_ok() {
            self.last_fleet.insert(path, content);
        }
    }

    /// The whole new lines of an append-only file since the last read. A file
    /// seen for the first time is skipped to its end, so a restart does not
    /// replay the backlog; one that is not there yet starts at 0, so the line
    /// that creates it is read. A fully read big file is emptied.
    pub fn drain(&mut self, path: &Path) -> Vec<String> {
        let Ok(mut file) = std::fs::File::open(path) else {
            self.offsets.entry(path.to_path_buf()).or_insert(0);
            return Vec::new();
        };
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let Some(&at) = self.offsets.get(path) else {
            self.offsets.insert(path.to_path_buf(), size);
            return Vec::new();
        };
        // Emptied by someone else: start over.
        let at = if at > size { 0 } else { at };
        let mut buf = Vec::new();
        if file.seek(SeekFrom::Start(at)).is_err() || (&mut file).take(SLICE_BYTES).read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        // A line still being written waits for the next tick.
        let whole = buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        let next = at + whole as u64;
        self.offsets.insert(path.to_path_buf(), next);
        let lines = String::from_utf8_lossy(&buf[..whole]).lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect();
        drop(file);
        if next == size && size > COMPACT_BYTES && empty_if_size(path, size) {
            self.offsets.insert(path.to_path_buf(), 0);
        }
        lines
    }
}

/// Empty the file, only if nothing was appended since it was read.
fn empty_if_size(path: &Path, size: u64) -> bool {
    let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) else { return false };
    file.metadata().is_ok_and(|m| m.len() == size) && file.set_len(0).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{BridgeAgent, BridgeFiles, OutboxLine, SpawnLine, delivery_targets, parse_outbox_line, parse_spawn_line, serialize_fleet};

    #[test]
    fn roster_matches_what_the_mcp_reads() {
        let a = BridgeAgent { id: "1".into(), name: "Ana".into(), status: "active", screen: "hi".into(), cli: "claude".into(), session: Some("s".into()), folder: r"D:\app".into(), since: Some(5) };
        let b = BridgeAgent { id: "2".into(), name: "Bob".into(), status: "stopped", screen: String::new(), cli: String::new(), session: None, folder: String::new(), since: None };
        let v: serde_json::Value = serde_json::from_str(&serialize_fleet("app", &[a, b])).unwrap();
        assert_eq!(v["agents"][0]["workspace"], "app");
        assert_eq!(v["agents"][0]["session"], "s");
        assert_eq!(v["agents"][0]["since"], 5);
        assert!(v["agents"][1].get("cli").is_none() && v["agents"][1].get("since").is_none());
    }

    #[test]
    fn reads_outbox_and_spawn_lines() {
        assert_eq!(parse_outbox_line(r#"{"ts":1,"from":"Ana","to":null,"message":"go"}"#), Some(OutboxLine { from: Some("Ana".into()), to: None, message: "go".into() }));
        assert_eq!(parse_outbox_line(r#"{"message":"  "}"#), None);
        assert_eq!(parse_outbox_line("not json"), None);
        assert_eq!(parse_spawn_line(r#"{"cli":"codex","task":"fix it","count":9}"#), Some(SpawnLine { cli: "codex".into(), task: Some("fix it".into()), count: 6 }));
        assert_eq!(parse_spawn_line(r#"{"cli":"claude"}"#).map(|s| s.count), Some(1));
    }

    #[test]
    fn broadcasts_skip_the_sender_and_the_stopped() {
        let fleet = vec![("Ana".to_string(), true), ("Bob".to_string(), true), ("Cid".to_string(), false)];
        assert_eq!(delivery_targets(&fleet, Some("ana"), None), vec![1]);
        assert_eq!(delivery_targets(&fleet, None, Some(" bob ")), vec![1]);
        assert_eq!(delivery_targets(&fleet, None, Some("Cid")), Vec::<usize>::new());
    }

    #[test]
    fn drains_only_new_whole_lines() {
        let dir = std::env::temp_dir().join(format!("bytehive-bridge-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("outbox.jsonl");
        let _ = std::fs::remove_file(&path);
        let mut files = BridgeFiles::default();
        // Not there yet: whatever arrives is new.
        assert!(files.drain(&path).is_empty());
        std::fs::write(&path, "a\nb\npart").unwrap();
        assert_eq!(files.drain(&path), vec!["a", "b"]);
        std::fs::write(&path, "a\nb\npartial\n").unwrap();
        assert_eq!(files.drain(&path), vec!["partial"]);
        // A fresh bridge skips the backlog.
        assert!(BridgeFiles::default().drain(&path).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
