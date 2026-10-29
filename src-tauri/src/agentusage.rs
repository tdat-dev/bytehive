//! What an agent of any CLI has used so far, read from wherever that CLI keeps
//! it: Codex's rollout (tokens, context), opencode's database (its own cost),
//! Gemini's and Qwen's chat files (tokens), grok's session signals (context).
//! Claude has its own reader (claude_session_usage). Everything is read
//! locally and cheaply: the end of a file, one SQL sum, a small JSON; a chat
//! file already read is not read again until it grows.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;

use crate::error::{run_blocking, CommandError};
use agent_core::{codex, opencode};

#[derive(Serialize, Default, Debug, Clone, PartialEq)]
pub struct AgentUsage {
    /// What it cost, when the CLI says (opencode prices its own messages).
    pub usd: Option<f64>,
    /// Tokens it has used in this conversation.
    pub tokens: Option<u64>,
    /// How full its context is, 0–100.
    pub context_pct: Option<f64>,
    pub model: Option<String>,
}

/// An agent's usage so far. `dir` is where it runs, `since_ms` when it
/// started (its own session is the newest begun there since).
#[tauri::command]
pub async fn agent_usage(badge: String, dir: String, since_ms: Option<u64>) -> Result<AgentUsage, CommandError> {
    run_blocking(move || {
        Ok(match badge.as_str() {
            "codex" => codex_usage(&dir, since_ms),
            "opencode" => opencode_usage(&dir, since_ms),
            "gemini" => gemini_usage(".gemini", &dir, since_ms),
            "qwen" => gemini_usage(".qwen", &dir, since_ms),
            "grok" => grok_usage(&dir, since_ms),
            _ => None,
        }
        .unwrap_or_default())
    })
    .await
}

fn home() -> Option<PathBuf> {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok().map(PathBuf::from)
}

fn modified_ms(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
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
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    lines
}

/// Codex: the newest `token_count` event of its rollout has the conversation's
/// total tokens, the last request's size and the model's window.
fn codex_usage(dir: &str, since_ms: Option<u64>) -> Option<AgentUsage> {
    let root = codex::sessions_root()?;
    let path = codex::find_rollout(&root, dir, since_ms.unwrap_or(0))?;
    Some(codex_usage_in(&tail_lines(&path, 1024 * 1024)))
}

pub(crate) fn codex_usage_in(lines: &[String]) -> AgentUsage {
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

/// opencode prices each reply itself: sum them over its session (and the
/// helper sessions it started).
fn opencode_usage(dir: &str, since_ms: Option<u64>) -> Option<AgentUsage> {
    let db = opencode::db_path()?;
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).ok()?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(1500));
    let id = opencode::newest_session(&conn, dir, since_ms)?;
    opencode_usage_in(&conn, &id)
}

pub(crate) fn opencode_usage_in(conn: &Connection, session: &str) -> Option<AgentUsage> {
    let (usd, tokens): (f64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(json_extract(data, '$.cost')), 0),
                    COALESCE(SUM(COALESCE(json_extract(data, '$.tokens.input'), 0) + COALESCE(json_extract(data, '$.tokens.output'), 0)
                               + COALESCE(json_extract(data, '$.tokens.reasoning'), 0) + COALESCE(json_extract(data, '$.tokens.cache.read'), 0)), 0)
             FROM message
             WHERE session_id IN (SELECT id FROM session WHERE id = ?1 OR parent_id = ?1)
               AND json_extract(data, '$.role') = 'assistant'",
            params![session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok()?;
    let model: Option<String> = conn
        .query_row(
            "SELECT json_extract(data, '$.modelID') FROM message WHERE session_id = ?1 AND json_extract(data, '$.role') = 'assistant' ORDER BY time_created DESC LIMIT 1",
            params![session],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    Some(AgentUsage { usd: Some(usd), tokens: Some(tokens.max(0) as u64), context_pct: None, model })
}

/// Gemini CLI (and Qwen Code, its fork) record a chat per session under
/// `~/.gemini/tmp/<project>/chats/session-*.json(l)`; each reply carries its
/// `tokens`. The project folder is the one whose `.project_root` names `dir`.
fn gemini_usage(home_dir: &str, dir: &str, since_ms: Option<u64>) -> Option<AgentUsage> {
    let tmp = home()?.join(home_dir).join("tmp");
    let since = since_ms.unwrap_or(0);
    let mut newest: Option<(u64, PathBuf)> = None;
    for project in std::fs::read_dir(&tmp).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let root = std::fs::read_to_string(project.join(".project_root")).unwrap_or_default();
        // older versions name the folder by a hash instead: those only count when nothing names `dir`
        let named = opencode::same_dir(root.trim(), dir);
        if !named && !root.trim().is_empty() {
            continue;
        }
        let Ok(chats) = std::fs::read_dir(project.join("chats")) else { continue };
        for f in chats.flatten().map(|e| e.path()) {
            let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with("session-") {
                continue;
            }
            let ms = modified_ms(&f);
            if ms >= since && newest.as_ref().map_or(true, |(b, _)| ms > *b) {
                newest = Some((ms, f));
            }
        }
    }
    let (_, path) = newest?;
    Some(cached(&path, |text| gemini_usage_in(text)))
}

/// Replies (`"type": "gemini"` with `tokens`) anywhere in the file, each counted
/// once by its id: a .json file holds them in one document, a .jsonl file as
/// updates that may repeat a reply.
pub(crate) fn gemini_usage_in(text: &str) -> AgentUsage {
    let mut replies: HashMap<String, (u64, Option<String>)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    fn walk(v: &Value, replies: &mut HashMap<String, (u64, Option<String>)>, order: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                if o.get("type").and_then(Value::as_str) == Some("gemini") {
                    if let Some(t) = o.get("tokens").filter(|t| t.is_object()) {
                        let n = |k: &str| t.get(k).and_then(Value::as_u64).unwrap_or(0);
                        let total = if n("total") > 0 { n("total") } else { n("input") + n("output") + n("thoughts") + n("tool") };
                        let id = o.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("#{}", order.len()));
                        if !replies.contains_key(&id) {
                            order.push(id.clone());
                        }
                        replies.insert(id, (total, o.get("model").and_then(Value::as_str).map(str::to_string)));
                    }
                }
                for x in o.values() {
                    walk(x, replies, order);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, replies, order)),
            _ => {}
        }
    }
    match serde_json::from_str::<Value>(text) {
        Ok(v) => walk(&v, &mut replies, &mut order),
        Err(_) => text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).for_each(|v| walk(&v, &mut replies, &mut order)),
    }
    if replies.is_empty() {
        return AgentUsage::default();
    }
    let tokens = replies.values().map(|(t, _)| t).sum();
    let model = order.iter().rev().find_map(|id| replies.get(id).and_then(|(_, m)| m.clone()));
    AgentUsage { usd: None, tokens: Some(tokens), context_pct: None, model }
}

/// grok keeps `signals.json` beside each session, with how full its context is.
/// Sessions sit under `~/.grok/sessions/<folder, percent-encoded>/<id>/`.
fn grok_usage(dir: &str, since_ms: Option<u64>) -> Option<AgentUsage> {
    let root = home()?.join(".grok").join("sessions");
    let folder = std::fs::read_dir(&root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| opencode::same_dir(&percent_decode(n), dir)))?;
    let since = since_ms.unwrap_or(0);
    let (_, signals) = std::fs::read_dir(&folder)
        .ok()?
        .flatten()
        .map(|e| e.path().join("signals.json"))
        .filter(|p| p.is_file())
        .map(|p| (modified_ms(&p), p))
        .filter(|(ms, _)| *ms >= since)
        .max_by_key(|(ms, _)| *ms)?;
    grok_usage_in(&std::fs::read_to_string(signals).ok()?)
}

pub(crate) fn grok_usage_in(text: &str) -> Option<AgentUsage> {
    let v: Value = serde_json::from_str(text).ok()?;
    let used = v.get("contextTokensUsed").and_then(Value::as_f64);
    let window = v.get("contextWindowTokens").and_then(Value::as_f64).filter(|w| *w > 0.0);
    let pct = match (used, window) {
        (Some(u), Some(w)) => Some((u / w * 100.0).min(100.0).round()),
        _ => v.get("contextWindowUsage").and_then(Value::as_f64),
    };
    Some(AgentUsage { usd: None, tokens: None, context_pct: pct, model: v.get("primaryModelId").and_then(Value::as_str).map(str::to_string) })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(x) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(x);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A file's result, worked out again only when the file has changed size.
fn cached(path: &Path, read: impl Fn(&str) -> AgentUsage) -> AgentUsage {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, (u64, AgentUsage)>>> = OnceLock::new();
    let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let cache = CACHE.get_or_init(Default::default);
    if let Some((l, u)) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(path) {
        if *l == len {
            return u.clone();
        }
    }
    let u = std::fs::read_to_string(path).map(|t| read(&t)).unwrap_or_default();
    cache.lock().unwrap_or_else(|e| e.into_inner()).insert(path.to_path_buf(), (len, u.clone()));
    u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_reads_its_last_token_count_and_model() {
        let lines: Vec<String> = [
            r#"{"type":"turn_context","payload":{"model":"gpt-5.3-codex"}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":1000},"last_token_usage":{"input_tokens":100},"model_context_window":1000}}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":54000},"last_token_usage":{"input_tokens":68000},"model_context_window":272000}}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{}}}"#,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let u = codex_usage_in(&lines);
        assert_eq!((u.tokens, u.context_pct, u.model.as_deref()), (Some(54000), Some(25.0), Some("gpt-5.3-codex")));
    }

    #[test]
    fn opencode_sums_its_own_prices_over_the_session_and_its_helpers() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE session (id text, parent_id text);
             CREATE TABLE message (id text, session_id text, time_created integer, data text);
             INSERT INTO session VALUES ('s1', NULL), ('h1', 's1'), ('other', NULL);
             INSERT INTO message VALUES
              ('m1','s1',1,'{\"role\":\"user\"}'),
              ('m2','s1',2,'{\"role\":\"assistant\",\"cost\":0.25,\"modelID\":\"deepseek-v4\",\"tokens\":{\"input\":100,\"output\":50,\"reasoning\":0,\"cache\":{\"read\":10,\"write\":0}}}'),
              ('m3','h1',3,'{\"role\":\"assistant\",\"cost\":0.5,\"tokens\":{\"input\":10,\"output\":5}}'),
              ('m4','other',4,'{\"role\":\"assistant\",\"cost\":9}');",
        )
        .unwrap();
        let u = opencode_usage_in(&c, "s1").unwrap();
        assert_eq!((u.usd, u.tokens, u.model.as_deref()), (Some(0.75), Some(175), Some("deepseek-v4")));
    }

    #[test]
    fn gemini_counts_each_reply_once_in_either_file_form() {
        let doc = r#"{"sessionId":"x","messages":[{"id":"u1","type":"user"},{"id":"g1","type":"gemini","model":"gemini-2.5-pro","tokens":{"input":900,"output":100,"total":1000}},{"id":"g2","type":"gemini","model":"gemini-3-pro","tokens":{"input":50,"output":25}}]}"#;
        let u = gemini_usage_in(doc);
        assert_eq!((u.tokens, u.model.as_deref()), (Some(1075), Some("gemini-3-pro")));
        let jsonl = [
            r#"{"sessionId":"x","kind":"main"}"#,
            r#"{"$set":{"messages":[{"id":"g1","type":"gemini","tokens":{"total":1000}}]}}"#,
            r#"{"$set":{"messages":[{"id":"g1","type":"gemini","tokens":{"total":1000}},{"id":"g2","type":"gemini","tokens":{"total":20}}]}}"#,
        ]
        .join("\n");
        assert_eq!(gemini_usage_in(&jsonl).tokens, Some(1020));
        assert_eq!(gemini_usage_in("{}"), AgentUsage::default());
    }

    #[test]
    fn grok_says_how_full_its_context_is() {
        let u = grok_usage_in(r#"{"contextWindowUsage":62,"contextTokensUsed":312996,"contextWindowTokens":500000,"primaryModelId":"grok-4.5"}"#).unwrap();
        assert_eq!((u.context_pct, u.model.as_deref()), (Some(63.0), Some("grok-4.5")));
        assert_eq!(percent_decode("D%3A%5CWhaleloSource"), "D:\\WhaleloSource");
    }
}
