//! One terminal: a ConPTY session, alacritty's grid, and GPUI painting it. It
//! keeps the feel of today's xterm pane: keys go whole to the program (keys.rs),
//! selecting copies, right click copies, Ctrl+V pastes, OSC 52 sets the clipboard.


mod colors;
mod focus;
mod mouse;
mod paint;
mod ime;
mod remote;

pub use remote::RemoteTerm;

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

// Named, not globbed: hsla() would clash with gpui_kit's own.
use colors::{dimmed, hsla, indexed, resolve};

const FONT_FAMILY: &str = "Geist Mono";
/// Text size in today's app's numbers (Settings, 10 to 20, 15 by default) is
/// drawn this much larger: the 15 there looks like 19 here.
const FONT_SCALE: f32 = 19.0 / 15.0;
static FONT_SIZE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(19.0f32.to_bits());

/// The size every terminal draws its text at.
pub fn font_size() -> f32 {
    f32::from_bits(FONT_SIZE.load(std::sync::atomic::Ordering::Relaxed))
}

/// Settings → Terminal → Font size: every terminal picks it up on its next frame.
pub fn set_font_size(setting: u32) {
    FONT_SIZE.store((setting as f32 * FONT_SCALE).round().to_bits(), std::sync::atomic::Ordering::Relaxed);
}
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
    /// When you last typed or pasted into it, ms since the epoch (0: never).
    /// Output within a moment of it is its echo, not the agent at work.
    last_input: Arc<AtomicU64>,
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
    /// Its own text size, while the Grid grows it; else the setting's.
    font: Option<f32>,
    /// It had the keyboard when last told (focus.rs), and what watches for that.
    had_focus: bool,
    focus_watch: Vec<Subscription>,
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
                last_input: Arc::new(AtomicU64::new(0)),
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
                font: None,
                had_focus: false,
                focus_watch: Vec::new(),
            }
        }))
    }

    /// Take the keyboard: keys.rs hands keys to this terminal from now on.
    pub fn claim_keys(&self) {
        keys::set_sink(self.writer.clone(), self.last_input.clone());
    }

    /// When you last typed or pasted into it, ms since the epoch (0: never).
    pub fn last_input_ms(&self) -> u64 {
        self.last_input.load(Ordering::Relaxed)
    }

    /// Your own input is on its way: what comes out next is its echo.
    pub(super) fn note_input(&self) {
        self.last_input.store(now_ms(), Ordering::Relaxed);
    }

    /// The CLI's exit code once it has ended.
    pub fn exit_code(&self) -> Option<u32> {
        *self.exit.lock().unwrap()
    }

    /// How long since the CLI last wrote anything, in ms.
    /// Bytes straight to the CLI's input, as a paste or a typed message goes.
    pub fn write_raw(&self, bytes: Vec<u8>) {
        self.write(bytes);
    }

    /// The CLI turned bracketed paste on (DECSET 2004).
    pub fn bracketed_paste(&self) -> bool {
        self.term.lock().unwrap().mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Type 	ext into the CLI as if it came from the keyboard.
    pub fn type_text(&self, text: &str) {
        self.write(keys::text_records(text));
    }

    /// How long since the CLI last wrote anything, in ms.
    pub fn quiet_for_ms(&self) -> u64 {
        now_ms().saturating_sub(self.last_output_ms())
    }

    /// When the CLI last wrote anything, ms since the epoch.
    pub fn last_output_ms(&self) -> u64 {
        self.last_output.load(Ordering::Relaxed)
    }

    /// The visible screen as plain text, one line per row, trailing blanks
    /// trimmed: what supervise reads a question or a sign-in notice from.
    pub fn screen_text(&self) -> String {
        let term = self.term.lock().unwrap();
        let grid = term.grid();
        let mut out = String::new();
        for row in 0..grid.screen_lines() {
            let line = &grid[Line(row as i32)];
            let mut text = String::with_capacity(grid.columns());
            for col in 0..grid.columns() {
                let cell = &line[Column(col)];
                if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    text.push(cell.c);
                }
            }
            out.push_str(text.trim_end());
            out.push('\n');
        }
        out
    }

    /// How many lines of history to keep; older ones are dropped.
    pub fn keep_lines(&self, lines: usize) {
        self.term.lock().unwrap().set_options(Config { scrolling_history: lines, ..Config::default() });
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// Draw at `size` px instead of the setting's size; None goes back to it.
    pub fn set_font(&mut self, size: Option<f32>, cx: &mut Context<Self>) {
        if self.font != size {
            self.font = size;
            cx.notify();
        }
    }

    /// The size its text draws at now.
    pub(super) fn text_px(&self) -> f32 {
        self.font.unwrap_or_else(font_size)
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

}

/// Pictures that keep their colour; other symbols stay plain text glyphs.
fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1f300..=0x1faff | 0x2705 | 0x2728 | 0x274c | 0x274e | 0x26a1 | 0x2b50)
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.watch_focus(window, cx);
        let this = cx.entity();
        let focus = self.focus.clone();
        let size = self.text_px();
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
                    let cell_w = text.advance(id, px(size), 'm').map(|s| s.width).unwrap_or(px(8.0));
                    let cell_h = px((size * LINE_HEIGHT).round());
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
