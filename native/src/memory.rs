//! Project memory (projectmemory.ts): what every agent in a project should
//! know about it, one fact per entry, in `<project>/.bytehive/memory.json`.
//! Agents change the same file through the bytehive MCP tools, so it stays
//! byte compatible and a write never overwrites an agent's change.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A fact is a sentence or two, not a document.
pub const MAX_FACT: usize = 600;
/// Enough for what a project needs every agent to know.
pub const MAX_ENTRIES: usize = 80;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub text: String,
    /// When it was written or last changed, in ms.
    pub at: u64,
    /// An agent's name, or "you".
    pub by: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    pub entries: Vec<Entry>,
}

pub fn file_of(project: &str) -> PathBuf {
    Path::new(project).join(".bytehive").join("memory.json")
}

/// Short ids an agent can type back: m1, m2, never reused while one lives.
fn new_id(entries: &[Entry]) -> String {
    let n = entries.iter().filter_map(|e| e.id.strip_prefix('m')?.parse::<u64>().ok()).max().unwrap_or(0);
    format!("m{}", n + 1)
}

/// Read forgivingly: a missing, empty or unknown file is an empty memory,
/// and entries without text are dropped.
pub fn parse(raw: &str) -> Result<Memory, String> {
    if raw.trim().is_empty() {
        return Ok(Memory::default());
    }
    let data: Value = serde_json::from_str(raw).map_err(|_| "The project memory file (.bytehive/memory.json) is not valid JSON.".to_string())?;
    let mut entries: Vec<Entry> = Vec::new();
    for e in data.get("entries").and_then(Value::as_array).into_iter().flatten() {
        let Some(text) = e.get("text").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()) else { continue };
        let id = e.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| new_id(&entries));
        entries.push(Entry { id, text: text.to_string(), at: e.get("at").and_then(Value::as_f64).map_or(0, |n| n as u64), by: e.get("by").and_then(Value::as_str).unwrap_or("").to_string() });
    }
    Ok(Memory { entries })
}

/// JSON.stringify(m, null, 2) + "\n", as both sides write it.
pub fn serialize(m: &Memory) -> String {
    #[derive(Serialize)]
    struct File<'a> {
        version: u8,
        entries: &'a [Entry],
    }
    serde_json::to_string_pretty(&File { version: 1, entries: &m.entries }).expect("memory serializes") + "\n"
}

/// Things that look like a key, a token or a password never go in.
pub fn looks_secret(text: &str) -> bool {
    const SECRET: [&str; 9] = [
        r"\bsk-[A-Za-z0-9_-]{16,}",
        r"\bgh[pousr]_[A-Za-z0-9]{20,}",
        r"\bgithub_pat_[A-Za-z0-9_]{20,}",
        r"\bAKIA[0-9A-Z]{16}\b",
        r"\bxox[abpr]-[A-Za-z0-9-]{10,}",
        r"\bAIza[0-9A-Za-z_-]{30,}",
        r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
        r"(?i)\b(password|passwd|pwd|secret|api[_-]?key|token)\s*[:=]\s*\S{6,}",
    ];
    SECRET.iter().any(|r| regex::Regex::new(r).is_ok_and(|re| re.is_match(text)))
}

fn norm(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches(['.', ' ']).to_string()
}

fn clean(text: &str) -> Result<String, String> {
    let t = text.trim().to_string();
    if t.is_empty() {
        return Err("The fact is empty.".into());
    }
    if t.chars().count() > MAX_FACT {
        return Err(format!("Keep a fact under {MAX_FACT} characters: one fact per entry."));
    }
    if looks_secret(&t) {
        return Err("That looks like a key, a token or a password. Project memory is a plain file every agent reads, so it never holds secrets.".into());
    }
    Ok(t)
}

impl Memory {
    /// Add a fact. The same fact twice is kept once.
    pub fn add(&mut self, text: &str, by: &str, now: u64) -> Result<Entry, String> {
        let t = clean(text)?;
        if let Some(same) = self.entries.iter().find(|e| norm(&e.text) == norm(&t)) {
            return Ok(same.clone());
        }
        if self.entries.len() >= MAX_ENTRIES {
            return Err(format!("Project memory is full ({MAX_ENTRIES} facts). Update or delete an old one first."));
        }
        let e = Entry { id: new_id(&self.entries), text: t, at: now, by: by.to_string() };
        self.entries.push(e.clone());
        Ok(e)
    }

    pub fn update(&mut self, id: &str, text: &str, by: &str, now: u64) -> Result<Entry, String> {
        let t = clean(text)?;
        let e = self.entries.iter_mut().find(|e| e.id == id.trim()).ok_or_else(|| format!("No memory entry \"{id}\". Read the list with memory_get."))?;
        e.text = t;
        e.at = now;
        e.by = by.to_string();
        Ok(e.clone())
    }

    pub fn delete(&mut self, id: &str) -> Result<Entry, String> {
        let at = self.entries.iter().position(|e| e.id == id.trim()).ok_or_else(|| format!("No memory entry \"{id}\". Read the list with memory_get."))?;
        Ok(self.entries.remove(at))
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The memory and when its file last changed (none when there is no file).
pub fn load(project: &str) -> Result<(Memory, Option<SystemTime>), String> {
    let path = file_of(project);
    match std::fs::read_to_string(&path) {
        Ok(raw) => Ok((parse(&raw)?, mtime(&path))),
        Err(_) => Ok((Memory::default(), None)),
    }
}

/// Read, change, write, without overwriting an agent's change made in
/// between: when the file moved under it, read again and redo the change once.
pub fn change<T>(project: &str, f: impl Fn(&mut Memory) -> Result<T, String>) -> Result<(T, Memory, Option<SystemTime>), String> {
    let path = file_of(project);
    for attempt in 0..2 {
        let (mut mem, seen) = load(project)?;
        let result = f(&mut mem)?;
        if mtime(&path) != seen && attempt == 0 {
            continue;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, serialize(&mem)).map_err(|e| e.to_string())?;
        return Ok((result, mem, mtime(&path)));
    }
    Err("The memory changed while saving. Try again.".into())
}

/// What an agent is asked to do with one fact, or all of them (fixPrompt):
/// in English, on one line so the CLI takes it whole.
pub fn fix_prompt(entry: Option<&Entry>, note: &str) -> String {
    let one = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let said = if one(note).is_empty() { String::new() } else { format!(" The user says: {}.", one(note)) };
    match entry {
        Some(e) => format!(
            "Fix this entry of the project memory, [{}]: \"{}\".{said} Check it against the code, then call memory_update with id {} and the corrected fact, or memory_delete if it no longer holds. Reply in one line with what you changed.",
            e.id,
            one(&e.text),
            e.id
        ),
        None => format!(
            "Review this project's shared memory: read it with memory_get.{said} Check each fact against the code. Fix wrong or stale facts with memory_update, delete ones that no longer hold with memory_delete, merge duplicates, and save important missing facts with memory_save. Reply with a short list of what you changed."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{Memory, fix_prompt, looks_secret, parse, serialize};

    #[test]
    fn reads_writes_and_keeps_ids() {
        let m = parse(r#"{"version":1,"entries":[{"id":"m3","text":" a fact ","at":5,"by":"Ana"},{"text":""},{"text":"no id"}]}"#).unwrap();
        assert_eq!(m.entries.len(), 2);
        assert_eq!((m.entries[0].text.as_str(), m.entries[1].id.as_str()), ("a fact", "m4"));
        assert!(serialize(&m).starts_with("{\n  \"version\": 1,\n  \"entries\": ["));
        assert!(parse("not json").is_err());
        assert_eq!(parse("").unwrap(), Memory::default());
    }

    #[test]
    fn adds_once_refuses_secrets() {
        let mut m = Memory::default();
        let a = m.add("Builds use -j 4.", "you", 1).unwrap();
        assert_eq!(m.add("builds use  -j 4", "you", 2).unwrap().id, a.id);
        assert!(m.add("api_key = abcdef123456", "you", 3).is_err());
        assert!(looks_secret("sk-abcdefghijklmnopqrstu"));
        assert_eq!(m.delete("m1").unwrap().id, "m1");
        assert!(m.update("m1", "x", "you", 4).is_err());
    }

    #[test]
    fn asks_an_agent_in_one_line() {
        let e = super::Entry { id: "m2".into(), text: "Two\nlines".into(), at: 0, by: String::new() };
        let p = fix_prompt(Some(&e), "  it moved ");
        assert!(p.contains("[m2]: \"Two lines\". The user says: it moved."));
        assert!(fix_prompt(None, "").starts_with("Review this project's shared memory: read it with memory_get. Check"));
    }
}
