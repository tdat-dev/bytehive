//! What an agent has used so far, read from where its CLI keeps it, ported
//! from the desktop app's `claude_session_usage` and `agent_usage` commands:
//! Claude's session transcript (tokens per model, read on from where it was
//! last read, each reply counted once), Codex's rollout (tokens, context
//! fill) and opencode's database (its own priced cost). All local and cheap.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;

use crate::{claude, codex, opencode};

/// Token usage for one model.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ModelUsage {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub messages: u64,
}

impl ModelUsage {
    pub fn tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_creation + self.cache_read
    }
}

/// What a non-Claude CLI says it used.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct AgentUsage {
    /// What it cost, when the CLI says (opencode prices its own replies).
    pub usd: Option<f64>,
    pub tokens: Option<u64>,
    /// How full its context is, 0 to 100.
    pub context_pct: Option<f64>,
    pub model: Option<String>,
}

/// One transcript's usage so far and how far it has been read. Transcripts
/// only grow, so each read takes just the lines added since the last.
#[derive(Default)]
struct FileUsage {
    read: u64,
    /// Claude writes one reply over several lines, each with the reply's
    /// whole usage: a reply counts once, by its id.
    recent: VecDeque<String>,
    by_model: HashMap<String, ModelUsage>,
}

const RECENT_IDS: usize = 32;

impl FileUsage {
    fn add_line(&mut self, line: &str) {
        if !line.contains("\"usage\"") {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        if v.get("type").and_then(Value::as_str) != Some("assistant") {
            return;
        }
        let Some(msg) = v.get("message") else { return };
        let Some(usage) = msg.get("usage") else { return };
        if let Some(id) = msg.get("id").and_then(Value::as_str) {
            if self.recent.iter().any(|r| r == id) {
                return;
            }
            if self.recent.len() == RECENT_IDS {
                self.recent.pop_front();
            }
            self.recent.push_back(id.to_string());
        }
        let model = msg.get("model").and_then(Value::as_str).unwrap_or("unknown");
        if model == "<synthetic>" {
            return;
        }
        let get = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
        let e = self.by_model.entry(model.to_string()).or_insert_with(|| ModelUsage { model: model.to_string(), ..Default::default() });
        e.input_tokens += get("input_tokens");
        e.output_tokens += get("output_tokens");
        e.cache_creation += get("cache_creation_input_tokens");
        e.cache_read += get("cache_read_input_tokens");
        e.messages += 1;
    }

    /// Read whole lines added since last time; a line still being written waits.
    fn catch_up(&mut self, path: &Path) {
        let Ok(file) = std::fs::File::open(path) else { return };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.read {
            *self = FileUsage::default(); // rewritten: start over
        }
        if len == self.read {
            return;
        }
        let mut r = std::io::BufReader::with_capacity(256 * 1024, file);
        if r.seek(SeekFrom::Start(self.read)).is_err() {
            return;
        }
        let mut buf = Vec::with_capacity(64 * 1024);
        loop {
            buf.clear();
            let n = r.read_until(b'\n', &mut buf).unwrap_or(0);
            if n == 0 || buf.last() != Some(&b'\n') {
                break;
            }
            self.read += n as u64;
            if let Ok(line) = std::str::from_utf8(&buf) {
                self.add_line(line);
            }
        }
    }
}

fn cache() -> &'static Mutex<HashMap<PathBuf, FileUsage>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, FileUsage>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// One transcript's usage by model, biggest first.
pub fn file_usage(path: &Path) -> Vec<ModelUsage> {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    let entry = cache.entry(path.to_path_buf()).or_default();
    entry.catch_up(path);
    let mut out: Vec<ModelUsage> = entry.by_model.values().cloned().collect();
    out.sort_by_key(|u| std::cmp::Reverse(u.input_tokens + u.output_tokens));
    out
}

fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Token usage of one Claude session (one agent's conversation), by model.
pub fn claude_session_usage(dir: &str, session_id: &str) -> Vec<ModelUsage> {
    if !is_session_id(session_id) {
        return Vec::new();
    }
    claude::project_dir(dir).map(|p| file_usage(&p.join(format!("{session_id}.jsonl")))).unwrap_or_default()
}

/// The last `bytes` of a file as whole lines (the first, cut, line dropped).
pub(crate) fn tail_lines(path: &Path, bytes: u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else { return Vec::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(bytes);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let mut lines: Vec<String> = String::from_utf8_lossy(&buf).lines().map(str::to_string).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    lines
}

/// Codex: its rollout's newest `token_count` has the conversation's tokens,
/// the last request's size and the model's window.
pub fn codex_usage(dir: &str, since_ms: u64) -> Option<AgentUsage> {
    let root = codex::sessions_root()?;
    let path = codex::find_rollout(&root, dir, since_ms)?;
    Some(codex_usage_in(&tail_lines(&path, 1024 * 1024)))
}

pub fn codex_usage_in(lines: &[String]) -> AgentUsage {
    let mut out = AgentUsage::default();
    for line in lines.iter().rev() {
        if out.tokens.is_none() && line.contains("\"token_count\"") {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(info) = v.pointer("/payload/info").filter(|i| !i.is_null()) else { continue };
            out.tokens = info.pointer("/total_token_usage/total_tokens").and_then(Value::as_u64);
            let last = info.pointer("/last_token_usage/input_tokens").and_then(Value::as_f64);
            let window = info.get("model_context_window").and_then(Value::as_f64).filter(|w| *w > 0.0);
            if let (Some(l), Some(w)) = (last, window) {
                out.context_pct = Some((l / w * 100.0).min(100.0).round());
            }
        }
        if out.model.is_none() && line.contains("\"turn_context\"") {
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                out.model = v.pointer("/payload/model").and_then(Value::as_str).map(str::to_string);
            }
        }
        if out.tokens.is_some() && out.model.is_some() {
            break;
        }
    }
    out
}

/// opencode prices each reply itself: the sum over its session and the
/// helper sessions it started.
pub fn opencode_usage(dir: &str, since_ms: u64) -> Option<AgentUsage> {
    let db = opencode::db_path()?;
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).ok()?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(1500));
    let id = opencode::newest_session(&conn, dir, Some(since_ms))?;
    let (usd, tokens): (f64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(json_extract(data, '$.cost')), 0),
                    COALESCE(SUM(COALESCE(json_extract(data, '$.tokens.input'), 0) + COALESCE(json_extract(data, '$.tokens.output'), 0)
                               + COALESCE(json_extract(data, '$.tokens.reasoning'), 0) + COALESCE(json_extract(data, '$.tokens.cache.read'), 0)), 0)
             FROM message
             WHERE session_id IN (SELECT id FROM session WHERE id = ?1 OR parent_id = ?1)
               AND json_extract(data, '$.role') = 'assistant'",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok()?;
    Some(AgentUsage { usd: Some(usd), tokens: Some(tokens.max(0) as u64), context_pct: None, model: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(id: &str, model: &str) -> String {
        format!("{{\"type\":\"assistant\",\"message\":{{\"id\":\"{id}\",\"model\":\"{model}\",\"usage\":{{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":100,\"cache_creation_input_tokens\":20}},\"content\":[]}}}}\n")
    }

    #[test]
    fn counts_each_reply_once_and_reads_on() {
        let path = std::env::temp_dir().join(format!("bytehive-usage-{}.jsonl", std::process::id()));
        // One reply over two lines, then a second reply.
        std::fs::write(&path, format!("{}{}{}", reply("a", "claude-opus-5-5"), reply("a", "claude-opus-5-5"), reply("b", "claude-opus-5-5"))).unwrap();
        let first = file_usage(&path);
        assert_eq!((first[0].messages, first[0].output_tokens, first[0].cache_read), (2, 10, 200));
        // Appended later: only the new line is read.
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        std::io::Write::write_all(&mut f, reply("c", "claude-haiku-4-5").as_bytes()).unwrap();
        let next = file_usage(&path);
        assert_eq!(next.len(), 2);
        assert_eq!(next.iter().map(|u| u.messages).sum::<u64>(), 3);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reads_codex_tokens_and_context() {
        let lines = vec![
            r#"{"type":"turn_context","payload":{"model":"gpt-5.6"}}"#.to_string(),
            r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":5000},"last_token_usage":{"input_tokens":64000},"model_context_window":128000}}}"#.to_string(),
        ];
        let u = codex_usage_in(&lines);
        assert_eq!(u.tokens, Some(5000));
        assert_eq!(u.context_pct, Some(50.0));
        assert_eq!(u.model.as_deref(), Some("gpt-5.6"));
    }

    #[test]
    fn a_bad_session_id_reads_nothing() {
        assert!(claude_session_usage("D:/x", "../../etc").is_empty());
    }
}
