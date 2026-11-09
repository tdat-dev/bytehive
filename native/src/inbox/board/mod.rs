//! The Board (kanban.ts, its dock panel in dock.ts, the kb- rules in
//! dock.css): the current project's lists and cards, in the drawer today's
//! app opens with Ctrl+Shift+K. Agents move the same cards through the
//! bytehive MCP tools, so the file is followed every 1.5 s and every change
//! is applied to a fresh read of it, off the UI thread. The conductor and
//! plan.json's import run on their own timer, open or not (conduct.rs).

mod act;
mod card;
mod conduct;
mod detail;
mod events;
mod keep;
mod lists;
mod look;
mod panel;
mod pick;
mod toggle;

use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::board::{self as model, Board};

use super::dropdown::Spot;
use super::*;

pub(in crate::inbox) use conduct::Conductor;
pub(in crate::inbox) use events::Field;

/// How often an open board looks for an agent's change.
const FOLLOW: Duration = Duration::from_millis(1500);

/// A delete waiting on its "OK".
#[derive(Clone, PartialEq)]
enum Ask {
    List(String),
    Card(String),
}

/// The fields the board types into, made once per open.
struct Inputs {
    rename: Entity<InputState>,
    new_list: Entity<InputState>,
    composer: Entity<TextareaState>,
    title: Entity<TextareaState>,
    desc: Entity<TextareaState>,
    item: Entity<InputState>,
    due: Entity<InputState>,
}

pub(in crate::inbox) struct BoardView {
    /// The project whose board this is; none shows "Open a project…".
    dir: Option<String>,
    board: Board,
    /// The file's time as last read or written.
    mtime: Option<SystemTime>,
    /// The card shown in detail, by id.
    open: Option<String>,
    /// The list whose title is being renamed.
    renaming: Option<String>,
    adding_list: bool,
    /// The list whose "+ Add a card" box is open.
    composing: Option<String>,
    asking: Option<Ask>,
    /// "Send to agent…" open, under its button.
    picking: bool,
    pick_spot: Spot,
    pub(in crate::inbox) focused: Option<Field>,
    /// Writes sent, so only the last one's answer replaces the board.
    writes: u64,
    /// One write at a time, in order.
    lock: Arc<Mutex<()>>,
    inputs: Inputs,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
    _follow: Task<()>,
}

impl Inbox {
    pub(in crate::inbox) fn open_board(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.menu = None;
        self.popup = None;
        crate::keys::set_active(false);
        let (inputs, subs) = Self::board_inputs(window, cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let follow = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(FOLLOW).await;
                let Ok(found) = this.update_in(cx, |this, window, cx| this.board_follow_project(window, cx)) else { break };
                let Some((dir, seen)) = found else { continue };
                let got = cx
                    .background_spawn(async move {
                        let now = model::mtime(&dir);
                        if now.is_none() || now == seen {
                            return None;
                        }
                        model::read(&dir).ok().flatten()
                    })
                    .await;
                if let Some((board, at)) = got {
                    if this.update_in(cx, |this, window, cx| this.board_reload(board, at, window, cx)).is_err() {
                        break;
                    }
                }
            }
        });
        let dir = self.project_dir();
        self.board = Some(BoardView {
            dir: dir.clone(),
            board: model::default_board(),
            mtime: None,
            open: None,
            renaming: None,
            adding_list: false,
            composing: None,
            asking: None,
            picking: false,
            pick_spot: Default::default(),
            focused: None,
            writes: 0,
            lock: Arc::default(),
            inputs,
            focus,
            _subs: subs,
            _follow: follow,
        });
        if let Some(dir) = dir {
            self.board_load(dir, window, cx);
        }
        cx.notify();
    }

    pub(in crate::inbox) fn close_board(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.board_commit_detail(window, cx);
        self.board = None;
        if !self.home {
            if let Some(index) = self.fleet.focused {
                self.fleet.focus(index, window, cx);
            }
        }
        cx.notify();
    }

    /// Esc: what is open inside the board goes first, then the board.
    pub(in crate::inbox) fn board_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        if b.picking {
            b.picking = false;
        } else if b.asking.is_some() {
            b.asking = None;
        } else if b.renaming.take().is_some() || std::mem::take(&mut b.adding_list) {
            self.board_blur(window, cx);
        } else if b.composing.is_some() {
            b.composing = None;
            b.inputs.composer.update(cx, |c, cx| c.set_value("", window, cx));
            self.board_blur(window, cx);
        } else if b.focused.is_some() {
            self.board_blur(window, cx);
        } else {
            self.close_board(window, cx);
            return;
        }
        cx.notify();
    }

    /// Take the keys back from a field, which keeps what was typed in it.
    fn board_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(b) = &self.board {
            b.focus.focus(window, cx);
        }
    }

    /// The board as the file has it; no file yet is a fresh board, written
    /// so agents see the same list ids (loadBoardForContext).
    fn board_load(&mut self, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        let read = cx.background_spawn(async move {
            match model::read(&dir) {
                Ok(Some((board, at))) => {
                    model::write_mirror(&dir, &board);
                    Some((board, at))
                }
                Ok(None) => {
                    let board = model::default_board();
                    let at = model::save(&dir, &board).ok().flatten();
                    model::write_mirror(&dir, &board);
                    Some((board, at))
                }
                // Corrupt or unreadable: a board in memory, the file left alone.
                Err(_) => None,
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let Some((board, at)) = read.await else { return };
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(b) = &mut this.board else { return };
                b.board = board;
                b.mtime = at;
                this.board_sync_detail(window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// The project the board follows, and its file's last time. Another
    /// project made current meanwhile gets its own board (setContext).
    fn board_follow_project(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<(String, Option<SystemTime>)> {
        let now = self.project_dir();
        let b = self.board.as_mut()?;
        let same = match (&b.dir, &now) {
            (Some(a), Some(n)) => presets::same_folder(a, n),
            (a, n) => a.is_none() && n.is_none(),
        };
        if !same {
            b.dir = now.clone();
            b.board = model::default_board();
            b.mtime = None;
            b.open = None;
            b.renaming = None;
            b.adding_list = false;
            b.composing = None;
            b.asking = None;
            b.picking = false;
            if let Some(dir) = now {
                self.board_load(dir, window, cx);
            }
            cx.notify();
            return None;
        }
        Some((b.dir.clone()?, b.mtime))
    }

    /// An agent changed the file. Not while you type or drag in the board
    /// (the next look catches up); a card it put in Done is said.
    fn board_reload(&mut self, board: Board, at: Option<SystemTime>, window: &mut Window, cx: &mut Context<Self>) {
        let busy = cx.has_active_drag();
        let Some(b) = &mut self.board else { return };
        if busy || b.focused.is_some() || b.renaming.is_some() || b.adding_list {
            return;
        }
        let before = model::done_card_ids(&b.board);
        let mut said = Vec::new();
        for list in board.lists.iter().filter(|l| l.title.trim().eq_ignore_ascii_case("done")) {
            for c in list.cards.iter().filter(|c| !before.contains(&c.id)) {
                let who = c.done.as_ref().and_then(|d| d.by.clone()).or_else(|| c.assignee.clone()).unwrap_or_else(|| "agent".to_string());
                said.push(format!("✅ {who} finished: {}", c.title));
            }
        }
        b.board = board;
        b.mtime = at;
        if let Some(dir) = b.dir.clone() {
            let mirror = b.board.clone();
            cx.background_spawn(async move { model::write_mirror(&dir, &mirror) }).detach();
        }
        self.board_sync_detail(window, cx);
        for text in said {
            self.toast(text, cx);
        }
        cx.notify();
    }

    /// Change the board: on screen at once, then on a fresh read of the file
    /// (withBoard). Without a project it lives in memory only.
    fn board_change(&mut self, f: impl Fn(&mut Board) + Send + Sync + 'static, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        let before = b.board.clone();
        f(&mut b.board);
        cx.notify();
        let Some(dir) = b.dir.clone() else { return };
        b.writes += 1;
        let no = b.writes;
        let lock = b.lock.clone();
        let write = cx.background_spawn(async move {
            let _one = lock.lock();
            model::change(&dir, &before, &f)
        });
        cx.spawn_in(window, async move |this, cx| {
            let got = write.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(b) = &mut this.board else { return };
                match got {
                    Ok((board, at)) => {
                        b.mtime = at;
                        if b.writes == no {
                            b.board = board;
                            this.board_sync_detail(window, cx);
                        }
                    }
                    Err(e) => this.toast(format!("Couldn't save the board: {e}"), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The open card's fields show what it holds now, unless you are in them.
    fn board_sync_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = &mut self.board else { return };
        let Some(id) = b.open.clone() else { return };
        let Some((l, c)) = model::find_card(&b.board, &id) else {
            b.open = None;
            return;
        };
        let card = b.board.lists[l].cards[c].clone();
        let focused = b.focused;
        let i = &b.inputs;
        if focused != Some(Field::Title) {
            i.title.update(cx, |t, cx| t.set_value(card.title.clone(), window, cx));
        }
        if focused != Some(Field::Desc) {
            i.desc.update(cx, |t, cx| t.set_value(card.desc.clone(), window, cx));
        }
        if focused != Some(Field::Due) {
            i.due.update(cx, |t, cx| t.set_value(card.due.clone().unwrap_or_default(), window, cx));
        }
    }

    /// The agents of the board's project: index, name, running.
    fn board_agents(&self, cx: &App) -> Vec<(usize, String, bool)> {
        let Some(dir) = self.board.as_ref().and_then(|b| b.dir.clone()) else { return Vec::new() };
        self.fleet
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| presets::same_folder(&s.spec.cwd, &dir))
            .map(|(i, s)| (i, s.spec.name.clone(), s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none())))
            .collect()
    }
}
