//! One terminal pane: emulator grid, PTY channels, paint + input.

use std::sync::mpsc::{channel, Receiver, Sender};

use agent_core::{CommandSpec, Registry};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{CursorShape, Processor};
use eframe::egui::{self, FontId, Pos2, Rect, Vec2};

use crate::color::{c32, rgb_of, BG_DEFAULT};
use crate::keys::key_bytes;
use crate::sessions_ui::Cli;
use crate::theme::{FOCUS, TEXT};

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
        Pane { id, name, spec, cli: None, session_id: None, term, processor: Processor::new(), rx, tx, term_rx, term_tx, cols, rows, exit_code: None, pending_title: None, last_output_ms: now_ms(), run: 1, worktree: None, merged: false }
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

    /// Paint the pane into `rect`: focus, resize, input, grid, cursor.
    pub fn paint(&mut self, ui: &mut egui::Ui, rect: Rect, registry: &mut Registry, font: &FontId, cell: (f32, f32)) {
        let (cell_w, cell_h) = cell;
        let id = ui.make_persistent_id(("term", &self.id));
        let resp = ui.interact(rect, id, egui::Sense::click());
        if resp.clicked() {
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

        if resp.has_focus() {
            let events: Vec<egui::Event> = ui.ctx().input(|i| i.events.clone());
            for ev in &events {
                match ev {
                    egui::Event::Text(t) => {
                        let _ = registry.write_input(&self.id, t.as_bytes());
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        if let Some(b) = key_bytes(*key, *modifiers) {
                            let _ = registry.write_input(&self.id, b);
                        }
                    }
                    egui::Event::MouseWheel { delta, .. } => {
                        let lines = (delta.y / 20.0).round() as i32;
                        if lines != 0 {
                            self.term.scroll_display(Scroll::Delta(lines));
                        }
                    }
                    _ => {}
                }
            }
            ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.0, FOCUS), egui::StrokeKind::Inside);
        }

        let painter = ui.painter();
        let content = self.term.renderable_content();
        for indexed in content.display_iter {
            let row = indexed.point.line.0 + content.display_offset as i32;
            if row < 0 || row as usize >= self.rows {
                continue;
            }
            let col = indexed.point.column.0;
            if col >= self.cols {
                continue;
            }
            let cell = &indexed.cell;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let mut fg = rgb_of(cell.fg);
            let mut bg = rgb_of(cell.bg);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let min = Pos2::new(rect.left() + col as f32 * cell_w, rect.top() + row as f32 * cell_h);
            if bg != BG_DEFAULT {
                painter.rect_filled(Rect::from_min_size(min, Vec2::new(cell_w, cell_h)), 0.0, c32(bg));
            }
            if cell.c != ' ' {
                painter.text(min, egui::Align2::LEFT_TOP, cell.c.to_string(), font.clone(), c32(fg));
            }
        }

        let cursor = &content.cursor;
        let crow = cursor.point.line.0 + content.display_offset as i32;
        let ccol = cursor.point.column.0;
        if crow >= 0 && (crow as usize) < self.rows && ccol < self.cols {
            let min = Pos2::new(rect.left() + ccol as f32 * cell_w, rect.top() + crow as f32 * cell_h);
            let cur = TEXT;
            match cursor.shape {
                CursorShape::Block => {
                    painter.rect_filled(Rect::from_min_size(min, Vec2::new(cell_w, cell_h)), 0.0, cur);
                }
                CursorShape::Underline => {
                    painter.rect_filled(
                        Rect::from_min_size(Pos2::new(min.x, min.y + cell_h - 2.0), Vec2::new(cell_w, 2.0)),
                        0.0,
                        cur,
                    );
                }
                CursorShape::Beam => {
                    painter.rect_filled(Rect::from_min_size(min, Vec2::new(2.0, cell_h)), 0.0, cur);
                }
                CursorShape::HollowBlock => {
                    painter.rect_stroke(Rect::from_min_size(min, Vec2::new(cell_w, cell_h)), 0.0, egui::Stroke::new(1.0, cur), egui::StrokeKind::Inside);
                }
                CursorShape::Hidden => {}
            }
        }
    }
}

/// Plain text of every visible row: what the painter reads, as strings.
pub fn grid_text<T: EventListener>(term: &Term<T>) -> Vec<String> {
    let content = term.renderable_content();
    let mut rows = vec![String::new(); 300];
    let mut n = 0;
    for indexed in content.display_iter {
        let row = indexed.point.line.0 + content.display_offset as i32;
        if row < 0 || row as usize >= rows.len() {
            continue;
        }
        n = n.max(row as usize + 1);
        rows[row as usize].push(indexed.cell.c);
    }
    rows.truncate(n);
    rows
}

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
    use alacritty_terminal::vte::ansi::{Color, NamedColor};
    use std::time::Instant;

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
