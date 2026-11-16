//! A Codex rollout (`~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`) as chat
//! items for the Remote store: what you typed, its replies, one line per step
//! and whether it failed. The slim cousin of src/codexmodel.ts (the old app's
//! full reader): no diffs, outputs, pictures or side panel.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::transcript::Item;

/// Context Codex puts in a user message that you did not type.
static INJECTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(<(environment_context|user_instructions|recommended_plugins|subagent_notification|turn_aborted|app-context|permissions|collaboration_mode|user_shell_command|INSTRUCTIONS)\b|</?image\b|# AGENTS\.md instructions)").unwrap()
});
static IDE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*# (Context from my IDE setup|Files mentioned by the user)").unwrap());
static REQUEST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"## My request[^\n]*:[ \t]*\r?\n([\s\S]*)$").unwrap());
static PATCH_FILE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\*\*\* (Add|Update|Delete) File: (.+)$").unwrap());
static CELL_CMD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\b(?:cmd|command)["']?\s*:\s*("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`)"#).unwrap());
static EXIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)(?:Process exited with code|^Exit code:)\s*(-?\d+)").unwrap());

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn first_line(t: &str, max: usize) -> String {
    let line = t.trim().lines().next().unwrap_or("");
    if line.chars().count() <= max {
        return line.to_string();
    }
    format!("{}\u{2026}", line.chars().take(max - 1).collect::<String>())
}

fn base_name(p: &str) -> String {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// A JS string literal's value: "...", '...' or `...`.
fn literal(lit: &str) -> String {
    if lit.starts_with('"') && let Ok(Value::String(v)) = serde_json::from_str(lit) {
        return v;
    }
    lit.get(1..lit.len().saturating_sub(1)).unwrap_or("").replace("\\n", "\n").replace("\\\"", "\"").replace("\\'", "'").replace("\\`", "`")
}

/// What you typed, out of a user message's parts.
fn user_words(content: &Value) -> String {
    let mut words = Vec::new();
    for c in content.as_array().into_iter().flatten() {
        if !matches!(s(c, "type"), "input_text" | "text") {
            continue;
        }
        let mut t = s(c, "text").to_string();
        if INJECTED.is_match(&t) {
            continue;
        }
        if IDE.is_match(&t) {
            let Some(m) = REQUEST.captures(&t) else { continue };
            t = m[1].to_string();
        }
        let t = t.trim();
        if !t.is_empty() {
            words.push(t.to_string());
        }
    }
    words.join("\n")
}

fn args_of(v: &Value) -> Value {
    match v {
        Value::String(text) => serde_json::from_str(text).unwrap_or(Value::Null),
        other => other.clone(),
    }
}

/// One step per file of a patch, as (verb, file).
fn patch_steps(patch: &str) -> Vec<(&'static str, String)> {
    PATCH_FILE
        .captures_iter(patch)
        .map(|m| {
            let verb = match &m[1] {
                "Add" => "Wrote",
                "Delete" => "Deleted",
                _ => "Edited",
            };
            (verb, base_name(m[2].trim()))
        })
        .collect()
}

/// How one call reads: (verb, target, target is code), one per file for a patch.
fn describe(name: &str, input: &Value) -> Vec<(String, String, bool)> {
    let a = args_of(input);
    let ran = |cmd: &str| vec![("Ran".to_string(), first_line(cmd, 120), true)];
    let patched = |patch: &str| {
        let files = patch_steps(patch);
        if files.is_empty() { vec![("Edited".to_string(), "files".to_string(), false)] } else { files.into_iter().map(|(v, f)| (v.to_string(), f, true)).collect() }
    };
    match name {
        "shell_command" | "exec_command" => ran(if s(&a, "command").is_empty() { s(&a, "cmd") } else { s(&a, "command") }),
        "shell" | "local_shell" => {
            let argv: Vec<String> = match a.get("command") {
                Some(Value::Array(v)) => v.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect(),
                Some(Value::String(c)) => vec![c.clone()],
                _ => Vec::new(),
            };
            let n = argv.len();
            let script = if n > 2 && ["-c", "-lc", "-command"].contains(&argv[n - 2].to_ascii_lowercase().as_str()) { argv[n - 1].clone() } else { argv.join(" ") };
            ran(&script)
        }
        "apply_patch" => patched(input.as_str().map(str::to_string).unwrap_or_else(|| s(&a, "input").to_string()).as_str()),
        "exec" | "functions__exec" | "js" => {
            let code = input.as_str().map(str::to_string).unwrap_or_else(|| s(&a, "code").to_string());
            if code.contains("*** Begin Patch") {
                return patched(&code.replace("\\n", "\n"));
            }
            let cmds: Vec<String> = CELL_CMD.captures_iter(&code).map(|m| literal(&m[1])).collect();
            match cmds.first() {
                Some(c) => {
                    let more = if cmds.len() > 1 { format!(" (+{} more)", cmds.len() - 1) } else { String::new() };
                    vec![("Ran".into(), first_line(c, 120) + &more, true)]
                }
                None if code.contains("tools.update_plan") => vec![("Updated the plan".into(), String::new(), false)],
                None => vec![("Ran a script".into(), first_line(&code, 80), true)],
            }
        }
        "update_plan" => vec![("Updated the plan".into(), String::new(), false)],
        "view_image" => vec![("Looked at".into(), base_name(s(&a, "path")), true)],
        "spawn_agent" | "spawn_agents" => vec![("Asked a helper agent to".into(), s(&a, "task_name").to_string(), false)],
        "request_user_input" => vec![("Asked you".into(), first_line(if s(&a, "question").is_empty() { "a question" } else { s(&a, "question") }, 120), false)],
        "write_stdin" | "wait" => vec![("Checked on".into(), "a running command".into(), false)],
        _ => vec![("Used".into(), name.replace('_', " "), false)],
    }
}

/// Whether a call's output says it failed.
fn failed(output: &Value) -> bool {
    let mut text = match output {
        Value::String(t) => t.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    };
    if text.trim_start().starts_with('{')
        && let Ok(o) = serde_json::from_str::<Value>(&text)
    {
        if let Some(code) = o.pointer("/metadata/exit_code").and_then(Value::as_i64) {
            return code != 0;
        }
        text = s(&o, "output").to_string();
    }
    EXIT.captures_iter(&text).any(|m| m[1].parse::<i64>().is_ok_and(|c| c != 0))
        || text.starts_with("Script error:")
        || text.trim_start().to_ascii_lowercase().starts_with("apply_patch verification failed")
}

/// The whole rollout as chat items.
pub fn items(text: &str) -> Vec<Item> {
    let mut out = Vec::new();
    let mut calls: HashMap<String, Vec<usize>> = HashMap::new();
    let mut n = 0;
    let mut id = || {
        n += 1;
        format!("x{}", n - 1)
    };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if s(&v, "type") != "response_item" {
            continue;
        }
        let Some(p) = v.get("payload") else { continue };
        match s(p, "type") {
            "message" => match s(p, "role") {
                "user" => {
                    let words = user_words(&p["content"]);
                    if !words.is_empty() {
                        out.push(Item::User { id: id(), text: words });
                    }
                }
                "assistant" => {
                    for c in p["content"].as_array().into_iter().flatten() {
                        let t = s(c, "text").trim();
                        if matches!(s(c, "type"), "output_text" | "text") && !t.is_empty() {
                            out.push(Item::Text { id: id(), text: t.to_string() });
                        }
                    }
                }
                _ => {}
            },
            kind @ ("function_call" | "custom_tool_call" | "local_shell_call") => {
                let (name, input) = if kind == "local_shell_call" {
                    ("local_shell", serde_json::json!({ "command": p.pointer("/action/command").cloned().unwrap_or(Value::Null) }))
                } else {
                    (s(p, "name"), if kind == "custom_tool_call" { p["input"].clone() } else { p["arguments"].clone() })
                };
                let mut at = Vec::new();
                for (verb, target, code) in describe(name, &input) {
                    at.push(out.len());
                    out.push(Item::Step { id: id(), verb, target, error: false, done: false, code });
                }
                calls.insert(s(p, "call_id").to_string(), at);
            }
            "function_call_output" | "custom_tool_call_output" => {
                let bad = failed(&p["output"]);
                for &i in calls.get(s(p, "call_id")).into_iter().flatten() {
                    if let Some(Item::Step { error, done, .. }) = out.get_mut(i) {
                        *error = bad;
                        *done = true;
                    }
                }
            }
            "web_search_call" => {
                let q = p.pointer("/action/query").and_then(Value::as_str).unwrap_or("");
                out.push(Item::Step { id: id(), verb: "Searched the web for".into(), target: q.to_string(), error: false, done: true, code: false });
            }
            _ => {}
        }
    }
    out
}

/// A Codex session's items, from its rollout file under `~/.codex/sessions`.
pub fn read(session: &str) -> Option<Vec<Item>> {
    let root = agent_core::codex::store_root()?.join("sessions");
    let path = agent_core::codex::rollout_file(&root, session)?;
    std::fs::read_to_string(path).ok().map(|t| items(&t))
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
