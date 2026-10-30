//! Ledgers that keep ChatMeta.files and ChatMeta.tasks summed and ordered.

use std::collections::HashMap;

use crate::types::{BgState, BgTask, ChatMeta, FileChange};

/// One file change to note.
#[derive(Debug, Clone, Default)]
pub struct FileNote {
    pub path: String,
    pub name: Option<String>,
    pub added: u32,
    pub removed: u32,
    pub is_new: bool,
    pub deleted: Option<bool>,
    pub step: Option<String>,
    pub at: u64,
}

/// Keeps ChatMeta.files: one entry per file, summed, newest change first.
#[derive(Debug, Default)]
pub struct FileLedger {
    files: HashMap<String, FileChange>,
    /// Newest first by the order changes happened in, not their clock (lines can share a timestamp).
    last: HashMap<String, u64>,
    n: u64,
}

impl FileLedger {
    pub fn new() -> FileLedger {
        FileLedger::default()
    }

    fn key(path: &str) -> String {
        path.replace('\\', "/").to_lowercase()
    }

    pub fn note(&mut self, meta: &mut ChatMeta, turn: u32, c: FileNote) {
        let key = Self::key(&c.path);
        let fc = self.files.entry(key.clone()).or_insert_with(|| FileChange {
            path: c.path.clone(),
            name: c.name.clone().unwrap_or_else(|| crate::text::base_name(&c.path).to_string()),
            added: 0,
            removed: 0,
            is_new: false,
            deleted: None,
            turn,
            at: c.at,
            steps: Vec::new(),
        });
        fc.added += c.added;
        fc.removed += c.removed;
        if c.is_new {
            fc.is_new = true;
        }
        if let Some(d) = c.deleted {
            fc.deleted = Some(d);
        }
        if let Some(step) = c.step.as_deref() {
            if !fc.steps.iter().any(|x| x == step) {
                fc.steps.push(step.to_string());
                fc.turn = turn;
                fc.at = fc.at.max(c.at);
                self.n += 1;
                self.last.insert(key.clone(), self.n);
            }
        }
        if !self.last.contains_key(&key) {
            self.n += 1;
            self.last.insert(key, self.n);
        }
        let mut files: Vec<FileChange> = self.files.values().cloned().collect();
        files.sort_by(|a, b| {
            let order = |f: &FileChange| self.last.get(&Self::key(&f.path)).copied().unwrap_or(0);
            order(b).cmp(&order(a))
        });
        meta.files = files;
    }
}

/// A background-task update to note.
#[derive(Debug, Clone, Default)]
pub struct TaskNote {
    pub id: String,
    pub label: Option<String>,
    pub command: Option<String>,
    pub state: Option<BgState>,
    pub output: Option<String>,
    pub at: Option<u64>,
}

/// Keeps ChatMeta.tasks: what each background command is doing, running ones first.
#[derive(Debug, Default)]
pub struct TaskLedger {
    tasks: HashMap<String, BgTask>,
}

impl TaskLedger {
    pub fn new() -> TaskLedger {
        TaskLedger::default()
    }

    pub fn note(&mut self, meta: &mut ChatMeta, t: TaskNote) {
        let cur = self.tasks.entry(t.id.clone()).or_insert_with(|| BgTask {
            id: t.id.clone(),
            label: t.label.clone().or_else(|| t.command.clone()).unwrap_or_else(|| t.id.clone()),
            command: None,
            state: BgState::Running,
            output: None,
            at: t.at.unwrap_or(0),
        });
        // Only set, non-empty fields merge over the current entry.
        if let Some(label) = t.label.as_deref() {
            if !label.is_empty() {
                cur.label = label.to_string();
            }
        }
        if let Some(command) = t.command.as_deref() {
            if !command.is_empty() {
                cur.command = Some(command.to_string());
            }
        }
        if let Some(state) = t.state {
            cur.state = state;
        }
        if let Some(output) = t.output.as_deref() {
            if !output.is_empty() {
                cur.output = Some(output.to_string());
            }
        }
        if let Some(at) = t.at {
            cur.at = at;
        }
        if cur.label.is_empty() {
            cur.label = cur.command.clone().unwrap_or_else(|| cur.id.clone());
        }
        let mut tasks: Vec<BgTask> = self.tasks.values().cloned().collect();
        tasks.sort_by(|a, b| {
            let rank = |s: BgState| if s == BgState::Running { 0 } else { 1 };
            rank(a.state).cmp(&rank(b.state)).then(b.at.cmp(&a.at))
        });
        meta.tasks = tasks;
    }
}
