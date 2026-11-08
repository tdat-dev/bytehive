//! Turns a Codex rollout into a conversation: your messages, its replies,
//! the steps it took with their results, and what the side panel needs.
//! Pure: feed it lines, read `items` and `meta`.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::codex::{describe_codex_tool, first_line, result_of, trim_output, user_words};
use crate::codex_cell::{cell_info, parse_args};
use crate::codex_patch::{parse_patch, PatchKind};
use crate::ledgers::{FileLedger, FileNote, TaskLedger, TaskNote};
use crate::text::{base_name, parse_ms, s, s_at};
use crate::types::{BgState, Chat, ChatItem, ChatMeta, NoteItem, StepItem, TextItem, UserItem};

/// Steps waiting for their output, by call id (a patch makes one step per file).
struct CallState {
    step_ids: Vec<String>,
    name: String,
    session: String,
}

pub struct CodexChat {
    items: Vec<ChatItem>,
    meta: ChatMeta,
    calls: HashMap<String, CallState>,
    files: FileLedger,
    tasks: TaskLedger,
    output_tokens: u64,
    n: usize,
}

fn failed_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(apply_patch verification failed|error:|write_stdin failed)").unwrap())
}

fn exec_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(exec|functions__exec|js)$").unwrap())
}

impl CodexChat {
    pub fn new() -> CodexChat {
        CodexChat { items: Vec::new(), meta: ChatMeta::default(), calls: HashMap::new(), files: FileLedger::new(), tasks: TaskLedger::new(), output_tokens: 0, n: 0 }
    }

    fn id(&mut self) -> String {
        let id = format!("x{}", self.n);
        self.n += 1;
        id
    }

    fn note_file(&mut self, c: FileNote) {
        let turn = self.meta.turn;
        self.files.note(&mut self.meta, turn, c);
    }

    fn note_task(&mut self, t: TaskNote) {
        self.tasks.note(&mut self.meta, t);
    }

    fn step_mut(&mut self, id: &str) -> Option<&mut StepItem> {
        self.items.iter_mut().find_map(|it| match it {
            ChatItem::Step(s) if s.id == id => Some(s),
            _ => None,
        })
    }

    /// Model, effort and approval policy, from turn_context or thread_settings_applied.
    fn settings(&mut self, s: &Value) {
        if let Some(model) = s.get("model").and_then(|m| m.as_str()) {
            if !model.is_empty() {
                self.meta.model = Some(model.to_string());
            }
        }
        let collab = s.get("collaboration_mode").cloned().unwrap_or(Value::Null);
        let in_collab = collab.get("settings").cloned().unwrap_or(Value::Null);
        let effort = s_at(s, "effort");
        let effort = if effort.is_empty() { s_at(s, "reasoning_effort") } else { effort };
        let effort = if effort.is_empty() { s_at(&in_collab, "reasoning_effort") } else { effort };
        if !effort.is_empty() {
            self.meta.effort = Some(effort.to_string());
        }
        if s_at(&collab, "mode") == "plan" {
            self.meta.permission = Some("plan".to_string());
        } else if !s_at(s, "approval_policy").is_empty() {
            self.meta.permission = Some(s_at(s, "approval_policy").to_string());
        }
    }

    fn on_user(&mut self, content: &Value, at: u64) {
        let (text, pics) = user_words(content);
        if text.is_empty() && pics.is_empty() {
            return;
        }
        self.meta.turn += 1;
        let id = self.id();
        self.items.push(ChatItem::User(UserItem { id, text, images: pics.len(), pics: if pics.is_empty() { None } else { Some(pics) }, at }));
    }

    fn on_assistant(&mut self, content: &Value, at: u64) {
        let parts = content.as_array().cloned().unwrap_or_default();
        for c in &parts {
            let kind = s_at(c, "type");
            let text = s_at(c, "text").trim().to_string();
            if (kind == "output_text" || kind == "text") && !text.is_empty() {
                let id = self.id();
                self.items.push(ChatItem::Text(TextItem { id, text, at }));
            }
        }
    }

    fn on_call(&mut self, p: &Value, at: u64) {
        let call_id = s_at(p, "call_id").to_string();
        let name = s_at(p, "name").to_string();
        let input = if s_at(p, "type") == "custom_tool_call" {
            p.get("input").cloned().unwrap_or(Value::Null)
        } else {
            p.get("arguments").cloned().unwrap_or(Value::Null)
        };
        let mut step_ids: Vec<String> = Vec::new();
        // A JS cell may apply a patch itself (tools.apply_patch(`*** Begin Patch…`)).
        let cell_patch = if exec_name_re().is_match(&name) {
            match &input {
                Value::String(code) if code.contains("apply_patch") => cell_info(code).patch,
                _ => None,
            }
        } else {
            None
        };
        if name == "apply_patch" || cell_patch.is_some() {
            let patch = match cell_patch {
                Some(p) => p,
                None => match &input {
                    Value::String(text) => text.clone(),
                    _ => {
                        let args = parse_args(&input);
                        let inner = s_at(&args, "input");
                        if !inner.is_empty() { inner.to_string() } else { s_at(&args, "patch").to_string() }
                    }
                },
            };
            for f in parse_patch(&patch) {
                let verb = match f.kind {
                    PatchKind::Add => "Wrote",
                    PatchKind::Delete => "Deleted",
                    PatchKind::Update => "Edited",
                };
                let id = self.id();
                step_ids.push(id.clone());
                self.items.push(ChatItem::Step(StepItem {
                    id: id.clone(),
                    tool: name.clone(),
                    verb: verb.to_string(),
                    target: base_name(&f.path).to_string(),
                    full: Some(f.path.clone()),
                    code: true,
                    output: None,
                    error: false,
                    done: false,
                    diff: Some(f.diff.clone()),
                    added: Some(f.added),
                    removed: Some(f.removed),
                    todos: None,
                    images: None,
                    questions: None,
                    at,
                }));
                self.note_file(FileNote {
                    path: f.path.clone(),
                    name: None,
                    added: f.added,
                    removed: f.removed,
                    is_new: f.kind == PatchKind::Add,
                    deleted: if f.kind == PatchKind::Delete { Some(true) } else { None },
                    step: Some(id),
                    at,
                });
            }
            if step_ids.is_empty() {
                let id = self.id();
                step_ids.push(id.clone());
                self.items.push(ChatItem::Step(StepItem {
                    id,
                    tool: name.clone(),
                    verb: "Edited".to_string(),
                    target: "files".to_string(),
                    full: None,
                    code: false,
                    output: None,
                    error: false,
                    done: false,
                    diff: None,
                    added: None,
                    removed: None,
                    todos: None,
                    images: None,
                    questions: None,
                    at,
                }));
            }
        } else {
            let d = describe_codex_tool(&name, &input, s_at(p, "namespace"));
            if d.todos.is_some() {
                self.meta.todos = d.todos.clone();
            }
            let id = self.id();
            step_ids.push(id.clone());
            self.items.push(ChatItem::Step(StepItem {
                id,
                tool: d.tool,
                verb: d.verb,
                target: d.target,
                full: d.full,
                code: d.code,
                output: None,
                error: false,
                done: false,
                diff: None,
                added: None,
                removed: None,
                todos: d.todos,
                images: None,
                questions: None,
                at,
            }));
        }
        let session = if name == "write_stdin" {
            match parse_args(&input).get("session_id") {
                Some(Value::String(id)) => id.clone(),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                _ => String::new(),
            }
        } else {
            String::new()
        };
        if !call_id.is_empty() {
            self.calls.insert(call_id, CallState { step_ids, name, session });
        }
    }

    fn on_output(&mut self, p: &Value, _at: u64) {
        let call = match self.calls.get(s_at(p, "call_id")) {
            Some(c) => c,
            None => return,
        };
        let empty = Value::Null;
        let output = p.get("output").unwrap_or(&empty);
        let r = result_of(output);
        let failed = matches!(r.exit, Some(code) if code != 0) || failed_re().is_match(r.text.trim());
        let finished = trim_output(&r.text);
        for id in call.step_ids.clone() {
            if let Some(s) = self.step_mut(&id) {
                s.output = Some(finished.clone());
                if !r.pics.is_empty() {
                    s.images = Some(r.pics.clone());
                }
                s.error = failed;
                s.done = true;
            }
        }
        // A command still going on its own is a background task until it exits.
        let call = match self.calls.get(s_at(p, "call_id")) {
            Some(c) => c,
            None => return,
        };
        let first_id = call.step_ids.first().cloned().unwrap_or_default();
        let (target, full, name, session) = {
            let first = self.items.iter().find_map(|it| match it {
                ChatItem::Step(s) if s.id == first_id => Some(s.clone()),
                _ => None,
            });
            match first {
                Some(f) => (f.target, f.full, call.name.clone(), call.session.clone()),
                None => return,
            }
        };
        if r.running.is_some() && name != "write_stdin" {
            let running = r.running.clone().unwrap_or_default();
            self.note_task(TaskNote {
                id: format!("s{running}"),
                label: Some(target),
                command: full,
                state: Some(BgState::Running),
                output: None,
                at: Some(_at),
            });
        } else if !session.is_empty() {
            if let Some(_running) = r.running {
                self.note_task(TaskNote { id: format!("s{session}"), state: Some(BgState::Running), ..Default::default() });
            } else if let Some(code) = r.exit {
                self.note_task(TaskNote {
                    id: format!("s{session}"),
                    state: Some(if code == 0 { BgState::Done } else { BgState::Failed }),
                    ..Default::default()
                });
            }
        }
    }
}

impl Default for CodexChat {
    fn default() -> CodexChat {
        CodexChat::new()
    }
}

impl Chat for CodexChat {
    fn items(&self) -> &[ChatItem] {
        &self.items
    }

    fn meta(&self) -> &ChatMeta {
        &self.meta
    }

    fn feed(&mut self, text: &str) {
        for line in text.split('\n') {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let empty = Value::Null;
            let p = v.get("payload").unwrap_or(&empty);
            let at = match v.get("timestamp") {
                Some(Value::String(raw)) => parse_ms(raw),
                _ => 0,
            };
            match s_at(&v, "type") {
                "turn_context" => self.settings(p),
                "compacted" => {
                    let id = self.id();
                    self.items.push(ChatItem::Note(NoteItem { id, text: "Conversation compacted".to_string(), at }));
                }
                "event_msg" => match s_at(p, "type") {
                    "thread_settings_applied" => self.settings(p.get("thread_settings").unwrap_or(&empty)),
                    "turn_aborted" => {
                        let id = self.id();
                        self.items.push(ChatItem::Note(NoteItem { id, text: "You stopped the agent".to_string(), at }));
                    }
                    "token_count" => {
                        let info = p.get("info").unwrap_or(&empty);
                        if let Some(last) = info.get("last_token_usage") {
                            if let Some(input) = last.get("input_tokens").and_then(|n| n.as_u64()) {
                                self.meta.context = input;
                            }
                        }
                        if let Some(window) = info.get("model_context_window").and_then(|n| n.as_u64()) {
                            if window > 0 {
                                self.meta.window = Some(window);
                            }
                        }
                        if let Some(total) = info.get("total_token_usage") {
                            if let Some(out) = total.get("output_tokens").and_then(|n| n.as_u64()) {
                                self.output_tokens = out;
                            }
                        }
                        self.meta.output = self.output_tokens;
                    }
                    "patch_apply_end" => {
                        if let Some(changes) = p.get("changes").and_then(|c| c.as_object()) {
                            for (path, c) in changes {
                                let mut kind = s_at(c, "type").to_string();
                                if kind.is_empty() {
                                    kind = c.as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_default();
                                }
                                if kind == "add" {
                                    self.note_file(FileNote { path: path.clone(), is_new: true, at, ..Default::default() });
                                } else if kind == "delete" {
                                    self.note_file(FileNote { path: path.clone(), deleted: Some(true), at, ..Default::default() });
                                }
                            }
                        }
                    }
                    _ => {}
                },
                "response_item" => {
                    if self.meta.started.is_none() && at != 0 {
                        self.meta.started = Some(at);
                    }
                    match s_at(p, "type") {
                        "message" => match s_at(p, "role") {
                            "user" => self.on_user(p.get("content").unwrap_or(&empty), at),
                            "assistant" => self.on_assistant(p.get("content").unwrap_or(&empty), at),
                            _ => {}
                        },
                        "function_call" | "custom_tool_call" => self.on_call(p, at),
                        "function_call_output" | "custom_tool_call_output" => self.on_output(p, at),
                        "local_shell_call" => {
                            let action = p.get("action").cloned().unwrap_or(Value::Null);
                            let command = action.get("command").cloned().unwrap_or(Value::Null);
                            let synth = serde_json::json!({ "type": "function_call", "call_id": s_at(p, "call_id"), "name": "local_shell", "arguments": { "command": command } });
                            self.on_call(&synth, at);
                        }
                        "web_search_call" => {
                            let action = p.get("action").cloned().unwrap_or(Value::Null);
                            let mut q = s_at(&action, "query").to_string();
                            if q.is_empty() {
                                q = action.get("queries").and_then(|l| l.as_array()).and_then(|l| l.first()).map(s).unwrap_or("").to_string();
                            }
                            if q.is_empty() {
                                q = s_at(&action, "url").to_string();
                            }
                            let id = self.id();
                            self.items.push(ChatItem::Step(StepItem {
                                id,
                                tool: "web_search".to_string(),
                                verb: if s_at(&action, "type") == "open_page" { "Read".to_string() } else { "Searched the web for".to_string() },
                                target: q,
                                full: None,
                                code: false,
                                output: None,
                                error: false,
                                done: true,
                                diff: None,
                                added: None,
                                removed: None,
                                todos: None,
                                images: None,
                                questions: None,
                                at,
                            }));
                        }
                        "tool_search_call" => {
                            let args = p.get("arguments").cloned().unwrap_or(Value::Null);
                            let id = self.id();
                            self.items.push(ChatItem::Step(StepItem {
                                id,
                                tool: "tool_search".to_string(),
                                verb: "Loaded tools".to_string(),
                                target: first_line(s_at(&args, "query"), 60),
                                full: None,
                                code: false,
                                output: None,
                                error: false,
                                done: true,
                                diff: None,
                                added: None,
                                removed: None,
                                todos: None,
                                images: None,
                                questions: None,
                                at,
                            }));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    fn trim(&mut self, keep: usize) {
        if self.items.len() <= keep {
            return;
        }
        let drop = self.items.len() - keep;
        let gone: HashSet<String> = self.items[..drop]
            .iter()
            .filter_map(|it| match it {
                ChatItem::Step(s) => Some(s.id.clone()),
                _ => None,
            })
            .collect();
        self.calls.retain(|_, c| c.step_ids.iter().any(|id| !gone.contains(id)));
        self.items.drain(..drop);
    }
}
