//! Turns a Muse session log into what the chat view shows: your
//! messages, the agent's replies, and the steps it took in between, each with
//! its result. Pure: feed it lines, read `items` and `meta`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::codex::trim_output;
use crate::ledgers::{FileLedger, FileNote};
use crate::muse::{describe_muse_tool, muse_questions, question_ids};
use crate::text::s_at;
use crate::types::{Chat, ChatItem, ChatMeta, StepItem, TextItem, TodoState, UserItem};

pub struct MuseChat {
    items: Vec<ChatItem>,
    /// Every item by its id, so repeats update in place instead of doubling.
    by_id: HashMap<String, usize>,
    /// Steps by their call id, waiting for results and answers.
    steps: HashMap<String, usize>,
    meta: ChatMeta,
    files: FileLedger,
    /// Envelope ids whose usage is already summed.
    counted: HashSet<String>,
    /// Steps already filed under Files changed.
    filed: HashSet<String>,
    /// Tool call id to its question ids, so answers land on the right one.
    qids: HashMap<String, Vec<String>>,
    n: usize,
}

fn num_of(v: &Value) -> u64 {
    if let Some(n) = v.as_u64() {
        return n;
    }
    if let Some(n) = v.as_i64() {
        return n.max(0) as u64;
    }
    match v.as_f64() {
        Some(n) if n.is_finite() && n > 0.0 => n as u64,
        _ => 0,
    }
}

impl MuseChat {
    pub fn new() -> MuseChat {
        MuseChat {
            items: Vec::new(),
            by_id: HashMap::new(),
            steps: HashMap::new(),
            meta: ChatMeta::default(),
            files: FileLedger::new(),
            counted: HashSet::new(),
            filed: HashSet::new(),
            qids: HashMap::new(),
            n: 0,
        }
    }

    fn id(&mut self) -> String {
        let id = format!("m{}", self.n);
        self.n += 1;
        id
    }

    fn note_file(&mut self, c: FileNote) {
        let turn = self.meta.turn;
        self.files.note(&mut self.meta, turn, c);
    }

    fn on_user(&mut self, text: &str, at: u64, key: &str) {
        if text.trim().is_empty() || self.by_id.contains_key(key) {
            return;
        }
        let it = ChatItem::User(UserItem { id: key.to_string(), text: text.to_string(), images: 0, pics: None, at });
        self.by_id.insert(key.to_string(), self.items.len());
        self.items.push(it);
        self.meta.turn += 1;
        if self.meta.started.is_none() {
            self.meta.started = Some(at);
        }
    }

    fn on_text(&mut self, mid: &str, text: &str, at: u64) {
        if text.is_empty() {
            return;
        }
        if let Some(&idx) = self.by_id.get(mid) {
            if let Some(ChatItem::Text(old)) = self.items.get_mut(idx) {
                old.text = text.to_string();
                old.at = at;
                return;
            }
        }
        let it = ChatItem::Text(TextItem { id: mid.to_string(), text: text.to_string(), at });
        self.by_id.insert(mid.to_string(), self.items.len());
        self.items.push(it);
    }

    fn on_calls(&mut self, calls: &Value, at: u64) {
        let list = calls.as_array().cloned().unwrap_or_default();
        for c in &list {
            let cid = s_at(c, "call_id");
            let cid = if cid.is_empty() { s_at(c, "id") } else { cid };
            if cid.is_empty() || self.steps.contains_key(cid) {
                continue;
            }
            let name = s_at(c, "name").to_string();
            let mut input = Value::Null;
            if let Ok(v) = serde_json::from_str::<Value>(s_at(c, "args")) {
                if v.is_object() {
                    input = v;
                }
            }
            let d = describe_muse_tool(&name, &input);
            if let Some(questions) = &d.questions {
                if !questions.is_empty() {
                    self.qids.insert(cid.to_string(), question_ids(input.get("questions").unwrap_or(&Value::Null)));
                }
            }
            let step = ChatItem::Step(StepItem {
                id: cid.to_string(),
                tool: d.tool,
                verb: d.verb,
                target: d.target,
                full: d.full,
                code: d.code,
                output: None,
                error: false,
                done: false,
                diff: d.diff,
                added: d.added,
                removed: d.removed,
                todos: None,
                images: None,
                questions: d.questions,
                at,
            });
            self.steps.insert(cid.to_string(), self.items.len());
            self.by_id.insert(cid.to_string(), self.items.len());
            self.items.push(step);
        }
    }

    fn on_results(&mut self, results: &Value, at: u64) {
        let list = results.as_array().cloned().unwrap_or_default();
        for r in &list {
            let idx = match self.steps.get(s_at(r, "tool_call_id")) {
                Some(&idx) => idx,
                None => continue,
            };
            let done = matches!(self.items.get(idx), Some(ChatItem::Step(s)) if s.done);
            if done {
                continue;
            }
            let (id, verb, full, target, added, removed) = match self.items.get_mut(idx) {
                Some(ChatItem::Step(s)) => {
                    s.output = Some(trim_output(s_at(r, "text")));
                    s.done = true;
                    (s.id.clone(), s.verb.clone(), s.full.clone(), s.target.clone(), s.added, s.removed)
                }
                _ => continue,
            };
            if (verb == "Edited" || verb == "Wrote") && !self.filed.contains(&id) {
                self.filed.insert(id.clone());
                self.note_file(FileNote {
                    path: full.unwrap_or_default(),
                    name: Some(target),
                    added: added.unwrap_or(0),
                    removed: removed.unwrap_or(0),
                    is_new: false,
                    deleted: None,
                    step: Some(id),
                    at,
                });
            }
        }
    }

    fn on_requested(&mut self, ev: &Value) {
        // The tool call's own args already carry these questions; this is the
        // backstop for a call from before the tail (whose step is missing).
        let idx = match self.steps.get(s_at(ev, "tool_call_id")) {
            Some(&idx) => idx,
            None => return,
        };
        let questions = muse_questions(ev.get("questions").unwrap_or(&Value::Null));
        if questions.is_empty() {
            return;
        }
        let already = matches!(self.items.get(idx), Some(ChatItem::Step(s)) if s.questions.as_ref().is_some_and(|q| !q.is_empty()));
        if already {
            return;
        }
        if let Some(ChatItem::Step(s)) = self.items.get_mut(idx) {
            s.questions = Some(questions);
            self.qids.insert(s.id.clone(), question_ids(ev.get("questions").unwrap_or(&Value::Null)));
        }
    }

    fn on_settled(&mut self, ev: &Value) {
        let idx = match self.steps.get(s_at(ev, "tool_call_id")) {
            Some(&idx) => idx,
            None => return,
        };
        let answers = ev.get("answers").and_then(|a| a.as_array()).cloned().unwrap_or_default();
        if answers.is_empty() {
            return;
        }
        let has_questions = matches!(self.items.get(idx), Some(ChatItem::Step(s)) if s.questions.as_ref().is_some_and(|q| !q.is_empty()));
        if !has_questions {
            return;
        }
        let step_id = match self.items.get(idx) {
            Some(ChatItem::Step(s)) => s.id.clone(),
            _ => return,
        };
        let ids = self.qids.get(&step_id).cloned().unwrap_or_default();
        if let Some(ChatItem::Step(s)) = self.items.get_mut(idx) {
            if let Some(questions) = s.questions.as_mut() {
                for (k, q) in questions.iter_mut().enumerate() {
                    let want = ids.get(k).map(String::as_str).unwrap_or("");
                    let got = answers
                        .iter()
                        .find(|a| {
                            let id = s_at(a, "id");
                            !id.is_empty() && id == want
                        })
                        .or_else(|| answers.get(k));
                    if let Some(a) = got {
                        let label = s_at(a, "selected_label");
                        if !label.is_empty() {
                            q.answer = Some(label.to_string());
                        }
                    }
                }
            }
        }
    }

    fn on_model(&mut self, ev: &Value, env_id: &str) {
        if self.counted.contains(env_id) {
            return;
        }
        self.counted.insert(env_id.to_string());
        let empty = Value::Null;
        let usage = ev.get("usage").unwrap_or(&empty);
        let model = s_at(ev, "model");
        if !model.is_empty() {
            self.meta.model = Some(model.to_string());
        }
        let input = num_of(usage.get("input_tokens").unwrap_or(&empty));
        if input > 0 {
            self.meta.context = input;
        }
        self.meta.output += num_of(usage.get("output_tokens").unwrap_or(&empty));
    }

    fn on_todos(&mut self, ev: &Value) {
        let list = match ev.get("items").and_then(|i| i.as_array()) {
            Some(list) => list,
            None => return,
        };
        self.meta.todos = Some(list.iter().map(|t| crate::types::Todo { text: s_at(t, "text").to_string(), state: TodoState::parse(s_at(t, "status")) }).collect());
    }

    fn on_record(&mut self, v: &Value) {
        if !v.is_object() {
            return;
        }
        let env_id = {
            let id = s_at(v, "id");
            if id.is_empty() { self.id() } else { id.to_string() }
        };
        let at = num_of(v.get("recorded_at").unwrap_or(&Value::Null)) / 1000;
        let empty = Value::Null;
        let ev = v.get("payload").and_then(|p| p.get("event")).unwrap_or(&empty);
        if !ev.is_object() {
            return;
        }
        match s_at(ev, "kind") {
            "started" => {
                let prompt = s_at(ev, "prompt");
                if !prompt.is_empty() {
                    self.on_user(prompt, at, &env_id);
                }
            }
            "inbox_item_queued" => {
                let src = ev.get("source").cloned().unwrap_or(Value::Null);
                if s_at(&src, "source") != "user_steer" {
                    return;
                }
                let p = ev.get("payload").cloned().unwrap_or(Value::Null);
                let prompt = s_at(&p, "prompt");
                let prompt = if prompt.is_empty() { s_at(ev, "body") } else { prompt };
                self.on_user(prompt, at, &env_id);
                let effort = s_at(&p, "reasoning_effort");
                if !effort.is_empty() {
                    self.meta.effort = Some(effort.to_string());
                }
            }
            "assistant_message_committed" => {
                let text = s_at(ev, "text");
                if !text.is_empty() {
                    let mid = s_at(ev, "message_id");
                    let mid = if mid.is_empty() { env_id.clone() } else { mid.to_string() };
                    self.on_text(&mid, text, at);
                }
            }
            "assistant_tool_calls_committed" => self.on_calls(ev.get("tool_calls").unwrap_or(&empty), at),
            "tool_result_batch_committed" => self.on_results(ev.get("results").unwrap_or(&empty), at),
            "user_input_prompt_requested" => self.on_requested(ev),
            "user_input_prompt_settled" => self.on_settled(ev),
            "model_completed" => self.on_model(ev, &env_id),
            "todo_snapshot_updated" => self.on_todos(ev),
            _ => {}
        }
    }
}

impl Default for MuseChat {
    fn default() -> MuseChat {
        MuseChat::new()
    }
}

impl Chat for MuseChat {
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
            if !v.is_object() {
                continue;
            }
            self.on_record(&v);
            // A retained frame wraps records as escaped strings; read those too.
            // Envelope ids dedupe anything also present as its own line.
            if let Some(children) = v.get("children").and_then(|c| c.as_array()) {
                for c in children {
                    let raw = s_at(c, "record_json");
                    if raw.is_empty() {
                        continue;
                    }
                    if let Ok(inner) = serde_json::from_str::<Value>(raw) {
                        if inner.is_object() {
                            self.on_record(&inner);
                        }
                    }
                }
            }
        }
    }

    fn trim(&mut self, keep: usize) {
        if self.items.len() <= keep {
            return;
        }
        let drop = self.items.len() - keep;
        let gone: HashSet<String> = self.items[..drop].iter().map(|it| it.id().to_string()).collect();
        self.by_id.retain(|k, _| !gone.contains(k));
        self.steps.retain(|k, _| !gone.contains(k));
        self.qids.retain(|k, _| !gone.contains(k));
        self.items.drain(..drop);
        // Indexes shifted: rebuild them from what is left.
        self.by_id.clear();
        self.steps.clear();
        for (idx, it) in self.items.iter().enumerate() {
            self.by_id.insert(it.id().to_string(), idx);
            if matches!(it, ChatItem::Step(_)) {
                self.steps.insert(it.id().to_string(), idx);
            }
        }
    }
}
