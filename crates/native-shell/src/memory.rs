//! Project memory: what every agent in a project should know about it
//! (conventions, decisions, where things are, gotchas), one fact per entry,
//! in `<project>/.bytehive/memory.json`. Agents read and save it through the
//! bytehive MCP tools; you read and edit it in the Memory sheet. Port of the
//! web app's `projectmemory.ts`, same file format, so both sides share it.
//! Every change re-reads the file first and writes through a temp file, so a
//! fact an agent saved a second ago survives.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryEntry {
    pub id: String,
    pub text: String,
    /// ms when it was written or last changed.
    pub at: u64,
    /// Who wrote it: an agent's name, or "you".
    pub by: String,
}

/// A fact is a sentence or two, not a document.
pub const MAX_FACT: usize = 600;
/// Small enough to read at the start of every session.
pub const MAX_ENTRIES: usize = 80;

pub fn memory_path(project: &str) -> PathBuf {
    Path::new(project).join(".bytehive").join("memory.json")
}

/// Reads memory.json, forgiving: missing or empty is an empty memory, and
/// entries without text are dropped. Broken JSON is an error, so a save
/// never overwrites a file someone is halfway through fixing.
pub fn parse_memory(raw: &str) -> Result<Vec<MemoryEntry>, String> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let v: Value = serde_json::from_str(raw).map_err(|_| "The project memory file (.bytehive/memory.json) is not valid JSON.".to_string())?;
    let mut out: Vec<MemoryEntry> = Vec::new();
    for e in v["entries"].as_array().into_iter().flatten() {
        let Some(text) = e["text"].as_str().map(str::trim).filter(|t| !t.is_empty()) else { continue };
        let id = e["id"].as_str().filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| new_id(&out));
        out.push(MemoryEntry { id, text: text.to_string(), at: e["at"].as_u64().unwrap_or(0), by: e["by"].as_str().unwrap_or("").to_string() });
    }
    Ok(out)
}

pub fn serialize_memory(entries: &[MemoryEntry]) -> String {
    let list: Vec<Value> = entries.iter().map(|e| json!({ "id": e.id, "text": e.text, "at": e.at, "by": e.by })).collect();
    serde_json::to_string_pretty(&json!({ "version": 1, "entries": list })).unwrap_or_default() + "\n"
}

/// Short ids an agent can type back: m1, m2, never reused while one lives.
fn new_id(entries: &[MemoryEntry]) -> String {
    let n = entries.iter().filter_map(|e| e.id.strip_prefix('m')?.parse::<u64>().ok()).max().unwrap_or(0);
    format!("m{}", n + 1)
}

static SECRET: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\bsk-[A-Za-z0-9_-]{16,}",
        r"\bgh[pousr]_[A-Za-z0-9]{20,}",
        r"\bgithub_pat_[A-Za-z0-9_]{20,}",
        r"\bAKIA[0-9A-Z]{16}\b",
        r"\bxox[abpr]-[A-Za-z0-9-]{10,}",
        r"\bAIza[0-9A-Za-z_-]{30,}",
        r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
        r"(?i)\b(password|passwd|pwd|secret|api[_-]?key|token)\s*[:=]\s*\S{6,}",
    ]
    .iter()
    .map(|r| Regex::new(r).unwrap())
    .collect()
});

/// Looks like a key, a token or a password: never goes in a plain file
/// every agent reads.
pub fn looks_secret(text: &str) -> bool {
    SECRET.iter().any(|r| r.is_match(text))
}

fn norm(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches(['.', ' ']).to_string()
}

fn clean_fact(text: &str) -> Result<String, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("The fact is empty.".into());
    }
    if t.chars().count() > MAX_FACT {
        return Err(format!("Keep a fact under {MAX_FACT} characters: one fact per entry."));
    }
    if looks_secret(t) {
        return Err("That looks like a key, a token or a password. Project memory is a plain file every agent reads, so it never holds secrets.".into());
    }
    Ok(t.to_string())
}

/// Adds a fact; the same fact twice is kept once. Hands back its id.
pub fn add_fact(m: &mut Vec<MemoryEntry>, text: &str, by: &str, now: u64) -> Result<String, String> {
    let t = clean_fact(text)?;
    if let Some(same) = m.iter().find(|e| norm(&e.text) == norm(&t)) {
        return Ok(same.id.clone());
    }
    if m.len() >= MAX_ENTRIES {
        return Err(format!("Project memory is full ({MAX_ENTRIES} facts). Update or delete an old one first."));
    }
    let id = new_id(m);
    m.push(MemoryEntry { id: id.clone(), text: t, at: now, by: by.to_string() });
    Ok(id)
}

pub fn update_fact(m: &mut [MemoryEntry], id: &str, text: &str, by: &str, now: u64) -> Result<(), String> {
    let t = clean_fact(text)?;
    let e = m.iter_mut().find(|e| e.id == id).ok_or_else(|| format!("No memory entry \"{id}\"."))?;
    e.text = t;
    e.at = now;
    e.by = by.to_string();
    Ok(())
}

/// Removes a fact and hands it back, for Undo.
pub fn delete_fact(m: &mut Vec<MemoryEntry>, id: &str) -> Result<MemoryEntry, String> {
    let i = m.iter().position(|e| e.id == id).ok_or_else(|| format!("No memory entry \"{id}\"."))?;
    Ok(m.remove(i))
}

/// What an agent is asked to do with one fact, or with all (`None`). In
/// English like every hand-off, on one line so the CLI takes it whole.
pub fn fix_prompt(entry: Option<&MemoryEntry>, note: &str) -> String {
    let one = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let said = if one(note).is_empty() { String::new() } else { format!(" The user says: {}.", one(note)) };
    match entry {
        Some(e) => format!(
            "Fix this entry of the project memory, [{}]: \"{}\".{said} Check it against the code, then call memory_update with id {} and the corrected fact, or memory_delete if it no longer holds. Reply in one line with what you changed.",
            e.id,
            one(&e.text),
            e.id
        ),
        None => format!("Review this project's shared memory: read it with memory_get.{said} Check each fact against the code. Fix wrong or stale facts with memory_update, delete ones that no longer hold with memory_delete, merge duplicates, and save important missing facts with memory_save. Reply with a short list of what you changed."),
    }
}

/// The file's facts; a missing file is an empty memory.
pub fn load(path: &Path) -> Result<Vec<MemoryEntry>, String> {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_memory(&raw),
        Err(_) => Ok(Vec::new()),
    }
}

/// When the file last changed, to notice an agent's save.
pub fn stamp(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Read the newest memory, change it, write it back through a temp file.
pub fn change<T>(path: &Path, f: impl FnOnce(&mut Vec<MemoryEntry>) -> Result<T, String>) -> Result<(T, Vec<MemoryEntry>), String> {
    let mut m = load(path)?;
    let out = f(&mut m)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not make {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serialize_memory(&m)).map_err(|e| format!("Could not write the memory: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Could not save the memory: {e}"))?;
    Ok((out, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_web_apps_file() {
        let raw = r#"{"version":1,"entries":[{"id":"m1","text":" Use rtk ","at":5,"by":"Ana"},{"text":""},{"text":"No id"}]}"#;
        let m = parse_memory(raw).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].text, "Use rtk");
        assert_eq!(m[1].id, "m2");
        assert!(parse_memory("{nope").is_err());
        assert_eq!(parse_memory("").unwrap(), vec![]);
        assert_eq!(parse_memory(&serialize_memory(&m)).unwrap(), m);
    }

    #[test]
    fn adds_once_and_refuses_secrets() {
        let mut m = Vec::new();
        assert_eq!(add_fact(&mut m, "Files stay under 500 lines.", "you", 1).unwrap(), "m1");
        assert_eq!(add_fact(&mut m, "files stay under   500 lines", "you", 2).unwrap(), "m1");
        assert_eq!(m.len(), 1);
        assert!(add_fact(&mut m, "api_key = abcdef123456", "you", 3).is_err());
        assert!(add_fact(&mut m, "  ", "you", 3).is_err());
        assert!(looks_secret("token: sk-abcdefghijklmnopqrstu"));
        update_fact(&mut m, "m1", "Files stay under 400 lines.", "Bob", 9).unwrap();
        assert_eq!((m[0].by.as_str(), m[0].at), ("Bob", 9));
        let gone = delete_fact(&mut m, "m1").unwrap();
        assert_eq!(gone.id, "m1");
        assert!(delete_fact(&mut m, "m1").is_err());
    }

    #[test]
    fn ids_are_not_reused_while_one_lives() {
        let mut m = Vec::new();
        add_fact(&mut m, "a", "you", 0).unwrap();
        add_fact(&mut m, "b", "you", 0).unwrap();
        delete_fact(&mut m, "m1").unwrap();
        assert_eq!(add_fact(&mut m, "c", "you", 0).unwrap(), "m3");
    }

    #[test]
    fn prompts_name_the_fact_and_the_tools() {
        let e = MemoryEntry { id: "m4".into(), text: "Builds go\nthrough rtk".into(), at: 0, by: String::new() };
        let p = fix_prompt(Some(&e), " stale ");
        assert!(p.contains("[m4]") && p.contains("Builds go through rtk") && p.contains("The user says: stale.") && p.contains("memory_update with id m4"));
        assert!(!p.contains('\n'));
        assert!(fix_prompt(None, "").contains("memory_get"));
    }

    #[test]
    fn changes_go_through_the_file() {
        let dir = std::env::temp_dir().join(format!("bytehive-mem-{}", std::process::id()));
        let path = memory_path(&dir.to_string_lossy());
        let (id, m) = change(&path, |m| add_fact(m, "First fact", "you", 1)).unwrap();
        assert_eq!((id.as_str(), m.len()), ("m1", 1));
        assert_eq!(load(&path).unwrap()[0].text, "First fact");
        assert!(change(&path, |m| delete_fact(m, "m9")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
