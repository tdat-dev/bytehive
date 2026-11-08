//! One terminal: a ConPTY session, alacritty's grid, and GPUI painting it. It
//! keeps the feel of today's xterm pane: keys go whole to the program (keys.rs),
//! selecting copies, right click copies, Ctrl+V pastes, OSC 52 sets the clipboard.

use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_core::{Agent, CommandSpec};
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point as Cell, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb};
use gpui_kit::*;
use portable_pty::PtySize;

use crate::{boxdraw, keys};

const FONT_FAMILY: &str = "Geist Mono";
const FONT_SIZE: f32 = 19.0;
const LINE_HEIGHT: f32 = 1.6;
/// History kept in view, and once out of sight: today's app keeps the same
/// (src/terminal.ts SCROLLBACK_SHOWN, SCROLLBACK_HIDDEN).
pub const SCROLLBACK: usize = 5_000;
pub const SCROLLBACK_HIDDEN: usize = 1_000;
/// Output arriving faster than this waits for the next frame.
const FRAME: Duration = Duration::from_millis(8);

const BACKGROUND: Rgb = Rgb { r: 0x0e, g: 0x0f, b: 0x11 };
const FOREGROUND: Rgb = Rgb { r: 0xe2, g: 0xe8, b: 0xf0 };
const CURSOR: Rgb = Rgb { r: 0xf4, g: 0xf4, b: 0xf5 };

/// The ANSI 16 of today's dark terminal (src/termtheme.ts, LIGHT_TEXT).
const ANSI: [u32; 16] = [
    0x1e293b, 0xef4444, 0x22c55e, 0xeab308, 0x3b82f6, 0xd946ef, 0x06b6d4, 0xf8fafc, 0x7c8aa0, 0xf87171, 0x4ade80, 0xfde047, 0x60a5fa, 0xe879f9,
    0x22d3ee, 0xffffff,
];

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn from_hex(hex: u32) -> Rgb {
    Rgb { r: (hex >> 16) as u8, g: (hex >> 8) as u8, b: hex as u8 }
}

fn hsla(c: Rgb) -> Hsla {
    rgb(((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32).into()
}

fn dimmed(c: Rgb) -> Rgb {
    Rgb { r: (c.r as f32 * 0.66) as u8, g: (c.g as f32 * 0.66) as u8, b: (c.b as f32 * 0.66) as u8 }
}

/// The 256 colour cube and grey ramp above the ANSI 16.
fn indexed(i: u8) -> Rgb {
    match i {
        0..=15 => from_hex(ANSI[i as usize]),
        16..=231 => {
            let step = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            let n = i - 16;
            Rgb { r: step(n / 36), g: step(n / 6 % 6), b: step(n % 6) }
        }
        _ => {
            let v = 8 + 10 * (i - 232);
            Rgb { r: v, g: v, b: v }
        }
    }
}

fn resolve(color: Color, set: &Colors) -> Rgb {
    match color {
        Color::Spec(c) => c,
        Color::Indexed(i) => set[i as usize].unwrap_or_else(|| indexed(i)),
        Color::Named(n) => set[n].unwrap_or_else(|| named(n)),
    }
}

fn named(n: NamedColor) -> Rgb {
    let i = n as usize;
    match n {
        NamedColor::Background => BACKGROUND,
        NamedColor::Cursor => CURSOR,
        NamedColor::Foreground | NamedColor::BrightForeground => FOREGROUND,
        NamedColor::DimForeground => dimmed(FOREGROUND),
        _ if i < 16 => from_hex(ANSI[i]),
        _ => dimmed(from_hex(ANSI[(i - NamedColor::DimBlack as usize) % 8])),
    }
}

/// Grid size for Term::new and resize (alacritty's own TermSize is test-only).
struct GridSize {
    rows: usize,
    cols: usize,
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

/// What the emulator asks of us. Dropping PtyWrite would hang the session:
/// ConPTY waits for the answer to its cursor query.
#[derive(Clone)]
struct Listener {
    writer: Sender<Vec<u8>>,
    copies: Arc<Mutex<Vec<String>>>,
    cell_px: Arc<Mutex<WindowSize>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(s) => {
                let _ = self.writer.send(s.into_bytes());
            }
            Event::ClipboardStore(_, text) => self.copies.lock().unwrap().push(text),
            Event::ColorRequest(i, answer) => {
                let color = match i {
                    256 => FOREGROUND,
                    257 => BACKGROUND,
                    258 => CURSOR,
                    _ => indexed(i as u8),
                };
                let _ = self.writer.send(answer(color).into_bytes());
            }
            Event::TextAreaSizeRequest(answer) => {
                let size = *self.cell_px.lock().unwrap();
                let _ = self.writer.send(answer(size).into_bytes());
            }
            _ => {}
        }
    }
}

/// A stretch of one row painted with one style.
struct Span {
    row: usize,
    col: usize,
    text: String,
    fg: Rgb,
    weight: FontWeight,
    italic: bool,
    underline: bool,
}

/// A filled stretch of cells: a cell background or the selection.
struct Fill {
    row: usize,
    col: usize,
    cols: usize,
    color: Hsla,
}

pub struct Terminal {
    term: Arc<Mutex<Term<Listener>>>,
    /// The CLI and its whole process tree: a kill-on-close job ends them all
    /// when this terminal is dropped.
    agent: Arc<Mutex<Agent>>,
    /// Its exit code once it ended.
    exit: Arc<Mutex<Option<u32>>>,
    /// When output last arrived, ms since the epoch: the queue reads working
    /// or idle from it.
    last_output: Arc<AtomicU64>,
    writer: Sender<Vec<u8>>,
    copies: Arc<Mutex<Vec<String>>>,
    cell_px: Arc<Mutex<WindowSize>>,
    focus: FocusHandle,
    cols: usize,
    rows: usize,
    cell_w: Pixels,
    cell_h: Pixels,
    origin: Point<Pixels>,
    /// What an IME is composing, shown at the cursor until it commits.
    preedit: String,
    selecting: bool,
    /// Wheel travel not yet a whole line (touchpads scroll in pixels).
    wheel: f32,
}

impl Terminal {
    /// Start `spec` in a new terminal. The CLI is spawned before the entity
    /// exists, so a launch failure comes back as an error, not a dead pane.
    pub fn spawn(spec: CommandSpec, cx: &mut App) -> anyhow::Result<Entity<Self>> {
        let (cols, rows) = (100, 30);
        let size = PtySize { rows: rows as u16, cols: cols as u16, pixel_width: 0, pixel_height: 0 };
        let (writer, keys_rx) = channel::<Vec<u8>>();
        let copies = Arc::new(Mutex::new(Vec::new()));
        let cell_px = Arc::new(Mutex::new(WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: 8, cell_height: 16 }));
        let listener = Listener { writer: writer.clone(), copies: copies.clone(), cell_px: cell_px.clone() };
        let config = Config { scrolling_history: SCROLLBACK, ..Config::default() };
        let term = Arc::new(Mutex::new(Term::new(config, &GridSize { rows, cols }, listener)));

        // Parsing happens here, off the UI thread. One wake per frame at most,
        // however many chunks arrive: a bounded channel of one drops the rest.
        let (wake, woken) = async_channel::bounded::<()>(1);
        let grid = term.clone();
        let last_output = Arc::new(AtomicU64::new(now_ms()));
        let heard = last_output.clone();
        let mut processor: Processor = Processor::new();
        let output_wake = wake.clone();
        let exit = Arc::new(Mutex::new(None));
        let exited = exit.clone();
        let agent = Agent::spawn(
            &spec,
            size,
            move |bytes: &[u8]| {
                keys::watch_mode(bytes);
                processor.advance(&mut *grid.lock().unwrap(), bytes);
                heard.store(now_ms(), Ordering::Relaxed);
                let _ = output_wake.try_send(());
            },
            move |code| {
                *exited.lock().unwrap() = Some(code);
                let _ = wake.try_send(());
            },
        )?;
        let agent = Arc::new(Mutex::new(agent));
        // Weak on purpose: the agent's output callback owns the grid, the grid
        // owns a writer, and the writer keeps this thread alive. A strong
        // handle here would close that loop, and stopping would never drop the
        // agent nor kill its tree.
        let to_pty = Arc::downgrade(&agent);
        std::thread::spawn(move || {
            for bytes in keys_rx {
                let Some(agent) = to_pty.upgrade() else { break };
                if agent.lock().unwrap().write_input(&bytes).is_err() {
                    break;
                }
            }
        });

        Ok(cx.new(|cx| {
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                while woken.recv().await.is_ok() {
                    if this.update(cx, |this, cx| this.on_output(cx)).is_err() {
                        break;
                    }
                    cx.background_executor().timer(FRAME).await;
                }
            })
            .detach();
            Self {
                term,
                agent,
                exit,
                last_output,
                writer,
                copies,
                cell_px,
                focus: cx.focus_handle(),
                cols,
                rows,
                cell_w: px(8.0),
                cell_h: px(16.0),
                origin: Point::default(),
                preedit: String::new(),
                selecting: false,
                wheel: 0.0,
            }
        }))
    }

    /// Take the keyboard: keys.rs hands keys to this terminal from now on.
    pub fn claim_keys(&self) {
        keys::set_sink(self.writer.clone());
    }

    /// The CLI's exit code once it has ended.
    pub fn exit_code(&self) -> Option<u32> {
        *self.exit.lock().unwrap()
    }

    /// How long since the CLI last wrote anything, in ms.
    pub fn quiet_for_ms(&self) -> u64 {
        now_ms().saturating_sub(self.last_output.load(Ordering::Relaxed))
    }

    /// How many lines of history to keep; older ones are dropped.
    pub fn keep_lines(&self, lines: usize) {
        self.term.lock().unwrap().set_options(Config { scrolling_history: lines, ..Config::default() });
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn on_output(&mut self, cx: &mut Context<Self>) {
        for text in self.copies.lock().unwrap().drain(..) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
        cx.notify();
    }

    fn write(&self, bytes: impl Into<Vec<u8>>) {
        let _ = self.writer.send(bytes.into());
    }

    fn fit(&mut self, bounds: Bounds<Pixels>, cell_w: Pixels, cell_h: Pixels) {
        self.origin = bounds.origin;
        self.cell_w = cell_w;
        self.cell_h = cell_h;
        let cols = ((bounds.size.width / cell_w) as usize).max(2);
        let rows = ((bounds.size.height / cell_h) as usize).max(1);
        if (cols, rows) == (self.cols, self.rows) {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.term.lock().unwrap().resize(GridSize { rows, cols });
        let _ = self.agent.lock().unwrap().resize(cols as u16, rows as u16);
        *self.cell_px.lock().unwrap() =
            WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: f32::from(cell_w) as u16, cell_height: f32::from(cell_h) as u16 };
    }

    /// The cell under a window position, and which half of it.
    fn cell_at(&self, position: Point<Pixels>) -> (usize, usize, Side) {
        let x = f32::from(position.x - self.origin.x).max(0.0) / f32::from(self.cell_w);
        let y = f32::from(position.y - self.origin.y).max(0.0) / f32::from(self.cell_h);
        let col = (x as usize).min(self.cols - 1);
        let row = (y as usize).min(self.rows - 1);
        (col, row, if x.fract() < 0.5 { Side::Left } else { Side::Right })
    }

    /// A program holding the mouse gets it, unless Shift asks for a selection.
    fn program_has_mouse(&self, modifiers: &Modifiers) -> bool {
        !modifiers.shift && self.term.lock().unwrap().mode().intersects(TermMode::MOUSE_MODE)
    }

    fn report_mouse(&self, button: u8, col: usize, row: usize, pressed: bool) {
        self.write(format!("\x1b[<{button};{};{}{}", col + 1, row + 1, if pressed { 'M' } else { 'm' }));
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        let (col, row, side) = self.cell_at(event.position);
        if event.button == MouseButton::Right {
            self.copy_selection(cx);
            return;
        }
        if self.program_has_mouse(&event.modifiers) {
            self.report_mouse(mouse_code(event.button), col, row, true);
            return;
        }
        if event.button == MouseButton::Left {
            let mut term = self.term.lock().unwrap();
            let point = Cell::new(Line(row as i32 - term.grid().display_offset() as i32), Column(col));
            let kind = match event.click_count {
                2 => SelectionType::Semantic,
                3 => SelectionType::Lines,
                _ => SelectionType::Simple,
            };
            term.selection = Some(Selection::new(kind, point, side));
            self.selecting = true;
            cx.notify();
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (col, row, side) = self.cell_at(event.position);
        if self.selecting {
            let mut term = self.term.lock().unwrap();
            let point = Cell::new(Line(row as i32 - term.grid().display_offset() as i32), Column(col));
            if let Some(selection) = term.selection.as_mut() {
                selection.update(point, side);
            }
            cx.notify();
        } else if event.pressed_button.is_some() && self.term.lock().unwrap().mode().intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) {
            self.report_mouse(mouse_code(event.pressed_button.unwrap_or(MouseButton::Left)) + 32, col, row, true);
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.selecting = false;
            self.copy_selection(cx);
        } else if self.program_has_mouse(&event.modifiers) {
            let (col, row, _) = self.cell_at(event.position);
            self.report_mouse(mouse_code(event.button), col, row, false);
        }
    }

    fn copy_selection(&self, cx: &mut Context<Self>) {
        let text = self.term.lock().unwrap().selection_to_string();
        if let Some(text) = text.filter(|t| !t.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn on_wheel(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.wheel += match event.delta {
            ScrollDelta::Lines(lines) => lines.y * 3.0,
            ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / f32::from(self.cell_h),
        };
        let lines = self.wheel as i32;
        if lines == 0 {
            return;
        }
        self.wheel -= lines as f32;
        let (col, row, _) = self.cell_at(event.position);
        let mode = *self.term.lock().unwrap().mode();
        if self.program_has_mouse(&event.modifiers) {
            for _ in 0..lines.abs() {
                self.report_mouse(if lines > 0 { 64 } else { 65 }, col, row, true);
            }
        } else if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let arrow = match (lines > 0, mode.contains(TermMode::APP_CURSOR)) {
                (true, false) => "\x1b[A",
                (false, false) => "\x1b[B",
                (true, true) => "\x1bOA",
                (false, true) => "\x1bOB",
            };
            self.write(arrow.repeat(lines.unsigned_abs() as usize));
        } else {
            self.term.lock().unwrap().scroll_display(Scroll::Delta(lines));
            cx.notify();
        }
    }

    /// Only Ctrl+V reaches here: keys.rs hands every other key to the program.
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control && keystroke.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.paste_text(&text);
            }
            cx.stop_propagation();
        }
    }

    fn paste_text(&self, text: &str) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.term.lock().unwrap().mode().contains(TermMode::BRACKETED_PASTE) {
            self.write(format!("\x1b[200~{text}\x1b[201~"));
        } else {
            self.write(text);
        }
    }

    /// One frame of the grid as flat lists, so the lock is not held while painting.
    fn snapshot(&self) -> (Vec<Fill>, Vec<Span>, Vec<(usize, usize, char, Rgb)>, Option<(usize, usize, CursorShape)>) {
        let term = self.term.lock().unwrap();
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let selection = content.selection;
        let selected: Hsla = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.22 }.into();

        let mut fills: Vec<Fill> = Vec::new();
        let mut spans: Vec<Span> = Vec::new();
        let mut drawn: Vec<(usize, usize, char, Rgb)> = Vec::new();
        let push_fill = |fills: &mut Vec<Fill>, row: usize, col: usize, cols: usize, color: Hsla| match fills.last_mut() {
            Some(last) if last.row == row && last.col + last.cols == col && last.color == color => last.cols += cols,
            _ => fills.push(Fill { row, col, cols, color }),
        };

        for indexed in content.display_iter {
            let cell = indexed.cell;
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let row = (indexed.point.line.0 + offset) as usize;
            let col = indexed.point.column.0;
            let wide = cell.flags.contains(Flags::WIDE_CHAR);
            let width = if wide { 2 } else { 1 };
            let (mut fg, mut bg) = (resolve(cell.fg, content.colors), resolve(cell.bg, content.colors));
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.flags.contains(Flags::DIM) {
                fg = dimmed(fg);
            }
            if bg != BACKGROUND {
                push_fill(&mut fills, row, col, width, hsla(bg));
            }
            if selection.is_some_and(|s| s.contains(indexed.point)) {
                push_fill(&mut fills, row, col, width, selected);
            }
            if cell.c == ' ' || cell.flags.contains(Flags::HIDDEN) {
                continue;
            }
            let weight = if cell.flags.contains(Flags::BOLD) { FontWeight::MEDIUM } else { FontWeight::NORMAL };
            let italic = cell.flags.contains(Flags::ITALIC);
            let underline = cell.flags.intersects(Flags::ALL_UNDERLINES);
            let mut text = String::from(cell.c);
            if let Some(marks) = cell.zerowidth() {
                text.extend(marks);
            }
            if boxdraw::is_drawn(cell.c) {
                drawn.push((row, col, cell.c, fg));
                continue;
            }
            // Only plain ASCII runs together: anything else may come from a
            // fallback font with its own width, and must stay on its cell.
            let ascii = cell.c.is_ascii();
            match spans.last_mut() {
                Some(last)
                    if ascii
                        && last.text.is_ascii()
                        && last.row == row
                        && last.col + last.text.chars().count() == col
                        && last.fg == fg
                        && last.weight == weight
                        && last.italic == italic
                        && last.underline == underline
                        && text.chars().count() == 1 =>
                {
                    last.text.push_str(&text)
                }
                _ => spans.push(Span { row, col, text, fg, weight, italic, underline }),
            }
        }

        let cursor = content.cursor;
        let at = (cursor.shape != CursorShape::Hidden && content.display_offset == 0)
            .then(|| ((cursor.point.line.0 + offset) as usize, cursor.point.column.0, cursor.shape));
        (fills, spans, drawn, at)
    }

    fn paint_grid(&self, bounds: Bounds<Pixels>, font_base: &Font, window: &mut Window, cx: &mut App) {
        let (cell_w, cell_h) = (self.cell_w, self.cell_h);
        let at = |row: usize, col: usize| bounds.origin + point(cell_w * col as f32, cell_h * row as f32);
        window.paint_quad(fill(bounds, hsla(BACKGROUND)));

        let (fills, spans, drawn, cursor) = self.snapshot();
        for f in fills {
            window.paint_quad(fill(Bounds::new(at(f.row, f.col), size(cell_w * f.cols as f32, cell_h)), f.color));
        }

        let focused = self.focus.is_focused(window);
        if let Some((row, col, shape)) = cursor {
            let origin = at(row, col);
            let color = hsla(CURSOR);
            let quad = match shape {
                CursorShape::Beam => fill(Bounds::new(origin, size(px(1.5), cell_h)), color),
                CursorShape::Underline => fill(Bounds::new(origin + point(px(0.0), cell_h - px(2.0)), size(cell_w, px(2.0))), color),
                _ if focused => fill(Bounds::new(origin, size(cell_w, cell_h)), color),
                _ => outline(Bounds::new(origin, size(cell_w, cell_h)), color, BorderStyle::Solid),
            };
            window.paint_quad(quad);
        }

        for (row, col, c, color) in drawn {
            boxdraw::paint(c, at(row, col), size(cell_w, cell_h), hsla(color), window);
        }

        for span in spans {
            let mut font = font_base.clone();
            if span.text.chars().next().is_some_and(is_emoji) {
                font.fallbacks = None;
            }
            font.weight = span.weight;
            if span.italic {
                font.style = FontStyle::Italic;
            }
            let mut color = span.fg;
            if focused && cursor.is_some_and(|(row, col, shape)| matches!(shape, CursorShape::Block) && row == span.row && col == span.col && span.text.chars().count() == 1) {
                color = BACKGROUND;
            }
            let color = hsla(color);
            let underline = span.underline.then_some(UnderlineStyle { thickness: px(1.0), color: Some(color), wavy: false });
            let run = TextRun { len: span.text.len(), font, color, background_color: None, underline, strikethrough: None };
            let line = window.text_system().shape_line(span.text.into(), px(FONT_SIZE), &[run], None);
            let _ = line.paint(at(span.row, span.col), cell_h, TextAlign::Left, None, window, cx);
        }

        if let Some((row, col, _)) = cursor.filter(|_| !self.preedit.is_empty()) {
            let color = hsla(FOREGROUND);
            let underline = Some(UnderlineStyle { thickness: px(1.0), color: Some(color), wavy: false });
            let run = TextRun { len: self.preedit.len(), font: font_base.clone(), color, background_color: Some(hsla(BACKGROUND)), underline, strikethrough: None };
            let line = window.text_system().shape_line(self.preedit.clone().into(), px(FONT_SIZE), &[run], None);
            let _ = line.paint(at(row, col), cell_h, TextAlign::Left, None, window, cx);
        }
    }
}

/// Pictures that keep their colour; other symbols stay plain text glyphs.
fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1f300..=0x1faff | 0x2705 | 0x2728 | 0x274c | 0x274e | 0x26a1 | 0x2b50)
}

fn mouse_code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        _ => 0,
    }
}

impl EntityInputHandler for Terminal {
    fn text_for_range(&mut self, _: Range<usize>, _: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.preedit.is_empty()).then(|| 0..self.preedit.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        cx.notify();
    }

    /// An IME committed: its text goes to the program as typed characters.
    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        self.write(keys::text_records(text));
        cx.notify();
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit = text.to_string();
        cx.notify();
    }

    /// Where the IME's candidate window should sit: at the cursor.
    fn bounds_for_range(&mut self, _: Range<usize>, element: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let term = self.term.lock().unwrap();
        let cursor = term.grid().cursor.point;
        let origin = element.origin + point(self.cell_w * cursor.column.0 as f32, self.cell_h * cursor.line.0 as f32);
        Some(Bounds::new(origin, size(self.cell_w, self.cell_h)))
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}

impl Render for Terminal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let focus = self.focus.clone();
        let mut font_base = font(FONT_FAMILY);
        // Symbols before emoji: Claude Code's bullet must stay a plain glyph.
        font_base.fallbacks = Some(FontFallbacks::from_fonts(vec!["Segoe UI Symbol".to_string(), "Cascadia Mono".to_string(), "Consolas".to_string()]));
        let grid = canvas(
            {
                let this = this.clone();
                let font_base = font_base.clone();
                move |bounds, window, cx| {
                    let text = window.text_system();
                    let id = text.resolve_font(&font_base);
                    let cell_w = text.advance(id, px(FONT_SIZE), 'm').map(|s| s.width).unwrap_or(px(8.0));
                    let cell_h = px((FONT_SIZE * LINE_HEIGHT).round());
                    this.update(cx, |this, _| this.fit(bounds, cell_w, cell_h));
                }
            },
            move |bounds, _, window, cx| {
                keys::set_active(focus.is_focused(window));
                window.handle_input(&focus, ElementInputHandler::new(bounds, this.clone()), cx);
                this.update(cx, |this, cx| this.paint_grid(bounds, &font_base, window, cx));
            },
        )
        .size_full();

        div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_scroll(cx.listener(Self::on_wheel))
            .child(grid)
    }
}
