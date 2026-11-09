//! Project memory (memoryview.ts): every fact a project's agents share, to
//! read, add, edit and delete by hand, and to send to a running agent of the
//! project to check against the code and fix. Agents change the same file
//! through the bytehive MCP tools; the panel follows them.

mod view;

use std::time::SystemTime;

use crate::memory::{self, Entry, Memory};

use super::*;

/// How often an open panel looks for an agent's change.
const FOLLOW: Duration = Duration::from_millis(1500);

pub(super) struct MemoryView {
    project: String,
    name: String,
    mem: Memory,
    mtime: Option<SystemTime>,
    pub(in crate::inbox) editing: Option<String>,
    /// The fact being sent to an agent, "*" for all of them.
    pub(in crate::inbox) asking: Option<String>,
    /// The agent picked to send to, by id.
    to: Option<String>,
    status: String,
    undo: Option<Entry>,
    error: String,
    add: Entity<TextareaState>,
    edit: Entity<TextareaState>,
    note: Entity<TextareaState>,
    _subs: Vec<Subscription>,
    _follow: Task<()>,
}

impl Inbox {
    pub(super) fn open_memory(&mut self, project: String, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.menu = None;
        crate::keys::set_active(false);
        let add = cx.new(|cx| TextareaState::new(window, cx).placeholder("Add a fact every agent here should know: a convention, a decision, where something lives…").auto_grow(2, 6));
        let edit = cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 8));
        let note = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 5));
        // Ctrl+Enter sends the form the box is in.
        let subs = vec![
            cx.subscribe_in(&add, window, |this, _, e: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary: true, .. } = e {
                    this.memory_add(window, cx);
                }
            }),
            cx.subscribe_in(&edit, window, |this, _, e: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { secondary: true, .. } = e {
                    this.memory_save_edit(cx);
                }
            }),
            cx.subscribe_in(&note, window, |this, _, e: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { secondary: true, .. } = e {
                    this.memory_send(window, cx);
                }
            }),
        ];
        let follow = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FOLLOW).await;
                if this.update(cx, |this, cx| this.memory_follow(cx)).is_err() {
                    break;
                }
            }
        });
        add.update(cx, |a, cx| a.focus(window, cx));
        let name = crate::fleet::project_of(&project);
        let (mem, mtime, error) = match memory::load(&project) {
            Ok((m, t)) => (m, t, String::new()),
            Err(e) => (Memory::default(), None, e),
        };
        self.memory = Some(MemoryView { project, name, mem, mtime, editing: None, asking: None, to: None, status: String::new(), undo: None, error, add, edit, note, _subs: subs, _follow: follow });
        cx.notify();
    }

    pub(in crate::inbox) fn close_memory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.memory = None;
        if !self.home {
            if let Some(index) = self.fleet.focused {
                self.fleet.focus(index, window, cx);
            }
        }
        cx.notify();
    }

    /// Esc puts an open form away first, then closes.
    pub(in crate::inbox) fn memory_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &mut self.memory {
            Some(m) if m.editing.is_some() || m.asking.is_some() => {
                m.editing = None;
                m.asking = None;
                cx.notify();
            }
            _ => self.close_memory(window, cx),
        }
    }

    /// An agent's change shows up while the panel is open, unless you are typing.
    fn memory_follow(&mut self, cx: &mut Context<Self>) {
        let Some(m) = &mut self.memory else { return };
        if m.editing.is_some() || m.asking.is_some() {
            return;
        }
        let now = std::fs::metadata(memory::file_of(&m.project)).and_then(|x| x.modified()).ok();
        if now == m.mtime {
            return;
        }
        match memory::load(&m.project) {
            Ok((mem, t)) => {
                m.mem = mem;
                m.mtime = t;
                m.error.clear();
            }
            Err(e) => m.error = e,
        }
        cx.notify();
    }

    /// Change the file and show what came of it.
    fn memory_run<T>(&mut self, f: impl Fn(&mut Memory) -> Result<T, String>, done: impl FnOnce(&mut MemoryView, T), cx: &mut Context<Self>) {
        let Some(m) = &mut self.memory else { return };
        m.error.clear();
        match memory::change(&m.project, f) {
            Ok((r, mem, t)) => {
                m.mem = mem;
                m.mtime = t;
                done(m, r);
            }
            Err(e) => m.error = e,
        }
        cx.notify();
    }

    fn memory_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = &self.memory else { return };
        let text = m.add.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        let add = m.add.clone();
        self.memory_run(
            move |mem| mem.add(&text, "you", now_ms()),
            |m, e| {
                m.status = format!("Added {}.", e.id);
                m.undo = None;
            },
            cx,
        );
        if self.memory.as_ref().is_some_and(|m| m.error.is_empty()) {
            add.update(cx, |a, cx| a.set_value("", window, cx));
        }
    }

    fn memory_save_edit(&mut self, cx: &mut Context<Self>) {
        let Some(m) = &self.memory else { return };
        let Some(id) = m.editing.clone() else { return };
        let text = m.edit.read(cx).value().to_string();
        let shown = id.clone();
        self.memory_run(
            move |mem| mem.update(&id, &text, "you", now_ms()),
            move |m, _| {
                m.editing = None;
                m.status = format!("Saved {shown}.");
                m.undo = None;
            },
            cx,
        );
    }

    fn memory_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.memory_run(
            move |mem| mem.delete(&id),
            |m, gone| {
                m.status = format!("Deleted {}.", gone.id);
                m.undo = Some(gone);
            },
            cx,
        );
    }

    fn memory_undo(&mut self, cx: &mut Context<Self>) {
        let Some(again) = self.memory.as_mut().and_then(|m| m.undo.take()) else { return };
        let by = if again.by.is_empty() { "you".to_string() } else { again.by.clone() };
        self.memory_run(move |mem| mem.add(&again.text, &by, now_ms()), |m, _| m.status = "Put back.".to_string(), cx);
    }

    /// The project's running agents, the one on screen first: who can be asked.
    fn memory_agents(&self, project: &str, cx: &App) -> Vec<(String, String)> {
        let mut list: Vec<(usize, String, String)> = self
            .fleet
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| presets::same_folder(&s.spec.cwd, project) && s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none()))
            .map(|(i, s)| (i, s.spec.id.clone(), s.spec.name.clone()))
            .collect();
        list.sort_by_key(|(i, ..)| Some(*i) != self.fleet.focused);
        list.into_iter().map(|(_, id, name)| (id, name)).collect()
    }

    fn memory_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = &self.memory else { return };
        let Some(asking) = m.asking.clone() else { return };
        let agents = self.memory_agents(&m.project, cx);
        let Some((id, name)) = m.to.as_ref().and_then(|to| agents.iter().find(|a| &a.0 == to)).or(agents.first()).cloned() else { return };
        let entry = if asking == "*" { None } else { m.mem.entries.iter().find(|e| e.id == asking).cloned() };
        let prompt = memory::fix_prompt(entry.as_ref(), &m.note.read(cx).value());
        let note = m.note.clone();
        self.queue_message(id, prompt, cx);
        if let Some(m) = &mut self.memory {
            m.asking = None;
            m.undo = None;
            m.status = format!("Sent to {name}. Its changes show up here as it makes them.");
        }
        note.update(cx, |n, cx| n.set_value("", window, cx));
        cx.notify();
    }
}
