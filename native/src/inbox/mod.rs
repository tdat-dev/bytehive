//! The Agent Inbox, Direction D Cinema: black stage, floating panels, colour
//! only for state. Sizes, colours and words follow today's app one to one:
//! src/styles/inbox.css, src/inbox.ts (rowLine, headline, renderQueue,
//! renderHeaders) and src/styles/selectmenu.css for the right-click menu.


mod style;
mod words;
mod queue;
mod background;
mod stage;
mod chrome;
mod menu;
mod sheet;
mod palette;
mod launch;
mod home;
mod compose;
mod dropdown;
mod logo;
mod settings;
mod sessions;
mod toast;
mod changes;
mod history;
mod files;
mod bridge;
mod memory;
mod limits;
mod diffs;
mod dialog;
mod usage_view;
mod schedules;
mod board;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use supervise::ask::{AskKind, parse_ask};
use supervise::task_state::{TaskState as State, TaskStatus};
use supervise::typing::{TypeStep, Typer};

use crate::brand::{self, Mark};
use crate::fleet::{Fleet, Race, Seat, now_ms};
use crate::presets::{self, PRESETS, Prefs, Template};
use crate::{plan, usage};

// Named, not globbed: white() would clash with gpui_kit's own.
use style::{color, float, hairline, haze, icon, mono, ring, tile, tint, tip, white};
use words::*;
use chrome::window_buttons;
use compose::Compose;
use dropdown::Popup;
use launch::Launch;
use settings::Settings;
use sessions::Sessions;
use toast::Toast;
use changes::{Changes, Kept};
use history::Histories;
use files::Files;
use memory::MemoryView;

const GROUND: u32 = 0x050506;
const PANEL: u32 = 0x0e0f11;
const RAISE: u32 = 0x15161a;
const INK: u32 = 0xf4f4f5;
const INK_2: u32 = 0xb4b4bc;
const INK_3: u32 = 0x8c8c96;
const WORK: u32 = 0x6fd6a0;
/// --ib-needs: amber, for whoever waits on you.
const NEEDS: u32 = 0xf0bf6a;
/// --ib-review: blue, for work ready to look at.
const REVIEW: u32 = 0x9ab8ee;
const DANGER: u32 = 0xf2766b;
const QUEUE_W: f32 = 320.0;
const DOCK_H: f32 = 84.0;
/// How often the queue rereads who is working.
const TICK: Duration = Duration::from_millis(700);

/// Past this, the job is typed anyway: a screen never read as ready is one
/// this cannot read, and typing beats losing the job.
const PROMPT_WAIT_MS: u64 = 2 * 60_000;

/// The CLI New agent starts: a preset, or any command typed under Custom….
#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Preset(&'static str),
    Custom,
}

/// New agent, as today's inboxnew.ts builds it: what to do, which CLI, how
/// many, whether it may run commands without asking.
struct Sheet {
    task: Entity<TextareaState>,
    choice: Choice,
    custom: Entity<InputState>,
    count: usize,
    skip_perms: bool,
    /// The folder it starts in: the current project's.
    dir: String,
    /// CLIs not on PATH, greyed out.
    missing: HashSet<&'static str>,
    saved: bool,
    error: Option<String>,
}

/// A job waiting to be typed into a new agent once its CLI is ready.
struct Job {
    id: String,
    text: String,
    started: u64,
    typer: Option<Typer>,
}

/// The right-click menu of one agent, where it was opened.
struct Menu {
    index: usize,
    at: Point<Pixels>,
}

/// One snapshot of every agent, taken once per render.
struct View {
    statuses: Vec<TaskStatus>,
    worked: Vec<bool>,
    since: Vec<u64>,
}

pub struct Inbox {
    fleet: Fleet,
    /// Home, the start screen, instead of the agents.
    home: bool,
    /// Home's composer, made the first time Home shows.
    compose: Option<Compose>,
    /// A Home chip's drop-down, while open.
    popup: Option<Popup>,
    settings: Option<Settings>,
    /// Earlier conversations, read back, while open.
    sessions: Option<Sessions>,
    toast: Option<Toast>,
    changes: Option<Changes>,
    /// Changes' note, viewed files and line comments, per agent id.
    kept: HashMap<String, Kept>,
    history: Histories,
    files: Option<Files>,
    /// The bytehive-mcp file bridge, started on the first render.
    bridge: Option<Task<()>>,
    memory: Option<MemoryView>,
    usage_view: Option<usage_view::UsageView>,
    schedules: Option<schedules::SchedulesView>,
    /// Scheduled agents' 30-second check, started on the first render.
    scheduler: Option<Task<()>>,
    /// The Board (Ctrl+Shift+K), while open.
    board: Option<board::BoardView>,
    limits: limits::LimitGuard,
    tray: Option<crate::tray::Tray>,
    prefs: Prefs,
    sheet: Option<Sheet>,
    jobs: Vec<Job>,
    /// A loop types the pending jobs, only while there are any.
    typing: bool,
    menu: Option<Menu>,
    /// Ctrl K, while open.
    palette: Option<Palette>,
    /// An agent waiting on "Remove?" to be confirmed.
    removing: Option<usize>,
    /// Projects whose stopped agents are folded away.
    folded: HashSet<String>,
    /// Per agent id: its state at the last tick and since when.
    since: HashMap<String, (State, u64)>,
    /// Agents that have worked on something: idle then reads as done.
    worked: HashSet<String>,
    /// Claude's and Codex's plan limits.
    plans: (plan::Source, plan::Source),
    /// Per agent id: what it has used, as last read.
    uses: HashMap<String, Use>,
    _ticker: Task<()>,
    _follow: Task<()>,
    _plans: Task<()>,
}

/// What an agent has used, and when that was last asked.
#[derive(Default)]
struct Use {
    usage: Option<usage::Usage>,
    at: u64,
    busy: bool,
}

impl Inbox {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        });
        // A few seconds after start, never in the way of CLIs starting, then
        // every few minutes.
        let plans = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(plan::FIRST_MS)).await;
            loop {
                if this.update(cx, |this, cx| this.ask_plans(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(plan::EVERY_MS)).await;
            }
        });
        // Every few seconds while a Claude agent runs: the session it is in now.
        let follow = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(crate::follow::CHECK_MS)).await;
                let Ok(any) = this.update(cx, |this, cx| !this.fleet.claude_runs(cx).is_empty()) else { break };
                if !any {
                    continue;
                }
                let live = cx.background_executor().spawn(async { agent_core::claude::live_sessions() }).await;
                if this.update(cx, |this, cx| this.fleet.follow(&live, cx)).is_err() {
                    break;
                }
            }
        });
        let mut prefs = Prefs::load();
        // Once: the fleet and settings of the app before this one.
        let imported = crate::import::take(&mut prefs);
        crate::terminal::set_font_size(prefs.font_size);
        // Settings → Reopen my projects on start.
        let mut fleet = if prefs.restore { Fleet::load() } else { Fleet::default() };
        let came = imported.map(|found| {
            let n = fleet.adopt(found.specs.clone());
            crate::import::finish(&found, n);
            n
        });
        let mut inbox = Self {
            home: fleet.seats.is_empty(),
            compose: None,
            popup: None,
            settings: None,
            sessions: None,
            toast: None,
            changes: None,
            kept: HashMap::new(),
            history: Histories::default(),
            files: None,
            bridge: None,
            memory: None,
            usage_view: None,
            schedules: None,
            scheduler: None,
            board: None,
            limits: limits::LimitGuard::default(),
            tray: crate::tray::Tray::new(),
            plans: Default::default(),
            uses: HashMap::new(),
            _plans: plans,
            fleet,
            prefs,
            sheet: None,
            jobs: Vec::new(),
            typing: false,
            menu: None,
            palette: None,
            removing: None,
            folded: HashSet::new(),
            since: HashMap::new(),
            worked: HashSet::new(),
            _ticker: ticker,
            _follow: follow,
        };
        inbox.show_tray();
        if let Some(n) = came {
            let agents = if n == 1 { "1 agent".to_string() } else { format!("{n} agents") };
            inbox.toast_for(format!("Brought over {agents} and your settings from ByteHive. Resume an agent to carry on its conversation."), 15_000, cx);
        }
        inbox
    }

    /// Reread every agent; redraw when a state moved or a minute rolled on.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if crate::tray::quit_asked() {
            cx.quit();
            return;
        }
        if self.prefs.hide_to_tray {
            crate::tray::tuck_away_if_minimised();
        }
        let running = self.fleet.seats.iter().filter(|s| s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none())).count();
        if let Some(tray) = &mut self.tray {
            tray.set_running(running);
        }
        self.refresh_usage(cx);
        let mut changed = self.fleet.absorb();
        let now = now_ms();
        let mut hits = Vec::new();
        for (i, seat) in self.fleet.seats.iter().enumerate() {
            let status = seat.status(cx);
            if let Some(hit) = &status.limit {
                hits.push((i, hit.clone()));
            }
            self.history.observe(&seat.spec.id, &status, seat.changes.count, now);
            let state = status.state;
            if state == State::Working {
                self.worked.insert(seat.spec.id.clone());
            }
            let entry = self.since.entry(seat.spec.id.clone()).or_insert((state, now));
            if entry.0 != state {
                *entry = (state, now);
                changed = true;
            }
        }
        self.check_limits(hits, cx);
        self.poll_changes(cx);
        // The times on the rows count in seconds under a minute.
        if changed || self.fleet.seats.iter().any(|s| self.since.get(&s.spec.id).is_some_and(|(_, at)| now - at < 60_000)) {
            cx.notify();
        }
    }

    /// The tray icon shows while Hide to tray is on.
    fn show_tray(&self) {
        if let Some(tray) = &self.tray {
            tray.set_visible(self.prefs.hide_to_tray);
        }
    }

    fn view(&self, cx: &App) -> View {
        let statuses: Vec<TaskStatus> = self.fleet.seats.iter().map(|s| s.status(cx)).collect();
        let worked = self.fleet.seats.iter().map(|s| self.worked.contains(&s.spec.id)).collect();
        let since = self.fleet.seats.iter().map(|s| self.since.get(&s.spec.id).map(|(_, at)| *at).unwrap_or(s.spec.started_ms)).collect();
        View { statuses, worked, since }
    }

}

impl Render for Inbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start_bridge(window, cx);
        if self.home {
            self.ensure_compose(window, cx);
        }
        let history = self.history_view(cx);
        let board = self.board_view(cx);
        let view = self.view(cx);
        let body = if self.home { self.home_view(&view, window, cx).into_any_element() } else { self.agents_view(&view, cx).into_any_element() };
        let sheet = self.sheet.as_ref().map(|sheet| self.sheet_view(sheet, cx));
        let menu = self.menu.as_ref().filter(|m| m.index < self.fleet.seats.len()).map(|m| self.menu_view(m, &view, cx));
        let confirm = self.removing.filter(|i| *i < self.fleet.seats.len()).map(|i| self.confirm_view(i, cx));
        let palette = self.palette.as_ref().map(|p| self.palette_view(p, &view, cx));
        let settings = self.settings.as_ref().map(|s| self.settings_view(s, cx));
        let sessions = self.sessions.as_ref().map(|s| self.sessions_view(s, window, cx));
        let popup = self.popup.as_ref().map(|p| self.popup_view(p, cx));
        let memory = self.memory.as_ref().map(|m| self.memory_view(m, cx));
        let dialogs = self.dialogs(window, cx);
        let toast = self.toast_view();
        let changes = self.changes.as_ref().and_then(|c| self.changes_view(c, cx));
        div()
            .relative()
            .size_full()
            .bg(color(GROUND))
            // Today's app takes these before the terminal sees them (keys.rs
            // lets them through): Ctrl+K the palette, Ctrl+Shift+T New agent.
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                let m = k.modifiers;
                if m.control && !m.alt && !m.shift && k.key == "k" {
                    if this.palette.is_some() { this.close_palette(window, cx) } else { this.open_palette(window, cx) }
                } else if m.control && m.shift && !m.alt && k.key == "t" {
                    this.palette = None;
                    this.open_sheet(window, cx);
                } else if m.control && m.shift && !m.alt && k.key == "e" {
                    this.toggle_files(window, cx);
                } else if m.control && m.shift && !m.alt && k.key == "k" {
                    if this.board.is_some() { this.close_board(window, cx) } else { this.open_board(window, cx) }
                } else if m.control && !m.alt && !m.shift && k.key == "," {
                    this.open_settings(window, cx);
                } else if m.alt && !m.control && !m.shift && k.key == "o" {
                    this.open_sessions(window, cx);
                } else if k.key == "escape" && this.dialog_open() {
                    this.dialog_escape(window, cx);
                } else if k.key == "escape" && this.settings.is_some() {
                    if this.popup.take().is_none() {
                        this.close_settings(window, cx);
                    }
                    cx.notify();
                } else if k.key == "escape" && this.sessions.is_some() {
                    this.close_sessions(window, cx);
                } else if k.key == "escape" && this.board.is_some() {
                    this.board_escape(window, cx);
                } else if k.key == "escape" && this.changes.as_ref().is_some_and(|c| c.discarding) {
                    if let Some(c) = &mut this.changes {
                        c.discarding = false;
                    }
                    cx.notify();
                } else if k.key == "escape" && this.changes.as_ref().is_some_and(|c| c.editing.is_some()) {
                    // Esc puts the comment away, not the whole drawer.
                    this.cancel_comment(window, cx);
                } else if k.key == "escape" && this.changes.is_some() {
                    this.close_changes(window, cx);
                } else if k.key == "escape" && this.memory.is_some() {
                    this.memory_escape(window, cx);
                } else if k.key == "escape" && this.history.open.is_some() {
                    this.close_history(window, cx);
                } else if m.alt && !m.control && !m.shift && k.key == "h" {
                    if let Some(index) = this.fleet.focused {
                        this.open_history(index, window, cx);
                    }
                } else if m.alt && !m.control && !m.shift && k.key == "r" {
                    if let Some(index) = this.fleet.focused {
                        this.open_changes(index, window, cx);
                    }
                }
            }))
            .font_family("Geist")
            .line_height(relative(1.2))
            .child(body)
            .children(changes)
            .children(history)
            .children(board)
            .children(sheet)
            .children(settings)
            .children(sessions)
            .children(memory)
            .children(dialogs)
            .children(popup)
            .children(menu)
            .children(confirm)
            .children(palette)
            .children(toast)
    }
}

impl Inbox {
    /// The agents: queue, stage, top bar and dock.
    fn agents_view(&self, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            // The faint amber haze over the top of the stage: inbox.css's
            // radial-gradient(1100px 460px at 62% -12%), baked into haze.png
            // since GPUI draws no radial gradients.
            .child(div().absolute().left(relative(0.62)).ml(px(-1100.0)).top(relative(-0.12)).mt(px(-460.0)).w(px(2200.0)).h(px(920.0)).child(img(haze()).size_full().object_fit(ObjectFit::Fill)))
            .child(self.top_bar(view, cx))
            .child(div().flex().flex_1().min_h_0().child(self.queue(view, cx)).child(self.stage(view, cx)).children(self.files_view(cx)))
            .child(self.dock(cx))
    }
}

/// What a palette line does.
#[derive(Clone, Copy)]
enum PalRun {
    Agent(usize),
    NewAgent,
    ResumeAll,
    Sessions,
    Settings,
    Files,
    Memory,
    Schedules,
    Usage,
    Board,
    Changes,
    History,
    Merge,
    /// The project of the agent at this index.
    Project(usize),
}

/// One line of Ctrl K (inboxpalette.ts PaletteItem).
struct PalItem {
    group: String,
    label: String,
    sub: Option<String>,
    keys: Option<&'static str>,
    agent: Option<String>,
    run: PalRun,
}

/// Ctrl K: the box, and which line is lit.
pub struct Palette {
    query: Entity<InputState>,
    at: usize,
    _sub: Subscription,
}
