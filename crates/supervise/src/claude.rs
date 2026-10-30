//! Turns a Claude Code transcript into chat items: your messages, the agent's
//! replies, and the steps between. Pure: feed it lines, read `items`.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::describe::{describe_tool, is_task_stop, parse_task_notice, task_state};
use crate::ledgers::{FileLedger, FileNote, TaskLedger, TaskNote};
use crate::text::{b_at, num_at, parse_ms, s_at};
use crate::types::{BgState, Chat, ChatItem, ChatMeta, NoteItem, StepItem, TextItem};

fn model_1m_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\[1m\]").unwrap())
}

pub struct ClaudeChat {
    pub(crate) items: Vec<ChatItem>,
    pub(crate) meta: ChatMeta,
    /// Tool-use id to the index of its step in `items`.
    pub(crate) steps: HashMap<String, usize>,
    pub(crate) files: FileLedger,
    pub(crate) tasks: TaskLedger,
    /// Message ids whose usage is already summed.
    pub(crate) counted: HashSet<String>,
    /// Messages already shown from the queue.
    pub(crate) queued_ids: HashSet<String>,
    /// The last ! command you ran, for the output that follows it.
    pub(crate) last_bang: String,
    /// Which run of the CLI wrote the last line.
    pub(crate) last_run: String,
    pub(crate) n: usize,
}

impl ClaudeChat {
    pub fn new() -> ClaudeChat {
        ClaudeChat {
            items: Vec::new(),
            meta: ChatMeta::default(),
            steps: HashMap::new(),
            files: FileLedger::new(),
            tasks: TaskLedger::new(),
            counted: HashSet::new(),
            queued_ids: HashSet::new(),
            last_bang: String::new(),
            last_run: String::new(),
            n: 0,
        }
    }

    pub(crate) fn id(&mut self) -> String {
        let id = format!("c{}", self.n);
        self.n += 1;
        id
    }

    pub(crate) fn note_file(&mut self, c: FileNote) {
        let turn = self.meta.turn;
        self.files.note(&mut self.meta, turn, c);
    }

    pub(crate) fn note_task(&mut self, t: TaskNote) {
        self.tasks.note(&mut self.meta, t);
    }

    /// Background commands finishing, as the CLI tells the agent.
    pub(crate) fn task_notices(&mut self, raw: &str) {
        for t in parse_task_notice(raw) {
            self.note_task(TaskNote { id: t.id, state: Some(t.state), output: t.output, ..Default::default() });
        }
    }

    fn on_assistant(&mut self, v: &Value, at: u64) {
        let msg = v.get("message").cloned().unwrap_or(Value::Null);
        if let Some(model) = msg.get("model").and_then(|m| m.as_str()) {
            if model != "<synthetic>" {
                self.meta.model = Some(model.to_string());
                if model_1m_re().is_match(model) {
                    self.meta.window = Some(1_000_000);
                }
            }
        }
        // One reply is written as several lines that share its id and its usage.
        let mid = s_at(&msg, "id");
        if let Some(usage) = msg.get("usage") {
            if !mid.is_empty() && !self.counted.contains(mid) {
                self.counted.insert(mid.to_string());
                self.meta.output += num_at(usage, "output_tokens");
                self.meta.context = num_at(usage, "input_tokens") + num_at(usage, "cache_read_input_tokens") + num_at(usage, "cache_creation_input_tokens");
            }
        }
        let Some(parts) = msg.get("content").and_then(|c| c.as_array()) else { return };
        for part in parts {
            match part.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    let text = s_at(part, "text").trim().to_string();
                    if !text.is_empty() && text != "(no content)" {
                        let id = self.id();
                        self.items.push(ChatItem::Text(TextItem { id, text, at }));
                    }
                }
                Some("tool_use") => {
                    let tid = s_at(part, "id").to_string();
                    if self.steps.contains_key(&tid) {
                        continue;
                    }
                    let name = s_at(part, "name");
                    let input = part.get("input").cloned().unwrap_or(Value::Null);
                    let desc = describe_tool(name, &input);
                    let id = self.id();
                    let step = StepItem {
                        id: id.clone(),
                        tool: desc.tool,
                        verb: desc.verb,
                        target: desc.target,
                        full: desc.full,
                        code: desc.code,
                        output: None,
                        error: false,
                        done: false,
                        diff: desc.diff,
                        added: desc.added,
                        removed: desc.removed,
                        todos: desc.todos.clone(),
                        images: desc.images,
                        questions: desc.questions,
                        at,
                    };
                    if let Some(todos) = step.todos.clone() {
                        self.meta.todos = Some(todos);
                    }
                    if let Some(full) = step.full.clone() {
                        if step.verb == "Edited" || step.verb == "Wrote" {
                            let turn = self.meta.turn;
                            self.files.note(
                                &mut self.meta,
                                turn,
                                FileNote {
                                    path: full,
                                    name: Some(step.target.clone()),
                                    added: step.added.unwrap_or(0),
                                    removed: step.removed.unwrap_or(0),
                                    step: Some(id.clone()),
                                    at,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                    // Stopping a background command by hand.
                    if is_task_stop(name) {
                        let bg = {
                            let t = s_at(&input, "task_id");
                            if !t.is_empty() {
                                t.to_string()
                            } else {
                                let t = s_at(&input, "shell_id");
                                if !t.is_empty() {
                                    t.to_string()
                                } else {
                                    s_at(&input, "id").to_string()
                                }
                            }
                        };
                        if !bg.is_empty() {
                            self.note_task(TaskNote { id: bg, state: Some(BgState::Stopped), ..Default::default() });
                        }
                    }
                    self.steps.insert(tid, self.items.len());
                    self.items.push(ChatItem::Step(step));
                }
                _ => {}
            }
        }
    }
}

impl Chat for ClaudeChat {
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
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            if b_at(&v, "isSidechain") || b_at(&v, "isMeta") {
                continue; // a helper agent's inner steps, injected context
            }
            let at = parse_ms(s_at(&v, "timestamp"));
            if at > 0 && self.meta.started.is_none() && (s_at(&v, "type") == "user" || s_at(&v, "type") == "assistant") {
                self.meta.started = Some(at);
            }
            // A new run of the CLI (resumed later): what the last run left in the background died with it.
            let run = s_at(&v, "session_id");
            if !run.is_empty() && run != self.last_run {
                if !self.last_run.is_empty() {
                    let running: Vec<String> = self.meta.tasks.iter().filter(|t| t.state == BgState::Running).map(|t| t.id.clone()).collect();
                    for id in running {
                        self.note_task(TaskNote { id, state: Some(BgState::Stopped), ..Default::default() });
                    }
                }
                self.last_run = run.to_string();
            }
            // Each reply says the effort it ran at; each of your messages, the permission mode.
            if let Some(effort) = v.get("effort").and_then(|e| e.as_str()) {
                if !effort.is_empty() {
                    self.meta.effort = Some(effort.to_string());
                }
            }
            if let Some(pm) = v.get("permissionMode").and_then(|e| e.as_str()) {
                if !pm.is_empty() {
                    self.meta.permission = Some(pm.to_string());
                }
            }
            match s_at(&v, "type") {
                "user" => self.on_user(&v, at),
                "assistant" => self.on_assistant(&v, at),
                "attachment" => {
                    let a = v.get("attachment").cloned().unwrap_or(Value::Null);
                    if s_at(&a, "type") == "task_status" && !s_at(&a, "taskId").is_empty() {
                        let shell = a.get("shell").cloned().unwrap_or(Value::Null);
                        let description = s_at(&a, "description");
                        let command = s_at(&shell, "command");
                        let output = s_at(&a, "outputFilePath");
                        self.note_task(TaskNote {
                            id: s_at(&a, "taskId").to_string(),
                            label: if description.is_empty() { None } else { Some(description.to_string()) },
                            command: if command.is_empty() { None } else { Some(command.to_string()) },
                            state: Some(task_state(s_at(&a, "status"))),
                            output: if output.is_empty() { None } else { Some(output.to_string()) },
                            at: None,
                        });
                    } else {
                        self.on_queued(&v, at);
                    }
                }
                "system" if s_at(&v, "subtype") == "compact_boundary" => {
                    let id = self.id();
                    self.items.push(ChatItem::Note(NoteItem { id, text: "Conversation compacted".to_string(), at }));
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
        self.steps.retain(|_, idx| *idx >= drop);
        for idx in self.steps.values_mut() {
            *idx -= drop;
        }
        self.items.drain(..drop);
    }
}

impl Default for ClaudeChat {
    fn default() -> ClaudeChat {
        ClaudeChat::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_and_tool_result_spans() {
        let mut chat = ClaudeChat::new();
        chat.feed(r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"run it"}]}}"#);
        chat.feed(r#"{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"make","description":"build"}}]}}"#);
        chat.feed(r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]},"toolUseResult":{"stdout":"ok","exitCode":0}}"#);
        assert_eq!(chat.items.len(), 2);
        assert_eq!(chat.meta.turn, 1);
        assert!(matches!(chat.items[0], ChatItem::User(_)));
        let step = match &chat.items[1] {
            ChatItem::Step(s) => s,
            _ => panic!("expected step"),
        };
        assert_eq!(step.tool, "Bash");
        assert_eq!(step.output.as_deref(), Some("ok"));
        assert_eq!(step.error, false);
        assert_eq!(step.done, true);
    }

    #[test]
    fn thinking_is_hidden_by_default() {
        let mut chat = ClaudeChat::new();
        chat.feed(r#"{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"done"}]}}"#);
        assert_eq!(chat.items.len(), 1);
        assert!(matches!(chat.items[0], ChatItem::Text(_)));
    }

    #[test]
    fn queued_message_shows_once() {
        let mut chat = ClaudeChat::new();
        chat.feed(r#"{"type":"attachment","attachment":{"type":"queued_command","prompt":"later","source_uuid":"q1","origin":{"kind":"human"}}}"#);
        chat.feed(r#"{"type":"attachment","attachment":{"type":"queued_command","prompt":"later","source_uuid":"q1","origin":{"kind":"human"}}}"#);
        assert_eq!(chat.items.len(), 1);
        assert!(matches!(chat.items[0], ChatItem::User(_)));
    }
}