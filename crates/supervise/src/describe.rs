//! How tool calls read in the chat, and background-task notices.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::text::{base_name, first_line, first_line_default, line_diff, s_at, url_host, MAX_DIFF_LINES};
use crate::types::{AskedOption, AskedQuestion, BgState, ChatImage, DiffLine, Todo, TodoState};

/// How a tool call reads in the chat: a verb, what it touched, and extras.
#[derive(Debug, Clone, Default)]
pub struct StepDesc {
    pub tool: String,
    pub verb: String,
    pub target: String,
    pub full: Option<String>,
    pub code: bool,
    pub diff: Option<Vec<DiffLine>>,
    pub added: Option<u32>,
    pub removed: Option<u32>,
    pub todos: Option<Vec<Todo>>,
    pub questions: Option<Vec<AskedQuestion>>,
    pub images: Option<Vec<ChatImage>>,
}

fn mcp_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^mcp__(.+?)__(.+)$").unwrap())
}

fn shot_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)screenshot|snapshot").unwrap())
}

fn plugin_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^plugin_[^_]+_").unwrap())
}

fn word_sep_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[_-]+").unwrap())
}

fn image_ext_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\.(png|jpe?g|gif|webp|bmp)$").unwrap())
}

pub fn describe_tool(name: &str, input: &Value) -> StepDesc {
    let file = {
        let f = s_at(input, "file_path");
        if !f.is_empty() {
            f
        } else {
            let f = s_at(input, "notebook_path");
            if !f.is_empty() {
                f
            } else {
                s_at(input, "path")
            }
        }
    }
    .to_string();
    match name {
        "Bash" | "PowerShell" => {
            let cmd = s_at(input, "command");
            let said = s_at(input, "description");
            StepDesc {
                tool: name.to_string(),
                verb: "Ran".to_string(),
                target: if said.is_empty() { first_line_default(cmd) } else { said.to_string() },
                full: Some(cmd.to_string()),
                code: said.is_empty(),
                ..Default::default()
            }
        }
        "AskUserQuestion" => {
            let questions: Vec<AskedQuestion> = input
                .get("questions")
                .and_then(|q| q.as_array())
                .map(|qs| {
                    qs.iter()
                        .map(|q| AskedQuestion {
                            question: s_at(q, "question").to_string(),
                            header: {
                                let h = s_at(q, "header");
                                if h.is_empty() {
                                    None
                                } else {
                                    Some(h.to_string())
                                }
                            },
                            multi: q.get("multiSelect").and_then(|m| m.as_bool()) == Some(true),
                            options: q
                                .get("options")
                                .and_then(|o| o.as_array())
                                .map(|os| {
                                    os.iter()
                                        .map(|o| AskedOption {
                                            label: s_at(o, "label").to_string(),
                                            description: {
                                                let d = s_at(o, "description");
                                                if d.is_empty() {
                                                    None
                                                } else {
                                                    Some(d.to_string())
                                                }
                                            },
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            answer: None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let target = match questions.first() {
                Some(first) => first.header.clone().unwrap_or_else(|| first_line_default(&first.question)),
                None => "a question".to_string(),
            };
            StepDesc { tool: name.to_string(), verb: "Asked you".to_string(), target, questions: Some(questions), ..Default::default() }
        }
        "Monitor" => {
            let cmd = s_at(input, "command");
            let said = s_at(input, "description");
            StepDesc {
                tool: name.to_string(),
                verb: "Watched".to_string(),
                target: if said.is_empty() { first_line_default(cmd) } else { said.to_string() },
                full: if cmd.is_empty() { None } else { Some(cmd.to_string()) },
                ..Default::default()
            }
        }
        "Read" => StepDesc {
            tool: name.to_string(),
            verb: if image_ext_re().is_match(&file) { "Looked at".to_string() } else { "Read".to_string() },
            target: base_name(&file).to_string(),
            full: Some(file),
            code: true,
            ..Default::default()
        },
        "Edit" => {
            let diff: Vec<DiffLine> = line_diff(s_at(input, "old_string"), s_at(input, "new_string")).into_iter().take(MAX_DIFF_LINES).collect();
            let (added, removed) = crate::text::count_diff(&diff);
            StepDesc {
                tool: name.to_string(),
                verb: "Edited".to_string(),
                target: base_name(&file).to_string(),
                full: Some(file),
                code: true,
                diff: Some(diff),
                added: Some(added),
                removed: Some(removed),
                ..Default::default()
            }
        }
        "MultiEdit" => {
            let edits = input.get("edits").and_then(|e| e.as_array()).cloned().unwrap_or_default();
            let mut diff = Vec::new();
            for (k, e) in edits.iter().enumerate() {
                if k > 0 {
                    diff.push(DiffLine { sign: crate::types::DiffSign::Same, text: "⋯".to_string() });
                }
                diff.extend(line_diff(s_at(e, "old_string"), s_at(e, "new_string")));
            }
            diff.truncate(MAX_DIFF_LINES);
            let (added, removed) = crate::text::count_diff(&diff);
            StepDesc {
                tool: name.to_string(),
                verb: "Edited".to_string(),
                target: base_name(&file).to_string(),
                full: Some(file),
                code: true,
                diff: Some(diff),
                added: Some(added),
                removed: Some(removed),
                ..Default::default()
            }
        }
        "Write" => {
            let content = s_at(input, "content");
            let diff: Vec<DiffLine> = content
                .split('\n')
                .map(|l| l.strip_suffix('\r').unwrap_or(l))
                .take(MAX_DIFF_LINES)
                .map(|text| DiffLine { sign: crate::types::DiffSign::Added, text: text.to_string() })
                .collect();
            let added = if content.is_empty() { 0 } else { content.split('\n').count() as u32 };
            StepDesc {
                tool: name.to_string(),
                verb: "Wrote".to_string(),
                target: base_name(&file).to_string(),
                full: Some(file),
                code: true,
                diff: Some(diff),
                added: Some(added),
                removed: Some(0),
                ..Default::default()
            }
        }
        "NotebookEdit" => StepDesc {
            tool: name.to_string(),
            verb: "Edited".to_string(),
            target: base_name(&file).to_string(),
            full: Some(file),
            code: true,
            ..Default::default()
        },
        "Grep" => StepDesc {
            tool: name.to_string(),
            verb: "Searched for".to_string(),
            target: s_at(input, "pattern").to_string(),
            code: true,
            full: {
                let p = s_at(input, "path");
                if p.is_empty() {
                    None
                } else {
                    Some(p.to_string())
                }
            },
            ..Default::default()
        },
        "Glob" => StepDesc {
            tool: name.to_string(),
            verb: "Looked for files".to_string(),
            target: s_at(input, "pattern").to_string(),
            code: true,
            ..Default::default()
        },
        "LS" => StepDesc {
            tool: name.to_string(),
            verb: "Listed".to_string(),
            target: base_name(&file).to_string(),
            full: Some(file),
            code: true,
            ..Default::default()
        },
        "WebSearch" => StepDesc {
            tool: name.to_string(),
            verb: "Searched the web for".to_string(),
            target: s_at(input, "query").to_string(),
            ..Default::default()
        },
        "WebFetch" => {
            let url = s_at(input, "url");
            StepDesc {
                tool: name.to_string(),
                verb: "Read".to_string(),
                target: url_host(url),
                full: Some(url.to_string()),
                ..Default::default()
            }
        }
        "Task" | "Agent" => {
            let said = s_at(input, "description");
            StepDesc {
                tool: name.to_string(),
                verb: "Asked a helper agent to".to_string(),
                target: if said.is_empty() { first_line_default(s_at(input, "prompt")) } else { said.to_string() },
                ..Default::default()
            }
        }
        "TodoWrite" => {
            let todos: Vec<Todo> = input
                .get("todos")
                .and_then(|t| t.as_array())
                .map(|ts| {
                    ts.iter()
                        .map(|t| {
                            let text = s_at(t, "content");
                            let text = if text.is_empty() { s_at(t, "activeForm") } else { text };
                            Todo { text: text.to_string(), state: TodoState::parse(s_at(t, "status")) }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let done = todos.iter().filter(|t| t.state == TodoState::Completed).count();
            StepDesc {
                tool: name.to_string(),
                verb: "Updated the plan".to_string(),
                target: format!("{done} of {} done", todos.len()),
                todos: Some(todos),
                ..Default::default()
            }
        }
        "Skill" => {
            let target = s_at(input, "skill");
            let target = if target.is_empty() { s_at(input, "command") } else { target };
            StepDesc { tool: name.to_string(), verb: "Used the skill".to_string(), target: target.to_string(), ..Default::default() }
        }
        "ToolSearch" => StepDesc {
            tool: name.to_string(),
            verb: "Loaded tools".to_string(),
            target: first_line(s_at(input, "query"), 60),
            ..Default::default()
        },
        "ExitPlanMode" => StepDesc { tool: name.to_string(), verb: "Proposed a plan".to_string(), target: String::new(), ..Default::default() },
        other => {
            if let Some(m) = mcp_re().captures(other) {
                let server = &m[1];
                let method = &m[2];
                if s_at(input, "action") == "screenshot" || shot_re().is_match(method) {
                    let target = word_sep_re().replace_all(plugin_re().replace_all(server, "").as_ref(), " ").to_string();
                    return StepDesc { tool: name.to_string(), verb: "Took a screenshot".to_string(), target, ..Default::default() };
                }
                let target = format!("{} · {}", word_sep_re().replace_all(server, " "), method.replace('_', " "));
                return StepDesc { tool: name.to_string(), verb: "Used".to_string(), target, ..Default::default() };
            }
            StepDesc { tool: name.to_string(), verb: "Used".to_string(), target: name.to_string(), ..Default::default() }
        }
    }
}

/// A CLI's word for how a background task ended, as ours.
pub fn task_state(status: &str) -> BgState {
    let s = status.to_lowercase();
    if s.contains("run") || s.contains("start") || s.contains("pending") {
        BgState::Running
    } else if s.contains("fail") || s.contains("error") {
        BgState::Failed
    } else if ["stop", "kill", "cancel", "abort", "interrupt"].iter().any(|w| s.contains(w)) {
        BgState::Stopped
    } else {
        BgState::Done
    }
}

pub struct TaskNotice {
    pub id: String,
    pub state: BgState,
    pub output: Option<String>,
    pub summary: Option<String>,
}

/// Claude Code's <task-notification> text: which tasks, how they ended, where the output is.
pub fn parse_task_notice(raw: &str) -> Vec<TaskNotice> {
    if !raw.contains("<task-notification>") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for block in raw.split("<task-notification>").skip(1) {
        let tag = |n: &str| -> String {
            let open = format!("<{n}>");
            let close = format!("</{n}>");
            let a = match block.find(&open) {
                Some(i) => i,
                None => return String::new(),
            };
            let b = match block[a..].find(&close) {
                Some(i) => a + i,
                None => return String::new(),
            };
            block[a + open.len()..b].trim().to_string()
        };
        let status = tag("status");
        if status.is_empty() {
            continue;
        }
        let output = {
            let o = tag("output-file");
            if o.is_empty() {
                None
            } else {
                Some(o)
            }
        };
        let summary = {
            let x = tag("summary");
            if x.is_empty() {
                None
            } else {
                Some(x)
            }
        };
        // One notice can name several tasks.
        let mut rest = block;
        while let Some(a) = rest.find("<task-id>") {
            let after = &rest[a + 9..];
            let Some(b) = after.find("</task-id>") else { break };
            let id = after[..b].trim().to_string();
            rest = &after[b + 10..];
            if id.is_empty() || id.contains('<') || id.starts_with("__") {
                continue;
            }
            out.push(TaskNotice { id, state: task_state(&status), output: output.clone(), summary: summary.clone() });
        }
    }
    out
}

/// Stopping a background command by hand.
pub fn is_task_stop(name: &str) -> bool {
    name == "TaskStop" || name == "KillShell" || name == "KillBash"
}
