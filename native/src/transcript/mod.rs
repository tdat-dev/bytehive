//! A Claude Code transcript (`~/.claude/projects/<slug>/<session>.jsonl`)
//! as the chat shows it: your messages, the agent's replies, and the steps it
//! took in between. Ported from `createChat().feed` in src/chatmodel.ts; the
//! side panel's bookkeeping (usage, files, tasks, notes) is left out.
#![allow(dead_code)]

mod steps;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use agent_core::claude::project_dir;
use agent_core::transcript::{is_session_id, read_start};
use regex::Regex;
use serde_json::Value;

use steps::s;

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// What you typed.
    User { id: String, text: String },
    /// The agent's words, markdown.
    Text { id: String, text: String },
    /// One tool call: "Read" + "main.rs", "Ran" + "npm test", ... as chatmodel.ts words it.
    /// `done` once its result is in; `code` unless a command's target is its
    /// description rather than the command itself.
    Step { id: String, verb: String, target: String, error: bool, done: bool, code: bool },
}

static MODEL_SET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<local-command-stdout>Set model to `([^`]+)`").unwrap());
static BASH_IN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*<bash-input>([\s\S]*?)</bash-input>\s*$").unwrap());
static BASH_OUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<bash-stdout>([\s\S]*?)</bash-stdout>").unwrap());
static BASH_ERR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<bash-stderr>([\s\S]*?)</bash-stderr>").unwrap());
static COMMAND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<command-name>([^<]*)</command-name>").unwrap());
static LOCAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<local-command-(stdout|stderr)>|<local-command-caveat>").unwrap());
static REMINDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<system-reminder>[\s\S]*?</system-reminder>").unwrap());

/// One conversation, built up line by line.
#[derive(Default)]
pub struct Chat {
    pub items: Vec<Item>,
    /// Where each tool call's step sits in `items`, by tool_use id.
    steps: HashMap<String, usize>,
    /// Messages already shown from the queue, by their uuid.
    queued: HashSet<String>,
    /// The last ! command you ran, for the output that follows it.
    last_bang: String,
    n: usize,
}

impl Chat {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&mut self) -> String {
        let id = format!("c{}", self.n);
        self.n += 1;
        id
    }

    /// Feed complete JSONL lines (as many as given); partial trailing line is ignored.
    pub fn feed(&mut self, text: &str) {
        for line in text.split('\n') {
            if line.trim().is_empty() {
                continue;
            }
            // a half-written line does not parse, so it is skipped
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            // a helper agent's inner steps, injected context
            if v.get("isSidechain") == Some(&Value::Bool(true)) || v.get("isMeta") == Some(&Value::Bool(true)) {
                continue;
            }
            match s(&v, "type") {
                "user" => self.on_user(&v),
                "assistant" => self.on_assistant(&v),
                "attachment" => self.on_queued(&v),
                _ => {}
            }
        }
    }

    fn on_user(&mut self, v: &Value) {
        // already shown when it was queued
        if self.queued.contains(s(v, "uuid")) {
            return;
        }
        let human = match v.get("origin") {
            None | Some(Value::Null) => true,
            Some(o) => s(o, "kind") == "human",
        };
        match v.get("message").and_then(|m| m.get("content")) {
            Some(Value::String(t)) => self.user_text(t, 0, human),
            Some(Value::Array(parts)) => {
                let (mut text, mut images) = (String::new(), 0);
                for part in parts {
                    match s(part, "type") {
                        "tool_result" => {
                            let Some(&at) = self.steps.get(s(part, "tool_use_id")) else { continue };
                            if let Some(Item::Step { error, done, .. }) = self.items.get_mut(at) {
                                *error = part.get("is_error") == Some(&Value::Bool(true));
                                *done = true;
                            }
                        }
                        "text" => push_line(&mut text, s(part, "text")),
                        "image" => images += 1,
                        _ => {}
                    }
                }
                if !text.is_empty() || images > 0 {
                    self.user_text(&text, images, human);
                }
            }
            _ => {}
        }
    }

    /// One thing you said, unless it is the CLI's own noise around it.
    fn user_text(&mut self, raw: &str, images: usize, human: bool) {
        // hook output, task notices...
        if !human {
            return;
        }
        // /model's answer: the TS shows it as a note, which this list has no room for
        if MODEL_SET.is_match(raw) {
            return;
        }
        // A ! command you ran yourself: your "!command", then a step for its output.
        if let Some(c) = BASH_IN.captures(raw) {
            self.last_bang = c[1].trim().to_string();
            let id = self.next_id();
            self.items.push(Item::User { id, text: format!("!{}", self.last_bang) });
            return;
        }
        let out = BASH_OUT.captures(raw).map(|c| c[1].trim().to_string());
        let err = BASH_ERR.captures(raw).map(|c| c[1].trim().to_string());
        if out.is_some() || err.is_some() {
            let id = self.next_id();
            let error = err.as_deref().is_some_and(|e| !e.is_empty());
            self.items.push(Item::Step { id, verb: "Ran".into(), target: self.last_bang.clone(), error, done: true, code: true });
            return;
        }
        // slash commands, their output, "[Request interrupted by user]": notes in the TS
        if COMMAND.is_match(raw) || LOCAL.is_match(raw) || raw.trim().starts_with("[Request interrupted by user") {
            return;
        }
        let text = REMINDER.replace_all(raw, "").trim().to_string();
        if text.is_empty() && images == 0 {
            return;
        }
        // the picture itself is not carried here, only that there was one
        let text = if text.is_empty() { "[Image]".to_string() } else { text };
        let id = self.next_id();
        self.items.push(Item::User { id, text });
    }

    /// A message you typed while the agent was working: the CLI queues it and
    /// hands it over mid-turn as an attachment, not as a user line.
    fn on_queued(&mut self, v: &Value) {
        let Some(a) = v.get("attachment") else { return };
        if s(a, "type") != "queued_command" {
            return;
        }
        if a.get("commandMode").is_some_and(|m| m.as_str() != Some("prompt")) {
            return;
        }
        let human = match a.get("origin") {
            Some(o) if !o.is_null() => s(o, "kind") == "human",
            _ => a.get("humanTurn") == Some(&Value::Bool(true)),
        };
        if !human {
            return;
        }
        let source = s(a, "source_uuid");
        if !source.is_empty() && !self.queued.insert(source.to_string()) {
            return;
        }
        match a.get("prompt") {
            Some(Value::String(t)) => self.user_text(t, 0, true),
            Some(Value::Array(parts)) => {
                let (mut text, mut images) = (String::new(), 0);
                for part in parts {
                    match s(part, "type") {
                        "text" => push_line(&mut text, s(part, "text")),
                        "image" => images += 1,
                        _ => {}
                    }
                }
                if !text.is_empty() || images > 0 {
                    self.user_text(&text, images, true);
                }
            }
            _ => {}
        }
    }

    fn on_assistant(&mut self, v: &Value) {
        let Some(parts) = v.get("message").and_then(|m| m.get("content")).and_then(Value::as_array) else { return };
        for part in parts {
            match s(part, "type") {
                "text" => {
                    let text = s(part, "text").trim();
                    if !text.is_empty() && text != "(no content)" {
                        let id = self.next_id();
                        self.items.push(Item::Text { id, text: text.to_string() });
                    }
                }
                "tool_use" => {
                    // one reply is written as several lines; each call shows once
                    let tid = s(part, "id");
                    if self.steps.contains_key(tid) {
                        continue;
                    }
                    let (name, input) = (s(part, "name"), part.get("input").unwrap_or(&Value::Null));
                    let (verb, target) = steps::describe(name, input);
                    let code = !(matches!(name, "Bash" | "PowerShell") && !s(input, "description").is_empty());
                    let id = self.next_id();
                    self.steps.insert(tid.to_string(), self.items.len());
                    self.items.push(Item::Step { id, verb, target, error: false, done: false, code });
                }
                // thinking, images, anything new: not shown
                _ => {}
            }
        }
    }
}

/// Add a line to text joined by newlines.
fn push_line(text: &mut String, line: &str) {
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(line);
}

/// One session read from disk.
pub struct Read {
    pub chat: Chat,
    /// Only the end was read: earlier messages are not in `chat`.
    pub partial: bool,
}

/// Read session `id` that ran in folder `cwd`. With `tail_bytes`, only the end of the
/// file is read (starting at the next full line); `partial` says whether the start was skipped.
pub fn read_session(cwd: &str, id: &str, tail_bytes: Option<u64>) -> Result<Read, String> {
    if cwd.trim().is_empty() {
        return Err("This session doesn't say which folder it ran in, so it can't be found.".into());
    }
    // A session id is a UUID; anything else never reaches the filesystem.
    if !is_session_id(id) {
        return Err("This session's id doesn't look like one Claude Code writes, so it can't be opened.".into());
    }
    let dir = project_dir(cwd).ok_or("Your home folder can't be found, so neither can this session.")?;
    read_file(&dir.join(format!("{id}.jsonl")), tail_bytes)
}

/// Read one transcript file, whole or from its tail.
fn read_file(path: &Path, tail_bytes: Option<u64>) -> Result<Read, String> {
    use std::io::{Read as _, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "This session's file is gone.".to_string(),
        _ => format!("This session's file can't be opened: {e}"),
    })?;
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = read_start(&mut file, size, 0, tail_bytes);
    let mut buf = Vec::new();
    file.seek(SeekFrom::Start(start))
        .and_then(|_| file.read_to_end(&mut buf))
        .map_err(|e| format!("This session's file can't be read: {e}"))?;
    // Whole lines only; one being written right now waits for the next read.
    buf.truncate(buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1));
    let mut chat = Chat::new();
    chat.feed(&String::from_utf8_lossy(&buf));
    Ok(Read { chat, partial: start > 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = r#"{"type":"user","uuid":"u1","message":{"role":"user","content":"Fix the build<system-reminder>noise</system-reminder>"}}"#;
    const REPLY: &str = r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"thinking","thinking":"hm"},{"type":"text","text":"Looking at it now."}]}}"#;
    const CALLS: &str = r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"D:\\x\\src\\main.rs"}},{"type":"tool_use","id":"t2","name":"Bash","input":{"command":"cargo test\n--all"}}]}}"#;
    const RESULTS: &str = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"fn main() {}"},{"type":"tool_result","tool_use_id":"t2","content":"failed","is_error":true}]}}"#;

    fn step(id: &str, verb: &str, target: &str, error: bool) -> Item {
        Item::Step { id: id.into(), verb: verb.into(), target: target.into(), error, done: true, code: true }
    }

    #[test]
    fn user_reply_and_steps() {
        let mut chat = Chat::new();
        chat.feed(&[USER, REPLY, CALLS, RESULTS].join("\n"));
        assert_eq!(chat.items, vec![
            Item::User { id: "c0".into(), text: "Fix the build".into() },
            Item::Text { id: "c1".into(), text: "Looking at it now.".into() },
            step("c2", "Read", "main.rs", false),
            step("c3", "Ran", "cargo test", true),
        ]);
    }

    #[test]
    fn tool_result_only_line_adds_nothing() {
        let mut chat = Chat::new();
        chat.feed(CALLS);
        chat.feed(RESULTS);
        assert_eq!(chat.items.len(), 2);
        // an orphan result (its call was before a tail read) is skipped too
        chat.feed(r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"gone","content":"x"}]}}"#);
        assert_eq!(chat.items.len(), 2);
    }

    #[test]
    fn same_call_streamed_twice_shows_once() {
        let mut chat = Chat::new();
        chat.feed(&format!("{CALLS}\n{CALLS}\n"));
        assert_eq!(chat.items.len(), 2);
    }

    #[test]
    fn noise_is_skipped() {
        let mut chat = Chat::new();
        chat.feed(&[
            r#"{"type":"user","isMeta":true,"message":{"content":"Caveat: injected"}}"#,
            r#"{"type":"user","message":{"content":"<command-name>/clear</command-name>"}}"#,
            r#"{"type":"user","message":{"content":"<local-command-stdout>ok</local-command-stdout>"}}"#,
            r#"{"type":"user","origin":{"kind":"hook"},"message":{"content":"hook said"}}"#,
            r#"{"type":"user","isSidechain":true,"message":{"content":"helper"}}"#,
            r#"{"type":"user","message":{"content":"[Request interrupted by user]"}}"#,
            r#"{"type":"user","message":{"content":"<bash-input>ls</bash-input>"}}"#,
            r#"{"type":"user","message":{"content":"<bash-stdout>a</bash-stdout><bash-stderr></bash-stderr>"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"half"#,
        ].join("\n"));
        assert_eq!(chat.items, vec![Item::User { id: "c0".into(), text: "!ls".into() }, step("c1", "Ran", "ls", false)]);
    }

    #[test]
    fn queued_message_shows_once() {
        let mut chat = Chat::new();
        chat.feed(r#"{"type":"attachment","attachment":{"type":"queued_command","prompt":"also this","humanTurn":true,"source_uuid":"q1"}}"#);
        chat.feed(r#"{"type":"user","uuid":"q1","message":{"content":"also this"}}"#);
        assert_eq!(chat.items, vec![Item::User { id: "c0".into(), text: "also this".into() }]);
    }

    #[test]
    fn tail_read_starts_at_next_full_line() {
        let dir = std::env::temp_dir().join(format!("bytehive-transcript-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        let early = r#"{"type":"user","message":{"content":"early message that is long enough to be cut in half"}}"#;
        let late = r#"{"type":"user","message":{"content":"late"}}"#;
        std::fs::write(&path, format!("{early}\n{late}\n")).unwrap();
        // the tail lands inside the first line: reading starts at the second
        let got = read_file(&path, Some(late.len() as u64 + 10)).unwrap();
        assert!(got.partial);
        assert_eq!(got.chat.items, vec![Item::User { id: "c0".into(), text: "late".into() }]);
        let whole = read_file(&path, None).unwrap();
        assert!(!whole.partial);
        assert_eq!(whole.chat.items.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(read_file(&path, None).err().as_deref(), Some("This session's file is gone."));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(read_session("", "abc", None).err().unwrap().contains("which folder"));
        assert!(read_session("D:\\x", "..\\..\\secret", None).is_err());
    }
}
