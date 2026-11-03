//! What an agent row says beyond a persona's name: the conversation's title
//! as its own CLI named it, and what it is doing right now (its latest tool
//! call: "Editing grid.rs", "Running cargo test"). Read from the CLIs' own
//! files on a worker thread every few seconds; no model is asked anything.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::sessions_ui::Cli;
use crate::term_pane::Pane;

const EVERY: Duration = Duration::from_secs(3);
/// How much of a transcript's end is read for the latest tool call.
const TAIL: u64 = 96 * 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Brief {
    /// The conversation's title, as the CLI (or you, by renaming it there) named it.
    pub title: Option<String>,
    /// The latest thing it did, in a few words.
    pub doing: Option<String>,
}

/// One pane to read, taken on the UI thread.
struct Ask {
    id: String,
    cli: Option<Cli>,
    session: Option<String>,
    cwd: String,
    /// The terminal title the program set, for CLIs without files to read.
    term_title: Option<String>,
}

pub struct Briefs {
    map: HashMap<String, Brief>,
    tx: Sender<Vec<(String, Brief)>>,
    rx: Receiver<Vec<(String, Brief)>>,
    busy: bool,
    last: Option<Instant>,
}

impl Default for Briefs {
    fn default() -> Self {
        let (tx, rx) = channel();
        Briefs { map: HashMap::new(), tx, rx, busy: false, last: None }
    }
}

impl Briefs {
    pub fn get(&self, pane_id: &str) -> Option<&Brief> {
        self.map.get(pane_id)
    }

    /// Take finished reads; start the next round when due.
    pub fn poll(&mut self, panes: &[Pane]) {
        while let Ok(got) = self.rx.try_recv() {
            self.busy = false;
            self.map = got.into_iter().collect();
        }
        if self.busy || self.last.is_some_and(|t| t.elapsed() < EVERY) || cfg!(test) {
            return;
        }
        self.last = Some(Instant::now());
        let asks: Vec<Ask> = panes
            .iter()
            .filter(|p| p.cli.is_some() || p.custom.is_some())
            .map(|p| Ask { id: p.id.clone(), cli: p.cli, session: p.session_id.clone(), cwd: crate::states::pane_cwd(p), term_title: p.term_title.clone() })
            .collect();
        if asks.is_empty() {
            self.map.clear();
            return;
        }
        self.busy = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let got = asks.into_iter().map(|a| (a.id.clone(), read(&a))).collect();
            let _ = tx.send(got);
        });
    }
}

fn read(a: &Ask) -> Brief {
    let mut b = match (a.cli, a.session.as_deref()) {
        (Some(Cli::Claude), Some(sid)) => claude(&a.cwd, sid),
        (Some(Cli::Codex), Some(sid)) => codex(sid),
        (Some(Cli::Opencode), Some(sid)) => match agent_core::opencode::db_path() {
            Some(db) => {
                let (title, tool) = agent_core::opencode::brief_in(&db, sid);
                Brief { title, doing: tool.and_then(|(n, i)| describe(&n, &i)) }
            }
            None => Brief::default(),
        },
        _ => Brief::default(),
    };
    // A CLI with no files to read (a custom command, Muse) still names its
    // work in the terminal title. Claude's title is the conversation's name,
    // not what it does, so it is never taken for that.
    if b.doing.is_none() && !matches!(a.cli, Some(Cli::Claude | Cli::Codex | Cli::Opencode)) {
        b.doing = a.term_title.as_deref().and_then(clean_title).filter(|d| b.title.as_deref() != Some(d.as_str()));
    }
    b
}

/// The start and the end of a file (titles sit near the head, the latest
/// tool call near the end).
fn head_and_tail(path: &Path) -> Option<(String, String)> {
    let mut f = std::fs::File::open(path).ok()?;
    let size = f.metadata().ok()?.len();
    let mut head = vec![0u8; size.min(32 * 1024) as usize];
    f.read_exact(&mut head).ok()?;
    let from = size.saturating_sub(TAIL);
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut tail = Vec::new();
    f.read_to_end(&mut tail).ok()?;
    Some((String::from_utf8_lossy(&head).into_owned(), String::from_utf8_lossy(&tail).into_owned()))
}

/// Claude Code writes `ai-title` (its own name for the conversation) and
/// `custom-title` (a /rename) lines; a rename wins.
fn claude(cwd: &str, sid: &str) -> Brief {
    let Some(dir) = agent_core::claude::project_dir(cwd) else { return Brief::default() };
    let Some((head, tail)) = head_and_tail(&dir.join(format!("{sid}.jsonl"))) else { return Brief::default() };
    claude_brief(&head, &tail)
}

pub fn claude_brief(head: &str, tail: &str) -> Brief {
    let (mut ai, mut custom) = (None, None);
    for line in head.lines().chain(tail.lines()).filter(|l| l.contains("-title\"")) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        match v["type"].as_str() {
            Some("ai-title") => ai = v["aiTitle"].as_str().map(String::from),
            Some("custom-title") => custom = v["customTitle"].as_str().map(String::from),
            _ => {}
        }
    }
    let doing = tail.lines().rev().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|v| v["type"] == "assistant").and_then(|v| {
        let parts = v["message"]["content"].as_array()?.clone();
        let tool = parts.iter().rev().find(|c| c["type"] == "tool_use")?;
        describe(tool["name"].as_str()?, &tool["input"])
    });
    Brief { title: custom.or(ai).filter(|t| !t.trim().is_empty()), doing }
}

/// Codex keeps thread names in ~/.codex/session_index.jsonl and its calls in
/// the session's rollout file.
fn codex(sid: &str) -> Brief {
    let Some(store) = agent_core::codex::store_root() else { return Brief::default() };
    let index = std::fs::read_to_string(store.join("session_index.jsonl")).unwrap_or_default();
    let title = codex_title(&index, sid);
    let doing = agent_core::codex::sessions_root().and_then(|root| rollout_of(&root, sid)).and_then(|p| head_and_tail(&p)).and_then(|(_, tail)| codex_doing(&tail));
    Brief { title, doing }
}

pub fn codex_title(index: &str, sid: &str) -> Option<String> {
    index.lines().filter(|l| l.contains(sid)).filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|v| v["id"] == sid).filter_map(|v| v["thread_name"].as_str().map(String::from)).last()
}

pub fn codex_doing(tail: &str) -> Option<String> {
    tail.lines().rev().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find_map(|v| {
        let p = &v["payload"];
        if v["type"] != "response_item" || !matches!(p["type"].as_str(), Some("function_call" | "custom_tool_call" | "local_shell_call")) {
            return None;
        }
        let input = match &p["arguments"] {
            Value::String(s) => serde_json::from_str(s).unwrap_or(Value::String(s.clone())),
            other if !other.is_null() => other.clone(),
            _ => p["input"].clone(),
        };
        describe(p["name"].as_str().unwrap_or("shell"), &input)
    })
}

/// The newest rollout file for `sid`: day folders are searched newest first.
fn rollout_of(root: &Path, sid: &str) -> Option<std::path::PathBuf> {
    for year in agent_core::codex::sorted_dirs(root).into_iter().rev() {
        for month in agent_core::codex::sorted_dirs(&year).into_iter().rev() {
            for day in agent_core::codex::sorted_dirs(&month).into_iter().rev() {
                let hit = std::fs::read_dir(&day).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(&format!("{sid}.jsonl"))));
                if hit.is_some() {
                    return hit;
                }
            }
        }
    }
    None
}

/// A tool call in a few words.
pub fn describe(name: &str, input: &Value) -> Option<String> {
    let s = |k: &str| input[k].as_str().map(str::trim).filter(|v| !v.is_empty());
    let file = |k: &str| s(k).map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string());
    let cmd = || match &input["command"] {
        Value::Array(a) => Some(a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")),
        Value::String(c) => Some(c.clone()),
        _ => s("cmd").map(String::from),
    };
    let short = |t: String| {
        let line = t.lines().next().unwrap_or("").trim().to_string();
        if line.chars().count() > 60 { format!("{}\u{2026}", line.chars().take(59).collect::<String>()) } else { line }
    };
    let n = name.rsplit("__").next().unwrap_or(name).to_lowercase();
    let what = match n.as_str() {
        "bash" | "shell" | "exec_command" | "powershell" | "local_shell" => format!("Running {}", short(cmd()?)),
        "edit" | "multiedit" | "write" | "notebookedit" => format!("Editing {}", file("file_path").or_else(|| file("filePath")).or_else(|| file("path"))?),
        "apply_patch" | "patch" => "Editing files".to_string(),
        "read" | "view" => format!("Reading {}", file("file_path").or_else(|| file("filePath")).or_else(|| file("path"))?),
        "grep" | "glob" | "search" | "list" | "ls" => format!("Searching {}", short(s("pattern").or_else(|| s("path")).unwrap_or("the code").to_string())),
        "webfetch" | "websearch" | "web_search" | "fetch" => "Searching the web".to_string(),
        "task" | "agent" => format!("Running a subagent{}", s("description").map(|d| format!(": {}", short(d.to_string()))).unwrap_or_default()),
        "todowrite" | "update_plan" | "todo" => "Updating its plan".to_string(),
        "askuserquestion" => "Asking you something".to_string(),
        _ => format!("Using {}", name.rsplit("__").next().unwrap_or(name)),
    };
    Some(what)
}

/// A terminal title as a line: spinner glyphs and paths dropped.
pub fn clean_title(t: &str) -> Option<String> {
    let t = t.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
    let boring = t.is_empty() || t.contains('\\') || t.contains('/') || t.ends_with(".exe") || t.eq_ignore_ascii_case("claude code");
    (!boring).then(|| t.chars().take(60).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claude_titles_and_last_tool() {
        let head = r#"{"type":"ai-title","aiTitle":"Fix the grid gaps","sessionId":"s"}"#;
        let tail = [
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"ok"},{"type":"tool_use","name":"Edit","input":{"file_path":"D:\\m\\grid.rs"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#,
        ]
        .join("\n");
        let b = claude_brief(head, &tail);
        assert_eq!(b.title.as_deref(), Some("Fix the grid gaps"));
        assert_eq!(b.doing.as_deref(), Some("Editing grid.rs"));
        let renamed = format!("{head}\n{}", r#"{"type":"custom-title","customTitle":"Grid polish"}"#);
        assert_eq!(claude_brief(&renamed, "").title.as_deref(), Some("Grid polish"));
    }

    #[test]
    fn codex_names_and_calls() {
        let index = "{\"id\":\"a1\",\"thread_name\":\"Old\"}\n{\"id\":\"a1\",\"thread_name\":\"Audit fit\"}\n{\"id\":\"b2\",\"thread_name\":\"Other\"}";
        assert_eq!(codex_title(index, "a1").as_deref(), Some("Audit fit"));
        let tail = r#"{"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"cargo\",\"test\"]}"}}"#;
        assert_eq!(codex_doing(tail).as_deref(), Some("Running cargo test"));
    }

    #[test]
    fn tool_calls_read_in_a_few_words() {
        assert_eq!(describe("Bash", &json!({"command": "npm run build\nnext"})).as_deref(), Some("Running npm run build"));
        assert_eq!(describe("mcp__bytehive__board_get", &json!({})).as_deref(), Some("Using board_get"));
        assert_eq!(describe("read", &json!({"filePath": "src/a.ts"})).as_deref(), Some("Reading a.ts"));
        assert_eq!(clean_title("\u{25D0} Fixing login"), Some("Fixing login".into()));
        assert_eq!(clean_title(r"C:\Windows\system32\cmd.exe"), None);
    }
}
