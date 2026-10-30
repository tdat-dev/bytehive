//! The pure pieces of the Codex parser: your words out of a message, the
//! patch format, what a JS cell runs, how a tool call reads, a result's text.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::codex_cell::{cell_info, parse_args, view_image_path};
use crate::text::{base_name, s, s_at, MAX_IMAGES, MAX_OUTPUT};
use crate::types::{ChatImage, Todo, TodoState};

/// The first line, shortened with an ellipsis past `max` chars.
pub fn first_line(text: &str, max: usize) -> String {
    let line = text.trim().split(['\r', '\n']).next().unwrap_or("");
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.chars().count() > max {
        let mut out: String = line.chars().take(max.saturating_sub(1)).collect();
        out.push('\u{2026}');
        out
    } else {
        line.to_string()
    }
}

/// Tool output, trimmed: trailing blankness gone, capped with an ellipsis.
pub fn trim_output(text: &str) -> String {
    let t = text.trim_end();
    if t.chars().count() > MAX_OUTPUT {
        let mut out: String = t.chars().take(MAX_OUTPUT).collect();
        out.push('\n');
        out.push('\u{2026}');
        out
    } else {
        t.to_string()
    }
}

fn picture_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^data:(image/[a-z]+);base64,([A-Za-z0-9+/=\s]+)$").unwrap())
}

/// A picture from a data: URL, as the chat keeps pictures.
pub fn picture_of(url: &str) -> Option<ChatImage> {
    let m = picture_re().captures(url)?;
    let media = m[1].to_lowercase();
    match media.as_str() {
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" => Some(ChatImage { media, data: m[2].to_string() }),
        _ => None,
    }
}

fn injected_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(<(environment_context|user_instructions|recommended_plugins|subagent_notification|turn_aborted|app-context|permissions|collaboration_mode|user_shell_command|INSTRUCTIONS)\b|</?image\b|# AGENTS\.md instructions)").unwrap()
    })
}

fn ide_preamble_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*# (Context from my IDE setup|Files mentioned by the user)").unwrap())
}

fn my_request_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)## My request[^\n]*:[ \t]*\r?\n(.*)$").unwrap())
}

/// What you typed, out of a user message's parts (and the pictures you pasted).
pub fn user_words(content: &Value) -> (String, Vec<ChatImage>) {
    let mut words: Vec<String> = Vec::new();
    let mut pics: Vec<ChatImage> = Vec::new();
    let parts = content.as_array().cloned().unwrap_or_default();
    for c in &parts {
        if s_at(c, "type") == "input_image" {
            if let Some(p) = picture_of(s_at(c, "image_url")) {
                pics.push(p);
            }
            continue;
        }
        let kind = s_at(c, "type");
        if kind != "input_text" && kind != "text" {
            continue;
        }
        let mut t = s_at(c, "text").to_string();
        if injected_re().is_match(&t) {
            continue;
        }
        // The IDE and pasted files come first; your words follow "## My request…:".
        if ide_preamble_re().is_match(&t) {
            match my_request_re().captures(&t) {
                Some(m) => t = m[1].to_string(),
                None => continue,
            }
        }
        t = t.trim().to_string();
        if !t.is_empty() {
            words.push(t);
        }
    }
    if pics.len() > MAX_IMAGES {
        pics = pics.split_off(pics.len() - MAX_IMAGES);
    }
    (words.join("\n"), pics)
}

/// How a Codex tool call reads in the chat: a verb and what it touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    pub tool: String,
    pub verb: String,
    pub target: String,
    pub full: Option<String>,
    pub code: bool,
    pub todos: Option<Vec<Todo>>,
}

impl Described {
    fn plain(tool: &str, verb: &str, target: String) -> Described {
        Described { tool: tool.to_string(), verb: verb.to_string(), target, full: None, code: false, todos: None }
    }
}

fn shell_flag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^-(c|lc|Command)$").unwrap())
}

fn word_space_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[_-]+").unwrap())
}

pub fn describe_codex_tool(name: &str, input: &Value, namespace: &str) -> Described {
    let a = parse_args(input);
    match name {
        "shell_command" | "exec_command" => {
            let cmd = if !s_at(&a, "command").is_empty() { s_at(&a, "command") } else { s_at(&a, "cmd") };
            Described { tool: name.to_string(), verb: "Ran".to_string(), target: first_line(cmd, 120), full: Some(cmd.to_string()), code: true, todos: None }
        }
        "shell" | "local_shell" => {
            let argv: Vec<String> = match a.get("command") {
                Some(Value::Array(items)) => items.iter().map(s).map(str::to_string).collect(),
                _ => vec![s_at(&a, "command").to_string()],
            };
            // ["powershell.exe", "-Command", "…"] / ["bash", "-lc", "…"]: the script is the last part.
            let cmd = if argv.len() > 2 && shell_flag_re().is_match(&argv[argv.len() - 2]) {
                argv[argv.len() - 1].clone()
            } else {
                argv.join(" ")
            };
            Described { tool: name.to_string(), verb: "Ran".to_string(), target: first_line(&cmd, 120), full: Some(cmd), code: true, todos: None }
        }
        "exec" | "functions__exec" | "js" => {
            let code = match input {
                Value::String(text) => text.clone(),
                _ => {
                    let c = s_at(&a, "code");
                    if !c.is_empty() { c.to_string() } else { s_at(&a, "input").to_string() }
                }
            };
            let cell = cell_info(&code);
            if !cell.commands.is_empty() {
                let more = if cell.commands.len() > 1 { format!(" (+{} more)", cell.commands.len() - 1) } else { String::new() };
                return Described {
                    tool: name.to_string(),
                    verb: "Ran".to_string(),
                    target: format!("{}{}", first_line(&cell.commands[0], 120), more),
                    full: Some(cell.commands.join("\n")),
                    code: true,
                    todos: None,
                };
            }
            if cell.tools.contains(&"update_plan".to_string()) {
                return Described::plain(name, "Updated the plan", String::new());
            }
            if cell.tools.contains(&"view_image".to_string()) {
                let file = view_image_path(&code).unwrap_or_default();
                return Described {
                    tool: name.to_string(),
                    verb: "Looked at".to_string(),
                    target: if file.is_empty() { "an image".to_string() } else { base_name(&file).to_string() },
                    full: if file.is_empty() { None } else { Some(file.clone()) },
                    code: !file.is_empty(),
                    todos: None,
                };
            }
            Described {
                tool: name.to_string(),
                verb: "Ran a script".to_string(),
                target: first_line(&code, 80),
                full: Some(code),
                code: true,
                todos: None,
            }
        }
        "write_stdin" => {
            let session = match a.get("session_id") {
                Some(Value::String(id)) => id.clone(),
                Some(Value::Null) | None => String::new(),
                Some(other) => match other {
                    Value::String(id) => id.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => String::new(),
                },
            };
            Described::plain(name, "Checked on", format!("the command running as session {session}").trim().to_string())
        }
        "wait" => Described::plain(name, "Waited for", "a running script".to_string()),
        "update_plan" => {
            let todos: Vec<Todo> = a
                .get("plan")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|t| Todo { text: s_at(t, "step").to_string(), state: TodoState::parse(s_at(t, "status")) })
                .collect();
            let done = todos.iter().filter(|t| t.state == TodoState::Completed).count();
            Described {
                tool: name.to_string(),
                verb: "Updated the plan".to_string(),
                target: format!("{done} of {} done", todos.len()),
                full: None,
                code: false,
                todos: Some(todos),
            }
        }
        "view_image" => {
            let p = s_at(&a, "path");
            Described {
                tool: name.to_string(),
                verb: "Looked at".to_string(),
                target: base_name(p).to_string(),
                full: Some(p.to_string()),
                code: true,
                todos: None,
            }
        }
        "spawn_agent" | "spawn_agents" => {
            let task = s_at(&a, "task_name");
            let task = if task.is_empty() { s_at(&a, "nickname") } else { task };
            Described::plain(name, "Asked a helper agent to", if task.is_empty() { "help".to_string() } else { task.to_string() })
        }
        "wait_agent" | "send_message" | "send_input" | "list_agents" | "close_agent" | "interrupt_agent" | "followup_task" => {
            Described::plain(name, "Talked to", "its helper agents".to_string())
        }
        "request_user_input" => {
            let q = s_at(&a, "question");
            let q = if q.is_empty() { s_at(&a, "prompt") } else { q };
            Described::plain(name, "Asked you", first_line(if q.is_empty() { "a question" } else { q }, 120))
        }
        "tool_search" => Described::plain(name, "Loaded tools", first_line(s_at(&a, "query"), 60)),
        _ => {
            let bare = namespace.strip_prefix("mcp__").unwrap_or(namespace);
            let bare = bare.strip_suffix("__").unwrap_or(bare);
            let place = word_space_re().replace_all(bare, " ");
            let place = place.trim();
            let plain = name.replace('_', " ");
            let target = if place.is_empty() { plain } else { format!("{place} \u{b7} {plain}") };
            Described::plain(name, "Used", target)
        }
    }
}

/// A tool result's text and pictures, from a string, a JSON envelope or content parts.
pub struct CallResult {
    pub text: String,
    pub pics: Vec<ChatImage>,
    pub exit: Option<i64>,
    pub running: Option<String>,
}

fn exit_code_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)(?:Process exited with code|^Exit code:)\s*(-?\d+)").unwrap())
}

fn script_error_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^Script error:").unwrap())
}

fn running_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"Process running with session ID (\d+)").unwrap())
}

fn header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(Script completed|Script running.*|Chunk ID: \S+|Wall time:? [\d.]+ seconds|Exit code: -?\d+|Process exited with code -?\d+|Process running with session ID \d+|Original token count: \d+|Output:)\s*$").unwrap()
    })
}

fn leading_blank_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)^\s*\n").unwrap())
}

pub fn result_of(output: &Value) -> CallResult {
    let mut text = String::new();
    let mut pics: Vec<ChatImage> = Vec::new();
    let mut exit: Option<i64> = None;
    if let Some(parts) = output.as_array() {
        for c in parts {
            if s_at(c, "type") == "input_image" {
                if let Some(p) = picture_of(s_at(c, "image_url")) {
                    pics.push(p);
                }
            } else if let Some(t) = c.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
    } else {
        text = s(output).to_string();
        if text.trim_start().starts_with('{') {
            // apply_patch answers {"output": "…", "metadata": {"exit_code": 0}}
            if let Ok(o) = serde_json::from_str::<Value>(&text) {
                if let Some(inner) = o.get("output").and_then(|o| o.as_str()) {
                    text = inner.to_string();
                    if let Some(code) = o.get("metadata").and_then(|m| m.get("exit_code")).and_then(|c| c.as_i64()) {
                        exit = Some(code);
                    }
                }
            }
        }
    }
    // A cell's exit codes: any failing command makes the step a failure.
    for m in exit_code_re().captures_iter(&text.clone()) {
        let code: i64 = m[1].parse().unwrap_or(0);
        if exit.is_none() || code != 0 {
            exit = Some(code);
        }
    }
    if exit.is_none() && script_error_re().is_match(&text) {
        exit = Some(1);
    }
    // "Process running with session ID 43354": a command still going on its own.
    let running = running_re().captures(&text).map(|m| m[1].to_string());
    // The headers Codex puts around command output (a cell nests one per command): keep what was printed.
    let kept: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).filter(|l| !header_re().is_match(l)).collect();
    text = leading_blank_re().replace(&kept.join("\n"), "").to_string();
    if pics.len() > MAX_IMAGES {
        pics = pics.split_off(pics.len() - MAX_IMAGES);
    }
    CallResult { text, pics, exit, running }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_words_keeps_only_yours_with_pictures() {
        let content = json!([
            { "type": "input_text", "text": "\n# Files mentioned by the user:\n\n## shot.png: C:/t/shot.png\n\n## My request:\nmake it blue\n" },
            { "type": "input_text", "text": "<image name=\"shot\">" },
            { "type": "input_image", "image_url": "data:image/png;base64,iVBORw0KGgo=" },
            { "type": "input_text", "text": "</image>" },
        ]);
        let (text, pics) = user_words(&content);
        assert_eq!(text, "make it blue");
        assert_eq!(pics, vec![ChatImage { media: "image/png".to_string(), data: "iVBORw0KGgo=".to_string() }]);
        let ide = json!([{ "type": "input_text", "text": "# Context from my IDE setup:\n\n## Open tabs:\n- a.ts\n\n## My request for Codex:\nrename it" }]);
        assert_eq!(user_words(&ide).0, "rename it");
        let injected = json!([{ "type": "input_text", "text": "<recommended_plugins>\n- x\n</recommended_plugins>" }]);
        assert_eq!(user_words(&injected).0, "");
    }

    #[test]
    fn describe_names_cells_shell_and_mcp_plainly() {
        let cell = json!("const r = await tools.exec_command({\"cmd\":\"git status\",\"workdir\":\"D:\\\\app\"});");
        let d = describe_codex_tool("exec", &cell, "");
        assert_eq!(d.verb, "Ran");
        assert_eq!(d.target, "git status");
        let shell = json!({ "command": ["bash", "-lc", "ls -la"] });
        let d = describe_codex_tool("shell", &shell, "");
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Ran", "ls -la"));
        let d = describe_codex_tool("take_snapshot", &json!({}), "mcp__chrome_devtools__");
        assert_eq!(d.verb, "Used");
        assert_eq!(d.target, "chrome devtools \u{b7} take snapshot");
    }

    #[test]
    fn result_strips_headers_and_reads_exit() {
        let r = result_of(&json!("Exit code: 0\nWall time: 2.1 seconds\nOutput:\n12 passed\n"));
        assert_eq!(trim_output(&r.text), "12 passed");
        assert_eq!(r.exit, Some(0));
        let r = result_of(&json!("Chunk ID: a1\nWall time: 1.0 seconds\nProcess exited with code 1\nOriginal token count: 3\nOutput:\nlint error\n"));
        assert_eq!(trim_output(&r.text), "lint error");
        assert_eq!(r.exit, Some(1));
        let r = result_of(&json!("Process running with session ID 43354\nOutput:\nready\n"));
        assert_eq!(trim_output(&r.text), "ready");
        assert_eq!(r.running, Some("43354".to_string()));
    }
}
