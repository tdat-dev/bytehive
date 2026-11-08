//! The user side of the Claude parser: your messages, queued messages, tool results.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::claude::ClaudeChat;
use crate::ledgers::{FileNote, TaskNote};
use crate::text::{ask_answers, b_at, clean_user_text, images_of, parse_ms, result_text, s_at, trim_output};
use crate::types::{BgState, ChatItem, NoteItem, StepItem, UserItem};

fn switched_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<local-command-stdout>Set model to `([^`]+)`").unwrap())
}

fn default_suffix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\s*\(default\)\s*$").unwrap())
}

fn context_1m_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b1M context\b").unwrap())
}

fn bash_in_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)^\s*<bash-input>(.*?)</bash-input>\s*$").unwrap())
}

fn bash_out_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<bash-stdout>(.*?)</bash-stdout>").unwrap())
}

fn bash_err_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<bash-stderr>(.*?)</bash-stderr>").unwrap())
}

fn cmd_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<command-name>([^<]*)</command-name>(?:.*?<command-args>([^<]*)</command-args>)?").unwrap())
}

fn local_cmd_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<local-command-(stdout|stderr)>|<local-command-caveat>").unwrap())
}


impl ClaudeChat {
    pub(crate) fn on_user(&mut self, v: &Value, at: u64) {
        if self.queued_ids.contains(s_at(v, "uuid")) {
            return; // already shown when it was queued
        }
        // Note: an empty uuid is looked up too, matching the TS (a store that
        // never holds "" makes the lookup miss, as there).
        let msg = v.get("message").cloned().unwrap_or(Value::Null);
        let content = msg.get("content").cloned().unwrap_or(Value::Null);
        if let Some(text) = content.as_str() {
            self.task_notices(text);
            self.user_text(text, 0, v, at, Vec::new());
            return;
        }
        let Some(parts) = content.as_array() else { return };
        let mut text = String::new();
        let mut images = 0usize;
        for part in parts {
            match part.get("type").and_then(|t| t.as_str()) {
                Some("tool_result") => {
                    let tid = s_at(part, "tool_use_id").to_string();
                    let Some(&idx) = self.steps.get(&tid) else { continue };
                    let extra = v.get("toolUseResult").cloned().unwrap_or(Value::Null);
                    let mut out = result_text(part.get("content").unwrap_or(&Value::Null));
                    if extra.get("stdout").and_then(|x| x.as_str()).is_some() || extra.get("stderr").and_then(|x| x.as_str()).is_some() {
                        out = [s_at(&extra, "stdout"), s_at(&extra, "stderr")]
                            .into_iter()
                            .filter(|x| !x.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n");
                    }
                    let pics = images_of(part.get("content").unwrap_or(&Value::Null));
                    let is_error = part.get("is_error").and_then(|x| x.as_bool()) == Some(true);
                    let full;
                    let verb;
                    {
                        let ChatItem::Step(step) = &mut self.items[idx] else { continue };
                        step.output = Some(trim_output(&out));
                        if !pics.is_empty() {
                            step.images = Some(pics);
                        }
                        step.error = is_error;
                        step.done = true;
                        if let Some(questions) = step.questions.as_mut() {
                            let got = ask_answers(&out, &extra);
                            for q in questions.iter_mut() {
                                q.answer = got.iter().find(|(k, _)| *k == q.question).map(|(_, a)| a.clone());
                            }
                        }
                        full = step.full.clone();
                        verb = step.verb.clone();
                    }
                    if extra.get("type").and_then(|t| t.as_str()) == Some("create") {
                        if let Some(path) = full.as_deref() {
                            if verb == "Edited" || verb == "Wrote" {
                                self.note_file(FileNote { path: path.to_string(), is_new: true, at, ..Default::default() });
                            }
                        }
                    }
                    // A command left running on its own: Bash/Monitor with run_in_background.
                    let bg = {
                        let b = s_at(&extra, "backgroundTaskId");
                        if !b.is_empty() {
                            b.to_string()
                        } else {
                            s_at(&extra, "taskId").to_string()
                        }
                    };
                    if !bg.is_empty() {
                        let (target, command) = match &self.items[idx] {
                            ChatItem::Step(step) => (step.target.clone(), step.full.clone()),
                            _ => (String::new(), None),
                        };
                        self.note_task(TaskNote {
                            id: bg,
                            label: Some(target),
                            command,
                            state: Some(BgState::Running),
                            at: Some(at),
                            ..Default::default()
                        });
                    }
                }
                Some("text") => {
                    let t = s_at(part, "text");
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                }
                Some("image") => images += 1,
                _ => {}
            }
        }
        if !text.is_empty() || images > 0 {
            let pics = images_of(&content);
            self.user_text(&text, images, v, at, pics);
        }
    }

    pub(crate) fn user_text(&mut self, raw: &str, images: usize, v: &Value, at: u64, pics: Vec<crate::types::ChatImage>) {
        self.task_notices(raw);
        if let Some(origin) = v.get("origin") {
            if s_at(origin, "kind") != "human" {
                return; // hook output, task notices…
            }
        }
        // /model answers with the model it switched to; that is the model from
        // now on, before any reply comes back with its id.
        if let Some(m) = switched_re().captures(raw) {
            let model = default_suffix_re().replace_all(&m[1], "").trim().to_string();
            self.meta.window = Some(if context_1m_re().is_match(&model) { 1_000_000 } else { 200_000 });
            self.meta.model = Some(model.clone());
            let id = self.id();
            self.items.push(ChatItem::Note(NoteItem { id, text: format!("Switched to {model}"), at }));
            return;
        }
        // A ! command you ran yourself (bash mode): the CLI records what you
        // typed and what it printed as tagged text, which read as raw tags in
        // a bubble. Show it as the terminal does: your "!command", then a
        // step with its output.
        if let Some(m) = bash_in_re().captures(raw) {
            self.last_bang = m[1].trim().to_string();
            self.meta.turn += 1;
            let bang = format!("!{}", self.last_bang);
            let id = self.id();
            self.items.push(ChatItem::User(UserItem { id, text: bang, images: 0, pics: None, at }));
            return;
        }
        let bash_out = bash_out_re().captures(raw).map(|m| m[1].trim().to_string()).unwrap_or_default();
        let bash_err = bash_err_re().captures(raw).map(|m| m[1].trim().to_string()).unwrap_or_default();
        if bash_out_re().is_match(raw) || bash_err_re().is_match(raw) {
            let out = [bash_out.as_str(), bash_err.as_str()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join("\n");
            let target = self.last_bang.clone();
            let id = self.id();
            self.items.push(ChatItem::Step(StepItem {
                id,
                tool: "Bash".to_string(),
                verb: "Ran".to_string(),
                target: target.clone(),
                full: Some(target),
                code: true,
                output: Some(trim_output(&out)),
                error: !bash_err.is_empty(),
                done: true,
                diff: None,
                added: None,
                removed: None,
                todos: None,
                images: None,
                questions: None,
                at,
            }));
            return;
        }
        if let Some(m) = cmd_re().captures(raw) {
            let name = m.get(1).map(|x| x.as_str()).unwrap_or("");
            // /model is said by its answer above
            if name == "/model" {
                return;
            }
            let args = m.get(2).map(|x| x.as_str()).unwrap_or("");
            let text = format!("{name}{}", if args.is_empty() { String::new() } else { format!(" {args}") }).trim().to_string();
            let id = self.id();
            self.items.push(ChatItem::Note(NoteItem { id, text, at }));
            return;
        }
        if local_cmd_re().is_match(raw) {
            return;
        }
        if raw.trim_start().starts_with("[Request interrupted by user") {
            let id = self.id();
            self.items.push(ChatItem::Note(NoteItem { id, text: "You stopped the agent".to_string(), at }));
            return;
        }
        let text = clean_user_text(raw);
        if text.is_empty() && images == 0 {
            return;
        }
        self.meta.turn += 1;
        let id = self.id();
        self.items.push(ChatItem::User(UserItem {
            id,
            text,
            images,
            pics: if pics.is_empty() { None } else { Some(pics) },
            at,
        }));
    }

    /// A message you typed while the agent was working: the CLI queues it and
    /// hands it over mid-turn as an attachment, not as a user line.
    pub(crate) fn on_queued(&mut self, v: &Value, at: u64) {
        let a = v.get("attachment").cloned().unwrap_or(Value::Null);
        if s_at(&a, "type") != "queued_command" {
            return;
        }
        if let Some(prompt) = a.get("prompt").and_then(|p| p.as_str()) {
            self.task_notices(prompt);
        }
        if a.get("commandMode").is_some() && s_at(&a, "commandMode") != "prompt" {
            return;
        }
        let human = match a.get("origin") {
            Some(origin) => s_at(origin, "kind") == "human",
            None => b_at(&a, "humanTurn"),
        };
        if !human {
            return;
        }
        let source = s_at(&a, "source_uuid");
        if !source.is_empty() {
            if self.queued_ids.contains(source) {
                return;
            }
            self.queued_ids.insert(source.to_string());
        }
        let when = {
            let t = parse_ms(s_at(&a, "timestamp"));
            if t > 0 {
                t
            } else {
                at
            }
        };
        let human_v = serde_json::json!({ "origin": { "kind": "human" } });
        if let Some(prompt) = a.get("prompt").and_then(|p| p.as_str()) {
            self.user_text(prompt, 0, &human_v, when, Vec::new());
            return;
        }
        let Some(parts) = a.get("prompt").and_then(|p| p.as_array()) else { return };
        let mut text = String::new();
        let mut images = 0usize;
        for part in parts {
            match part.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(s_at(part, "text"));
                }
                Some("image") => images += 1,
                _ => {}
            }
        }
        if !text.is_empty() || images > 0 {
            let pics = images_of(a.get("prompt").unwrap_or(&Value::Null));
            self.user_text(&text, images, &human_v, when, pics);
        }
    }

}