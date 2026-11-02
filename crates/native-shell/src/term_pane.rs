//! One terminal pane: emulator grid, PTY channels, paint + input.

use std::sync::mpsc::{channel, Receiver, Sender};

use agent_core::{CommandSpec, Registry};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use eframe::egui::{self, FontId, Pos2, Rect, Vec2};

use crate::keys::key_bytes;
use crate::sessions_ui::Cli;

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub enum Msg {
    Output(Vec<u8>),
    Exit(u32),
}

/// What the emulator asks the host to do: answer the PTY (DSR replies like
/// cursor position, without which shells wait forever) or retitle the window.
pub enum TermEvent {
    WriteToPty(String),
    Title(String),
}

/// Forwards the emulator's replies to the UI thread; the UI writes them back
/// to the PTY. VoidListener drops them, which hangs readline-style shells.
#[derive(Clone)]
pub struct PtyEvents {
    pub tx: Sender<TermEvent>,
}

impl EventListener for PtyEvents {
    fn send_event(&self, event: Event) {
        let out = match event {
            Event::PtyWrite(s) => TermEvent::WriteToPty(s),
            Event::Title(t) => TermEvent::Title(t),
            _ => return,
        };
        let _ = self.tx.send(out);
    }
}

/// Grid size for Term::new/resize (alacritty's own TermSize is test-only).
pub struct GridSize {
    pub rows: usize,
    pub cols: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Pane {
    pub id: String,
    pub name: String,
    pub spec: CommandSpec,
    pub cli: Option<Cli>,
    pub session_id: Option<String>,
    pub term: Term<PtyEvents>,
    processor: Processor,
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    term_rx: Receiver<TermEvent>,
    term_tx: Sender<TermEvent>,
    pub cols: usize,
    pub rows: usize,
    pub exit_code: Option<u32>,
    pub pending_title: Option<String>,
    /// ms the last PTY output arrived, for working/idle derivation.
    pub last_output_ms: u64,
    /// Process generation: 1 at spawn, +1 per restart. Stuck trails reset on change.
    pub run: u64,
    /// When this pane works in its own worktree: where, and which branch for what base.
    pub worktree: Option<WorktreeInfo>,
    /// Its branch landed and its worktree was removed.
    pub merged: bool,
    /// Launch flags kept across restarts (a skip-permissions flag).
    pub extra_args: Vec<String>,
    /// The command a custom agent runs (`gemini`, `ollama run llama3.1`).
    pub custom: Option<String>,
    /// The task it was started on (first line), for the Board.
    pub task: Option<String>,
    /// When it was spawned, to find its own session among the CLI's.
    pub started_ms: u64,
    /// What it used so far, when known.
    pub usage: Option<crate::usage_watch::UsageFig>,
    /// The plan limit its screen says it hit.
    pub limit: Option<supervise::limits::LimitHit>,
    /// The file its output is being recorded to.
    pub recording: Option<std::path::PathBuf>,
    /// Find in output (Ctrl+Shift+F).
    pub find: crate::term_search::FindBar,
    /// Wheel travel not yet a whole line (touchpads scroll in points).
    wheel: f32,
    /// What the last drag selected, kept: a CLI that redraws (a spinner, its
    /// status line) clears the live selection before Ctrl+C gets to it.
    last_sel: Option<String>,
}

/// An agent's isolated checkout: repo it came from, branch it works on.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub repo: String,
    pub branch: String,
    pub base: String,
    pub path: String,
}

impl Pane {
    pub fn new(id: String, name: String, spec: CommandSpec, cols: usize, rows: usize) -> Self {
        let (tx, rx) = channel();
        let (term_tx, term_rx) = channel();
        let config = Config { scrolling_history: 2000, ..Config::default() };
        let term = Term::new(config, &GridSize { rows, cols }, PtyEvents { tx: term_tx.clone() });
        Pane { id, name, spec, cli: None, session_id: None, term, processor: Processor::new(), rx, tx, term_rx, term_tx, cols, rows, exit_code: None, pending_title: None, last_output_ms: now_ms(), run: 1, worktree: None, merged: false, extra_args: Vec::new(), custom: None, task: None, started_ms: now_ms(), usage: None, limit: None, recording: None, find: Default::default(), wheel: 0.0, last_sel: None }
    }

    /// The visible screen as text, newest row last: what the state reads.
    pub fn screen_text(&self) -> String {
        grid_text(&self.term).join("\n")
    }

    #[cfg(test)]
    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
        self.last_output_ms = now_ms();
    }

    pub fn sender(&self) -> Sender<Msg> {
        self.tx.clone()
    }

    /// Fresh grid for a fresh process (restart).
    pub fn reset(&mut self) {
        let config = Config { scrolling_history: 2000, ..Config::default() };
        self.term = Term::new(config, &GridSize { rows: self.rows, cols: self.cols }, PtyEvents { tx: self.term_tx.clone() });
        self.exit_code = None;
        self.pending_title = None;
        self.run += 1;
    }

    /// Move PTY bytes into the grid and emulator replies back to the PTY.
    pub fn drain(&mut self, registry: &mut Registry) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Output(bytes) => {
                    self.processor.advance(&mut self.term, &bytes);
                    self.last_output_ms = now_ms();
                }
                Msg::Exit(code) => self.exit_code = Some(code),
            }
        }
        while let Ok(ev) = self.term_rx.try_recv() {
            match ev {
                TermEvent::WriteToPty(s) => {
                    let _ = registry.write_input(&self.id, s.as_bytes());
                }
                TermEvent::Title(t) => self.pending_title = Some(t),
            }
        }
    }

    fn copy_selection(&mut self, ctx: &egui::Context) {
        let live = self.term.selection_to_string().filter(|t| !t.is_empty());
        if let Some(t) = live.or_else(|| self.last_sel.take()) {
            ctx.copy_text(t);
        }
        self.term.selection = None;
        self.last_sel = None;
    }

    /// Something to copy: a live selection, or what the last drag selected.
    fn has_selection(&self) -> bool {
        self.term.selection.as_ref().is_some_and(|s| !s.is_empty()) || self.last_sel.is_some()
    }

    /// Take the keyboard back when nothing else holds it, as a terminal
    /// window does: a click on the top bar or the list let go of the focus
    /// and the next keys went nowhere. Call right after `paint`.
    pub fn claim_focus(&self, ui: &egui::Ui) {
        let id = ui.make_persistent_id(("term", &self.id));
        if ui.memory(|m| m.focused().is_none()) && !egui::Popup::is_any_open(ui.ctx()) {
            ui.memory_mut(|m| m.request_focus(id));
        }
    }

    /// Paint the pane into `rect`: focus, resize, input, grid, cursor.
    pub fn paint(&mut self, ui: &mut egui::Ui, rect: Rect, registry: &mut Registry, font: &FontId, cell: (f32, f32)) {
        let (cell_w, cell_h) = cell;
        let id = ui.make_persistent_id(("term", &self.id));
        let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
        if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
            resp.request_focus();
        }

        // Breathe: glyphs sit inside a small margin, the grid fits that.
        let rect = rect.shrink2(Vec2::new(4.0, 2.0));
        // Resize the grid (and the PTY) to the allotted rect.
        let cols = ((rect.width() / cell_w).floor() as usize).clamp(20, 400);
        let rows = ((rect.height() / cell_h).floor() as usize).clamp(5, 200);
        if cols != self.cols || rows != self.rows {
            self.cols = cols;
            self.rows = rows;
            self.term.resize(GridSize { rows, cols });
            let _ = registry.resize(&self.id, cols as u16, rows as u16);
        }
        let bracketed = self.term.mode().contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE);
        // A new press starts over; a finished drag keeps what it selected.
        if resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
            self.last_sel = None;
        }
        let picked = crate::term_select::mouse(&mut self.term, &resp, rect, cell);
        // Letting go of a selection copies it, as in a console window.
        if resp.drag_stopped() || resp.double_clicked() {
            self.last_sel = self.term.selection_to_string().filter(|t| !t.trim().is_empty());
            if let Some(t) = &self.last_sel {
                ui.ctx().copy_text(t.clone());
            }
        }
        if let Some(crate::term_select::MenuPick::Paste) = picked {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        // Files dropped from Explorer onto this pane: type their paths.
        let (dropped, at) = ui.ctx().input(|i| (i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect::<Vec<_>>(), i.pointer.hover_pos()));
        if !dropped.is_empty() && at.is_some_and(|p| rect.contains(p)) {
            let _ = registry.write_input(&self.id, &crate::paste::paste_bytes(&crate::term_select::drop_text(&dropped), bracketed));
            resp.request_focus();
        }

        // The wheel works wherever the pointer is, focused or not.
        let hovered = ui.ctx().input(|i| i.pointer.hover_pos()).is_some_and(|p| rect.contains(p));
        if hovered {
            let wheel: Vec<(egui::MouseWheelUnit, f32, Pos2)> = ui.ctx().input(|i| {
                let at = i.pointer.hover_pos().unwrap_or(rect.min);
                i.events.iter().filter_map(|e| if let egui::Event::MouseWheel { unit, delta, .. } = e { Some((*unit, delta.y, at)) } else { None }).collect()
            });
            for (unit, dy, at) in wheel {
                // Three lines a notch, as terminals scroll.
                self.wheel += match unit {
                    egui::MouseWheelUnit::Line => dy * 3.0,
                    egui::MouseWheelUnit::Page => dy * self.rows as f32,
                    egui::MouseWheelUnit::Point => dy / cell_h,
                };
                let lines = self.wheel.trunc();
                if lines == 0.0 {
                    continue;
                }
                self.wheel -= lines;
                let cell_at = (((at.x - rect.left()) / cell_w) as usize + 1, ((at.y - rect.top()) / cell_h) as usize + 1);
                match crate::keys::wheel_bytes(*self.term.mode(), lines > 0.0, lines.abs() as usize, cell_at) {
                    Some(b) => {
                        let _ = registry.write_input(&self.id, &b);
                    }
                    None => self.term.scroll_display(Scroll::Delta(lines as i32)),
                }
            }
        }

        if resp.has_focus() {
            // Arrows, Tab and Esc belong to the program, not to egui's focus
            // moves: without this the first arrow in a CLI's menu took the
            // focus away and the next keys never reached it.
            ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
            let app_cursor = self.term.mode().contains(alacritty_terminal::term::TermMode::APP_CURSOR);
            let events: Vec<egui::Event> = ui.ctx().input(|i| i.events.clone());
            for ev in &events {
                match ev {
                    egui::Event::Text(t) => {
                        // Typing while scrolled back jumps to the prompt.
                        self.term.scroll_display(Scroll::Bottom);
                        let _ = registry.write_input(&self.id, t.as_bytes());
                    }
                    egui::Event::Key { key: egui::Key::F, pressed: true, modifiers, .. } if modifiers.ctrl && modifiers.shift => self.find.toggle(),
                    // Copy and paste with Shift, as in Windows Terminal: never an
                    // interrupt sent by a hand used to it.
                    egui::Event::Key { key: egui::Key::C, pressed: true, modifiers, .. } if modifiers.ctrl && modifiers.shift => self.copy_selection(ui.ctx()),
                    egui::Event::Key { key: egui::Key::V, pressed: true, modifiers, .. } if modifiers.ctrl && modifiers.shift => ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        let app = if app_cursor { crate::keys::app_cursor_bytes(*key, *modifiers) } else { None };
                        if let Some(b) = app.or_else(|| key_bytes(*key, *modifiers)) {
                            self.term.scroll_display(Scroll::Bottom);
                            let _ = registry.write_input(&self.id, b);
                        }
                    }
                    // Ctrl+C copies a selection; with none it interrupts.
                    egui::Event::Copy if self.has_selection() => self.copy_selection(ui.ctx()),
                    egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_) | egui::Event::PasteImage(_) => {
                        if let Some(bytes) = crate::paste::clipboard_bytes(ev, bracketed) {
                            let _ = registry.write_input(&self.id, &bytes);
                        }
                    }
                    _ => {}
                }
            }
        }

        paint_grid(ui.painter(), &self.term, rect, (self.cols, self.rows), font, cell);
        self.find.show(ui, &self.id, rect, &mut self.term);
    }
}

pub use crate::term_paint::{cell_metrics, grid_text, paint_grid};

#[cfg(test)]
pub fn grid_size_of<T: EventListener>(term: &Term<T>) -> (usize, usize) {
    let content = term.renderable_content();
    let (mut rows, mut cols) = (0, 0);
    for indexed in content.display_iter {
        let row = indexed.point.line.0 + content.display_offset as i32;
        if row >= 0 {
            rows = rows.max(row as usize + 1);
            cols = cols.max(indexed.point.column.0 + 1);
        }
    }
    (rows, cols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
    use std::time::Instant;

    #[test]
    fn copies_what_was_selected_after_a_redraw_cleared_it() {
        let spec = agent_core::launch::launch_spec("cmd.exe", &[]);
        let mut p = Pane::new("t".into(), "t".into(), spec, 80, 24);
        p.last_sel = Some("keep me".into());
        assert!(p.term.selection.is_none(), "the live selection is gone");
        let ctx = egui::Context::default();
        let out = ctx.run_ui(egui::RawInput::default(), |ui| p.copy_selection(ui.ctx()));
        let copied = out.platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "keep me"));
        out.drop_without_applying_deltas();
        assert!(copied);
        assert!(p.last_sel.is_none(), "copied once");
    }

    #[test]
    fn colors_and_cursor_read_back() {
        let mut term = Term::new(Config::default(), &GridSize { rows: 24, cols: 80 }, VoidListener);
        let mut proc: Processor = Processor::new();
        proc.advance(&mut term, b"\x1b[31mRED\x1b[0m plain");
        let content = term.renderable_content();
        let reds: Vec<_> = content
            .display_iter
            .filter(|i| i.cell.c == 'R' || i.cell.c == 'E' || i.cell.c == 'D')
            .collect();
        assert_eq!(reds.len(), 3);
        for i in reds {
            assert_eq!(i.cell.fg, Color::Named(NamedColor::Red));
        }
        assert_eq!(content.cursor.shape, CursorShape::Block);
    }

    #[test]
    fn scroll_up_and_down_moves_the_offset() {
        let mut term = Term::new(Config { scrolling_history: 200, ..Config::default() }, &GridSize { rows: 24, cols: 80 }, VoidListener);
        let mut proc: Processor = Processor::new();
        let mut out = Vec::new();
        for k in 0..100 {
            out.extend_from_slice(format!("line-{k:03}\r\n").as_bytes());
        }
        proc.advance(&mut term, &out);
        assert_eq!(term.renderable_content().display_offset, 0);
        term.scroll_display(Scroll::Delta(10));
        assert_eq!(term.renderable_content().display_offset, 10);
        let grid = grid_text(&term);
        assert!(grid.iter().any(|r| r.contains("line-0")), "scrolled view shows old lines: {grid:?}");
        term.scroll_display(Scroll::Delta(-10));
        assert_eq!(term.renderable_content().display_offset, 0);
    }

    #[test]
    fn resize_changes_the_grid() {
        let mut term = Term::new(Config::default(), &GridSize { rows: 24, cols: 80 }, VoidListener);
        assert_eq!(grid_size_of(&term), (24, 80));
        term.resize(GridSize { rows: 30, cols: 100 });
        assert_eq!(grid_size_of(&term), (30, 100));
    }

    #[test]
    fn throughput_probe_advance_and_grid_walk() {
        // Observation, not a gate: how fast 1 MB of shell-like output parses
        // and how long one full grid read (what each frame paints) takes.
        let mut term = Term::new(Config { scrolling_history: 2000, ..Config::default() }, &GridSize { rows: 40, cols: 120 }, VoidListener);
        let mut proc: Processor = Processor::new();
        let line: Vec<u8> = format!("\x1b[32m{:120}\x1b[0m\r\n", "x".repeat(110)).into_bytes();
        let mut blob = Vec::with_capacity(1024 * 1024);
        while blob.len() < 1024 * 1024 {
            blob.extend_from_slice(&line);
        }
        let t = Instant::now();
        proc.advance(&mut term, &blob);
        let parse_ms = t.elapsed().as_millis();
        let t = Instant::now();
        let grid = grid_text(&term);
        let walk_ms = t.elapsed().as_millis();
        assert_eq!(grid.len(), 40);
        eprintln!("throughput: 1 MB parsed in {parse_ms} ms, full grid walk in {walk_ms} ms");
    }
}
