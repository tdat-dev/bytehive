//! The pure pieces of the opencode parser: unified diffs, patch bodies,
//! and how a tool call reads in the chat.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::codex::first_line;
use crate::muse::host_of;
use crate::text::{base_name, line_diff, s_at, MAX_DIFF_LINES, MAX_OUTPUT};
use crate::types::{ChatImage, DiffLine, DiffSign, Todo, TodoState};

/// Tool output, trimmed: line endings unified, blankness gone, capped.
pub fn trim_output(text: &str) -> String {
    let t = text.replace("\r\n", "\n");
    let t = t.trim();
    if t.chars().count() > MAX_OUTPUT {
        let mut out: String = t.chars().take(MAX_OUTPUT).collect();
        out.push('\n');
        out.push('\u{2026}');
        out
    } else {
        t.to_string()
    }
}

/// The lines of a unified diff (what opencode keeps for an edit), headers left out.
pub fn unified_diff(patch: &str) -> Vec<DiffLine> {
    let mut out: Vec<DiffLine> = Vec::new();
    let mut in_hunk = false;
    for line in patch.replace("\r\n", "\n").split('\n') {
        if line.starts_with("@@") {
            if !out.is_empty() {
                out.push(DiffLine { sign: DiffSign::Same, text: "\u{22ef}".to_string() });
            }
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        if line.starts_with('\\') {
            continue;
        }
        match line.chars().next() {
            Some('+') => out.push(DiffLine { sign: DiffSign::Added, text: line[1..].to_string() }),
            Some('-') => out.push(DiffLine { sign: DiffSign::Removed, text: line[1..].to_string() }),
            Some(' ') => out.push(DiffLine { sign: DiffSign::Same, text: line[1..].to_string() }),
            None => out.push(DiffLine { sign: DiffSign::Same, text: String::new() }),
            _ => {}
        }
    }
    while matches!(out.last(), Some(DiffLine { sign: DiffSign::Same, text }) if text.is_empty()) {
        out.pop();
    }
    out.truncate(MAX_DIFF_LINES);
    out
}

fn data_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)^data:([^;,]+);base64,(.*)$").unwrap())
}

/// A picture kept as a data URL (a pasted image, a screenshot a tool read).
pub fn picture_of(url: &str, mime: &str) -> Option<ChatImage> {
    let m = data_url_re().captures(url)?;
    let media = m.get(1).map(|g| g.as_str()).unwrap_or(mime).to_lowercase();
    match media.as_str() {
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" => Some(ChatImage { media, data: m[2].to_string() }),
        _ => None,
    }
}

fn patch_file_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\*\*\* (Add|Update|Delete) File: (.+)$").unwrap())
}

/// The files an apply_patch body adds, changes or deletes.
pub struct PatchRef {
    pub path: String,
    pub kind: PatchRefKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchRefKind {
    Add,
    Update,
    Delete,
}

pub fn patch_files(body: &str) -> Vec<PatchRef> {
    patch_file_re()
        .captures_iter(body)
        .map(|m| {
            let kind = match &m[1] {
                "Add" => PatchRefKind::Add,
                "Delete" => PatchRefKind::Delete,
                _ => PatchRefKind::Update,
            };
            PatchRef { path: m[2].trim().to_string(), kind }
        })
        .collect()
}

/// How an opencode tool call reads in the chat: a verb, what it touched, and extras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpencodeDescribed {
    pub tool: String,
    pub verb: String,
    pub target: String,
    pub full: Option<String>,
    pub code: bool,
    pub diff: Option<Vec<DiffLine>>,
    pub added: Option<u32>,
    pub removed: Option<u32>,
    pub todos: Option<Vec<Todo>>,
}

impl OpencodeDescribed {
    fn plain(tool: &str, verb: &str, target: String) -> OpencodeDescribed {
        OpencodeDescribed { tool: tool.to_string(), verb: verb.to_string(), target, full: None, code: false, diff: None, added: None, removed: None, todos: None }
    }
}

fn image_ext_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\.(png|jpe?g|gif|webp|bmp)$").unwrap())
}

fn word_space_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[_-]+").unwrap())
}

pub fn describe_opencode_tool(tool: &str, input: &Value, metadata: &Value) -> OpencodeDescribed {
    let mut file = s_at(input, "filePath");
    if file.is_empty() {
        file = s_at(input, "file_path");
    }
    if file.is_empty() {
        file = s_at(input, "path");
    }
    if file.is_empty() {
        file = s_at(metadata, "filepath");
    }
    match tool {
        "bash" => {
            let cmd = s_at(input, "command");
            let said = s_at(input, "description");
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Ran".to_string(),
                target: if said.is_empty() { first_line(cmd, 120) } else { said.to_string() },
                full: Some(cmd.to_string()),
                code: said.is_empty(),
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "read" => OpencodeDescribed {
            tool: tool.to_string(),
            verb: if image_ext_re().is_match(file) { "Looked at".to_string() } else { "Read".to_string() },
            target: base_name(file).to_string(),
            full: Some(file.to_string()),
            code: true,
            ..OpencodeDescribed::plain(tool, "", String::new())
        },
        "edit" | "multiedit" => {
            let empty = Value::Null;
            let fd = metadata.get("filediff").unwrap_or(&empty);
            let patch = s_at(fd, "patch");
            let patch = if patch.is_empty() { s_at(metadata, "diff") } else { patch };
            let diff: Vec<DiffLine> = if !patch.is_empty() {
                unified_diff(patch)
            } else if let Some(edits) = input.get("edits").and_then(|e| e.as_array()) {
                let mut d = Vec::new();
                for (k, e) in edits.iter().enumerate() {
                    if k > 0 {
                        d.push(DiffLine { sign: DiffSign::Same, text: "\u{22ef}".to_string() });
                    }
                    d.extend(line_diff(s_at(e, "oldString"), s_at(e, "newString")));
                }
                d.truncate(MAX_DIFF_LINES);
                d
            } else {
                let mut d = line_diff(s_at(input, "oldString"), s_at(input, "newString"));
                d.truncate(MAX_DIFF_LINES);
                d
            };
            let mut counted_added = 0;
            let mut counted_removed = 0;
            for l in &diff {
                match l.sign {
                    DiffSign::Added => counted_added += 1,
                    DiffSign::Removed => counted_removed += 1,
                    DiffSign::Same => {}
                }
            }
            let added = fd.get("additions").and_then(|n| n.as_u64()).map(|n| n as u32).unwrap_or(counted_added);
            let removed = fd.get("deletions").and_then(|n| n.as_u64()).map(|n| n as u32).unwrap_or(counted_removed);
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Edited".to_string(),
                target: base_name(file).to_string(),
                full: Some(file.to_string()),
                code: true,
                diff: Some(diff),
                added: Some(added),
                removed: Some(removed),
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "write" => {
            let content = s_at(input, "content");
            let lines: Vec<&str> = if content.is_empty() { Vec::new() } else { content.split('\n').collect() };
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Wrote".to_string(),
                target: base_name(file).to_string(),
                full: Some(file.to_string()),
                code: true,
                diff: Some(lines.iter().take(MAX_DIFF_LINES).map(|text| DiffLine { sign: DiffSign::Added, text: text.to_string() }).collect()),
                added: Some(lines.len() as u32),
                removed: Some(0),
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "patch" | "apply_patch" => {
            let mut body = s_at(input, "patchText");
            if body.is_empty() {
                body = s_at(input, "patch");
            }
            if body.is_empty() {
                body = s_at(input, "input");
            }
            let files = patch_files(body);
            let target = if files.len() == 1 { base_name(&files[0].path).to_string() } else { format!("{} files", files.len()) };
            let full = files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>().join("\n");
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Edited".to_string(),
                target,
                full: if full.is_empty() { None } else { Some(full) },
                code: files.len() == 1,
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "glob" => OpencodeDescribed {
            tool: tool.to_string(),
            verb: "Looked for files".to_string(),
            target: s_at(input, "pattern").to_string(),
            code: true,
            ..OpencodeDescribed::plain(tool, "", String::new())
        },
        "grep" => {
            let path = s_at(input, "path");
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Searched for".to_string(),
                target: s_at(input, "pattern").to_string(),
                full: if path.is_empty() { None } else { Some(path.to_string()) },
                code: true,
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "list" | "ls" => {
            let what = if file.is_empty() { "." } else { file };
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Listed".to_string(),
                target: base_name(what).to_string(),
                full: if file.is_empty() { None } else { Some(file.to_string()) },
                code: true,
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "webfetch" => {
            let url = s_at(input, "url");
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Read".to_string(),
                target: host_of(url).to_string(),
                full: Some(url.to_string()),
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "websearch" => OpencodeDescribed::plain(tool, "Searched the web for", s_at(input, "query").to_string()),
        "todowrite" | "todoread" => {
            let todos: Vec<Todo> = input
                .get("todos")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|t| Todo { text: s_at(t, "content").to_string(), state: TodoState::parse(s_at(t, "status")) })
                .collect();
            let done = todos.iter().filter(|t| t.state == TodoState::Completed).count();
            OpencodeDescribed {
                tool: tool.to_string(),
                verb: "Updated the plan".to_string(),
                target: format!("{done} of {} done", todos.len()),
                todos: Some(todos),
                ..OpencodeDescribed::plain(tool, "", String::new())
            }
        }
        "task" => {
            let said = s_at(input, "description");
            OpencodeDescribed::plain(tool, "Asked a helper agent to", if said.is_empty() { first_line(s_at(input, "prompt"), 120) } else { said.to_string() })
        }
        "skill" => OpencodeDescribed::plain(tool, "Used the skill", s_at(input, "name").to_string()),
        "question" => {
            let first = input.get("questions").and_then(|q| q.as_array()).and_then(|q| q.first()).cloned().unwrap_or(Value::Null);
            let q = s_at(&first, "question");
            let q = if q.is_empty() { s_at(input, "question") } else { q };
            OpencodeDescribed::plain(tool, "Asked you", first_line(q, 80))
        }
        _ => OpencodeDescribed::plain(tool, "Used", word_space_re().replace_all(tool, " ").to_string()),
    }
}

/// opencode ids start with a time: "prt_0b55a7148001…". Their tails sort in time order.
pub fn order(id: &str) -> &str {
    match id.split_once('_') {
        Some((prefix, rest)) if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_lowercase()) => rest,
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unified_diff_skips_headers_and_marks_hunks() {
        let d = unified_diff("--- a\n+++ b\n@@ -1 +1 @@\n-a\n+b\n@@ -9 +9 @@\n c\n");
        let signs: Vec<(&str, &str)> = d.iter().map(|l| {
            let sign = match l.sign {
                DiffSign::Same => " ",
                DiffSign::Added => "+",
                DiffSign::Removed => "-",
            };
            (sign, l.text.as_str())
        }).collect();
        assert_eq!(signs, vec![("-", "a"), ("+", "b"), (" ", "\u{22ef}"), (" ", "c")]);
    }

    #[test]
    fn names_tools_like_a_person() {
        let empty = json!({});
        let d = describe_opencode_tool("read", &json!({ "filePath": "D:/a/shot.png" }), &empty);
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Looked at", "shot.png"));
        let d = describe_opencode_tool("grep", &json!({ "pattern": "TODO" }), &empty);
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Searched for", "TODO"));
        let d = describe_opencode_tool("webfetch", &json!({ "url": "https://docs.rs/x" }), &empty);
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Read", "docs.rs"));
        let d = describe_opencode_tool("task", &json!({ "description": "Write the tests" }), &empty);
        assert_eq!(d.target, "Write the tests");
        let d = describe_opencode_tool("apply_patch", &json!({ "patchText": "*** Begin Patch\n*** Add File: D:/a/n.ts\n+x\n*** Update File: D:/a/o.ts\n*** End Patch" }), &empty);
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Edited", "2 files"));
        let d = describe_opencode_tool("mcp_thing", &json!({}), &empty);
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Used", "mcp thing"));
    }
}
