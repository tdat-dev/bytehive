//! Turns an opencode session into what the chat view shows: the same items
//! and meta the other parsers build. Rows come from opencode's store: message
//! rows say who spoke, part rows hold the text and the tool calls. A part is
//! rewritten in place while its tool runs, so rows upsert by id, never append
//! twice. Pure: feed it lines, read `items` and `meta`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::codex::first_line;
use crate::ledgers::{FileLedger, FileNote};
use crate::opencode::{describe_opencode_tool, order, patch_files, picture_of, trim_output};
use crate::text::{s_at, MAX_IMAGES};
use crate::types::{Chat, ChatImage, ChatItem, ChatMeta, NoteItem, StepItem, TextItem, UserItem};

struct Msg {
    role: String,
    created: u64,
    parts: HashMap<String, Value>,
    part_at: HashMap<String, u64>,
}

impl Msg {
    fn fresh() -> Msg {
        Msg { role: String::new(), created: 0, parts: HashMap::new(), part_at: HashMap::new() }
    }
}

pub struct OpencodeChat {
    items: Vec<ChatItem>,
    meta: ChatMeta,
    files: FileLedger,
    msgs: HashMap<String, Msg>,
    /// What the chat shows, by the row it came from, kept in time order.
    shown: HashMap<String, ChatItem>,
    /// Shown id to its sort key.
    keys: HashMap<String, String>,
    /// Tool parts whose file changes are counted.
    filed: HashSet<String>,
    /// User messages already counted as a turn.
    users: HashSet<String>,
    /// Output tokens per reply, summed once.
    outputs: HashMap<String, u64>,
    latest_user: u64,
    latest_reply: u64,
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

impl OpencodeChat {
    pub fn new() -> OpencodeChat {
        OpencodeChat {
            items: Vec::new(),
            meta: ChatMeta::default(),
            files: FileLedger::new(),
            msgs: HashMap::new(),
            shown: HashMap::new(),
            keys: HashMap::new(),
            filed: HashSet::new(),
            users: HashSet::new(),
            outputs: HashMap::new(),
            latest_user: 0,
            latest_reply: 0,
        }
    }

    fn put(&mut self, key: &str, sort: &str, it: ChatItem) {
        self.shown.insert(key.to_string(), it);
        self.keys.insert(key.to_string(), sort.to_string());
    }

    /// Your message, from its text parts and pictures.
    fn draw_user(&mut self, id: &str) {
        let (created, mut parts) = match self.msgs.get(id) {
            Some(m) => (m.created, m.parts.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>()),
            None => return,
        };
        parts.sort_by(|a, b| order(&a.0).cmp(order(&b.0)));
        let mut text = String::new();
        let mut images = 0;
        let mut pics: Vec<ChatImage> = Vec::new();
        for (_, p) in &parts {
            if s_at(p, "type") == "text" && p.get("synthetic") != Some(&Value::Bool(true)) && p.get("ignored") != Some(&Value::Bool(true)) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(s_at(p, "text"));
            } else if s_at(p, "type") == "file" {
                if let Some(pic) = picture_of(s_at(p, "url"), s_at(p, "mime")) {
                    pics.push(pic);
                    images += 1;
                }
            }
        }
        let text = text.trim().to_string();
        if text.is_empty() && images == 0 {
            return;
        }
        if !self.users.contains(id) {
            self.users.insert(id.to_string());
            self.meta.turn += 1;
        }
        if pics.len() > MAX_IMAGES {
            pics = pics.split_off(pics.len() - MAX_IMAGES);
        }
        let sort = order(id).to_string();
        self.put(id, &sort, ChatItem::User(UserItem { id: id.to_string(), text, images, pics: if pics.is_empty() { None } else { Some(pics) }, at: created }));
    }

    fn on_message(&mut self, id: &str, created: u64, d: &Value) {
        self.msgs.entry(id.to_string()).or_insert_with(Msg::fresh);
        let knew = !self.msgs.get(id).map(|m| m.role.as_str()).unwrap_or("").is_empty();
        if let Some(m) = self.msgs.get_mut(id) {
            m.role = s_at(d, "role").to_string();
        }
        let role = self.msgs.get(id).map(|m| m.role.clone()).unwrap_or_default();
        // Parts that came in before their message waited for its role: draw them now.
        if !knew && role == "assistant" {
            let waiting: Vec<(String, Value, u64)> = self
                .msgs
                .get(id)
                .map(|m| m.parts.iter().map(|(pid, p)| (pid.clone(), p.clone(), m.part_at.get(pid).copied().unwrap_or(created))).collect())
                .unwrap_or_default();
            for (pid, p, at) in waiting {
                self.draw_part(&pid, &p, at);
            }
        }
        let empty = Value::Null;
        let time = d.get("time").unwrap_or(&empty);
        let msg_created = {
            let t = num_of(time.get("created").unwrap_or(&empty));
            if t > 0 { t } else { created }
        };
        if let Some(m) = self.msgs.get_mut(id) {
            m.created = msg_created;
        }
        if self.meta.started.is_none() || msg_created < self.meta.started.unwrap_or(u64::MAX) {
            self.meta.started = Some(msg_created);
        }
        if role == "user" {
            // The mode it runs in (build / plan) and the model's variant (its effort) are chosen per message.
            if msg_created >= self.latest_user {
                self.latest_user = msg_created;
                if !s_at(d, "agent").is_empty() {
                    self.meta.permission = Some(s_at(d, "agent").to_string());
                }
                let model = d.get("model").unwrap_or(&empty);
                if !s_at(model, "variant").is_empty() {
                    self.meta.effort = Some(s_at(model, "variant").to_string());
                }
            }
            self.draw_user(id);
            return;
        }
        if role != "assistant" {
            return;
        }
        if msg_created >= self.latest_reply {
            self.latest_reply = msg_created;
            if !s_at(d, "modelID").is_empty() {
                self.meta.model = Some(s_at(d, "modelID").to_string());
            }
            let tokens = d.get("tokens").unwrap_or(&empty);
            let cache = tokens.get("cache").unwrap_or(&empty);
            let context = num_of(tokens.get("input").unwrap_or(&empty)) + num_of(cache.get("read").unwrap_or(&empty)) + num_of(cache.get("write").unwrap_or(&empty));
            if context > 0 {
                self.meta.context = context;
            }
        }
        let tokens = d.get("tokens").unwrap_or(&empty);
        self.outputs.insert(id.to_string(), num_of(tokens.get("output").unwrap_or(&empty)) + num_of(tokens.get("reasoning").unwrap_or(&empty)));
        self.meta.output = self.outputs.values().sum();
        if let Some(err) = d.get("error") {
            if !err.is_null() {
                let data = err.get("data").unwrap_or(&empty);
                let mut said = s_at(data, "message");
                if said.is_empty() {
                    said = s_at(err, "name");
                }
                if said.is_empty() {
                    said = "Something went wrong";
                }
                let text = if s_at(err, "name") == "MessageAbortedError" {
                    "You stopped the agent".to_string()
                } else {
                    format!("Error: {}", first_line(said, 160))
                };
                let key = format!("{id}:err");
                let sort = format!("{}~", order(id));
                self.put(&key.clone(), &sort, ChatItem::Note(NoteItem { id: key, text, at: msg_created }));
            }
        }
    }

    fn on_part(&mut self, id: &str, message_id: &str, created: u64, data: &Value) {
        let m = self.msgs.entry(message_id.to_string()).or_insert_with(Msg::fresh);
        m.parts.insert(id.to_string(), data.clone());
        m.part_at.insert(id.to_string(), created);
        let role = m.role.clone();
        if role == "user" {
            self.draw_user(message_id);
        } else if role == "assistant" {
            self.draw_part(id, data, created);
        }
        // else: its message has not come yet; drawn when it does
    }

    /// One part of a reply. Step-start/finish, reasoning, snapshots and patches are not shown.
    fn draw_part(&mut self, id: &str, p: &Value, at: u64) {
        match s_at(p, "type") {
            "text" => {
                let text = s_at(p, "text").trim().to_string();
                if !text.is_empty() && p.get("synthetic") != Some(&Value::Bool(true)) {
                    let sort = order(id).to_string();
                    self.put(id, &sort, ChatItem::Text(TextItem { id: id.to_string(), text, at }));
                } else {
                    self.shown.remove(id);
                    self.keys.remove(id);
                }
            }
            "compaction" => {
                let sort = order(id).to_string();
                self.put(id, &sort, ChatItem::Note(NoteItem { id: id.to_string(), text: "Conversation compacted".to_string(), at }));
            }
            "subtask" => {
                let target = s_at(p, "description");
                let target = if target.is_empty() { s_at(p, "command") } else { target };
                let sort = order(id).to_string();
                self.put(id, &sort, ChatItem::Step(StepItem {
                    id: id.to_string(),
                    tool: "subtask".to_string(),
                    verb: "Asked a helper agent to".to_string(),
                    target: target.to_string(),
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
            "tool" => self.on_tool(id, p, at),
            _ => {}
        }
    }

    fn on_tool(&mut self, id: &str, p: &Value, at: u64) {
        let tool = s_at(p, "tool").to_string();
        let empty = Value::Null;
        let state = p.get("state").unwrap_or(&empty);
        let status = s_at(state, "status").to_string();
        let input = state.get("input").unwrap_or(&empty);
        let md = state.get("metadata").unwrap_or(&empty);
        let d = describe_opencode_tool(&tool, input, md);
        let mut step = StepItem {
            id: id.to_string(),
            tool: d.tool,
            verb: d.verb,
            target: d.target,
            full: d.full,
            code: d.code,
            output: None,
            error: false,
            done: status == "completed" || status == "error",
            diff: d.diff,
            added: d.added,
            removed: d.removed,
            todos: d.todos,
            images: None,
            questions: None,
            at,
        };
        if status == "error" {
            step.error = true;
            step.output = Some(trim_output(s_at(state, "error")));
        } else {
            let out = s_at(state, "output");
            let out = if out.is_empty() { s_at(md, "output") } else { out };
            if !out.is_empty() {
                step.output = Some(trim_output(out));
            }
        }
        // A picture the tool read or took comes back as an attachment.
        let pics: Vec<ChatImage> = state
            .get("attachments")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|a| picture_of(s_at(a, "url"), s_at(a, "mime")))
            .collect();
        if !pics.is_empty() {
            let keep = if pics.len() > MAX_IMAGES { pics[pics.len() - MAX_IMAGES..].to_vec() } else { pics };
            step.images = Some(keep);
        }
        if step.todos.as_ref().is_some_and(|t| !t.is_empty()) {
            self.meta.todos = step.todos.clone();
        }
        let sort = order(id).to_string();
        self.put(id, &sort, ChatItem::Step(step));
        if status != "completed" || self.filed.contains(id) {
            return;
        }
        self.filed.insert(id.to_string());
        let turn = self.meta.turn;
        if tool == "edit" || tool == "multiedit" {
            if let Some(ChatItem::Step(s)) = self.shown.get(id) {
                let (full, target, added, removed) = (s.full.clone().unwrap_or_default(), s.target.clone(), s.added.unwrap_or(0), s.removed.unwrap_or(0));
                self.files.note(&mut self.meta, turn, FileNote { path: full, name: Some(target), added, removed, is_new: false, deleted: None, step: Some(id.to_string()), at });
            }
        } else if tool == "write" {
            if let Some(ChatItem::Step(s)) = self.shown.get(id) {
                let (full, target, added) = (s.full.clone().unwrap_or_default(), s.target.clone(), s.added.unwrap_or(0));
                let is_new = md.get("exists") == Some(&Value::Bool(false));
                self.files.note(&mut self.meta, turn, FileNote { path: full, name: Some(target), added, removed: 0, is_new, deleted: None, step: Some(id.to_string()), at });
            }
        } else if tool == "patch" || tool == "apply_patch" {
            let mut body = s_at(input, "patchText");
            if body.is_empty() {
                body = s_at(input, "patch");
            }
            if body.is_empty() {
                body = s_at(input, "input");
            }
            for f in patch_files(body) {
                self.files.note(&mut self.meta, turn, FileNote {
                    path: f.path,
                    name: None,
                    added: 0,
                    removed: 0,
                    is_new: f.kind == crate::opencode::PatchRefKind::Add,
                    deleted: if f.kind == crate::opencode::PatchRefKind::Delete { Some(true) } else { None },
                    step: Some(id.to_string()),
                    at,
                });
            }
        }
    }

    fn redraw(&mut self) {
        // A reply's error goes after the last of its parts.
        let err_keys: Vec<String> = self.shown.keys().filter(|k| k.ends_with(":err")).cloned().collect();
        for key in err_keys {
            let msg_id = key.trim_end_matches(":err").to_string();
            let last = self
                .msgs
                .get(&msg_id)
                .map(|m| m.parts.keys().map(|k| order(k)).max().unwrap_or_else(|| order(&msg_id)).to_string())
                .unwrap_or_else(|| order(&msg_id).to_string());
            self.keys.insert(key, format!("{last}~"));
        }
        let mut sorted: Vec<(String, String)> = self.shown.keys().map(|k| (k.clone(), self.keys.get(k).cloned().unwrap_or_default())).collect();
        sorted.sort_by(|a, b| a.1.cmp(&b.1));
        self.items.clear();
        for (key, _) in sorted {
            if let Some(it) = self.shown.get(&key) {
                self.items.push(it.clone());
            }
        }
    }
}

impl Default for OpencodeChat {
    fn default() -> OpencodeChat {
        OpencodeChat::new()
    }
}

impl Chat for OpencodeChat {
    fn items(&self) -> &[ChatItem] {
        &self.items
    }

    fn meta(&self) -> &ChatMeta {
        &self.meta
    }

    fn feed(&mut self, text: &str) {
        let mut changed = false;
        for line in text.split('\n') {
            if line.trim().is_empty() {
                continue;
            }
            let r: Value = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let data = r.get("data").cloned().unwrap_or(Value::Null);
            if !data.is_object() && !data.is_array() {
                continue;
            }
            let id = s_at(&r, "id").to_string();
            let message_id = s_at(&r, "messageId").to_string();
            let created = num_of(r.get("created").unwrap_or(&Value::Null));
            match s_at(&r, "kind") {
                "message" => self.on_message(&id, created, &data),
                "part" => self.on_part(&id, &message_id, created, &data),
                _ => continue,
            }
            changed = true;
        }
        if changed {
            self.redraw();
        }
    }

    fn trim(&mut self, keep: usize) {
        if self.shown.len() <= keep {
            return;
        }
        let mut by_time: Vec<String> = self.shown.keys().cloned().collect();
        by_time.sort_by(|a, b| self.keys.get(a).cloned().unwrap_or_default().cmp(&self.keys.get(b).cloned().unwrap_or_default()));
        let drop = by_time[..by_time.len() - keep].to_vec();
        let kept: HashSet<String> = by_time[by_time.len() - keep..].iter().map(|k| k.split(':').next().unwrap_or("").to_string()).collect();
        for k in drop {
            self.shown.remove(&k);
            self.keys.remove(&k);
        }
        self.msgs.retain(|id, _| kept.contains(id));
        self.redraw();
    }
}
