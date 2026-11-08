//! The Agent Inbox, Direction D Cinema: black stage, floating panels, colour
//! only for state. Sizes, colours and words follow today's app one to one:
//! src/styles/inbox.css, src/inbox.ts (rowLine, headline, renderQueue,
//! renderHeaders) and src/styles/selectmenu.css for the right-click menu.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use supervise::ask::{AskKind, parse_ask};
use supervise::task_state::{TaskState as State, TaskStatus};
use supervise::typing::{TypeStep, Typer};

use crate::brand::{self, Mark};
use crate::fleet::{Fleet, Race, Seat, now_ms};
use crate::presets::{self, PRESETS, Prefs, Template};

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

/// A job waits this long after its agent starts before it is typed in
/// (Settings jobDelay in today's app), then for the CLI to go quiet.
const JOB_DELAY_MS: u64 = 4000;
/// Past this, the job is typed anyway: a screen never read as ready is one
/// this cannot read, and typing beats losing the job.
const PROMPT_WAIT_MS: u64 = 2 * 60_000;

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn white(alpha: f32) -> Hsla {
    hsla(0.0, 0.0, 1.0, alpha)
}

fn tint(hex: u32, alpha: f32) -> Hsla {
    color(hex).opacity(alpha)
}

fn hairline() -> Hsla {
    white(0.07)
}

/// `color-mix(in oklch, brand 22%, #111114)`, close enough in sRGB.
fn tile(hex: u32) -> Hsla {
    let mix = |shift: u32| ((hex >> shift & 0xff) as f32 * 0.22 + (0x11 as f32) * 0.78) as u32;
    color((mix(16) << 16) | (mix(8) << 8) | mix(0))
}

/// `inset 0 0 0 1px`: a ring that takes no room, so rows never shift.
fn ring(c: Hsla) -> BoxShadow {
    BoxShadow { color: c, offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: true }
}

/// `--ib-float`: the shadow every floating dock piece stands on.
fn float() -> Vec<BoxShadow> {
    vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(18.0)), blur_radius: px(40.0), spread_radius: px(0.0), inset: false }, ring(white(0.08))]
}

fn icon(name: &'static str, size: f32, tone: Hsla) -> Svg {
    svg().path(format!("icons/{name}.svg")).size(px(size)).flex_none().text_color(tone)
}

fn mono() -> Div {
    div().font_family("Geist Mono")
}

/// The CLI's name as the header says it: its preset's label, else the program.
fn cli_label(cli: &str) -> String {
    let key = cli.to_ascii_lowercase();
    if let Some(p) = PRESETS.iter().find(|p| p.program == key) {
        return p.label.to_string();
    }
    match key.trim_end_matches(".exe") {
        "powershell" | "pwsh" => "PowerShell".to_string(),
        "cmd" => "CMD".to_string(),
        other => other.to_string(),
    }
}

/// GROUP_LABEL in inbox.ts.
fn label_of(state: State) -> &'static str {
    match state {
        State::Needs | State::Blocked => "Needs you",
        State::Review => "Ready to review",
        State::Working => "Working",
        State::Idle => "Idle",
        State::Stopped => "Stopped",
        State::Merged => "Merged",
    }
}

/// The colour a state's words and dot take; None keeps the quiet ink.
fn hue_of(state: State) -> Option<u32> {
    match state {
        State::Needs | State::Blocked => Some(NEEDS),
        State::Review => Some(REVIEW),
        State::Working => Some(WORK),
        _ => None,
    }
}

fn waits(state: State) -> bool {
    matches!(state, State::Needs | State::Blocked)
}

fn ended(state: State) -> bool {
    matches!(state, State::Stopped | State::Merged)
}

/// rowLine in inbox.ts: the second line of a queue row, from the agent's side.
fn row_line(status: &TaskStatus, worked: bool) -> String {
    if let Some(limit) = &status.limit {
        return match limit.resets_at {
            Some(at) => format!("Out of usage · back {}", supervise::task_state::clock_of(at, now_ms())),
            None => "Out of usage".to_string(),
        };
    }
    let said = match status.state {
        State::Needs | State::Blocked => match &status.ask {
            None => match &status.blocked {
                Some(b) => b.what.clone(),
                None => "Needs you".to_string(),
            },
            Some(a) => match (a.kind, &a.detail) {
                (AskKind::Run, Some(d)) => format!("Wants to run {d}"),
                (AskKind::Run, None) => "Wants to run a command".to_string(),
                (AskKind::Edit, Some(d)) => format!("Wants to edit {d}"),
                (AskKind::Edit, None) => "Wants to edit a file".to_string(),
                _ => format!("Asks: {}", a.prompt),
            },
        },
        State::Review => "Changes to review".to_string(),
        State::Working => "Working".to_string(),
        State::Stopped => "Stopped".to_string(),
        State::Merged => "Merged into the base branch".to_string(),
        State::Idle => if worked { "Done" } else { "Waiting for a task" }.to_string(),
    };
    let state = label_of(status.state);
    if said == state || status.state == State::Merged { said } else { format!("{state} · {said}") }
}

/// The top bar's sentence, worded as today's app words it (inbox.ts headline).
fn headline(statuses: &[TaskStatus], worked: &[bool]) -> (String, String) {
    let n = |s: State| statuses.iter().filter(|x| x.state == s).count();
    let are = |k: usize| if k == 1 { "is" } else { "are" };
    if statuses.is_empty() {
        return ("No agents yet.".to_string(), " Start one with New agent.".to_string());
    }
    let needs = n(State::Needs) + n(State::Blocked);
    let lead = match needs {
        0 => "Nothing needs you.".to_string(),
        1 => "1 agent needs you.".to_string(),
        k => format!("{k} agents need you."),
    };
    let (review, working) = (n(State::Review), n(State::Working));
    let mut parts: Vec<String> = Vec::new();
    if review > 0 {
        parts.push(format!("{review} {} ready to review", are(review)));
    }
    if working > 0 {
        parts.push(format!("{working} {} working", are(working)));
    }
    if parts.is_empty() {
        let idle = n(State::Idle);
        let done = statuses.iter().zip(worked).filter(|(s, w)| s.state == State::Idle && **w).count();
        let (stopped, merged) = (n(State::Stopped), n(State::Merged));
        if done > 0 {
            parts.push(format!("{done} {} done", are(done)));
        }
        if idle > done {
            parts.push(format!("{} {} waiting for a task", idle - done, are(idle - done)));
        }
        if stopped > 0 {
            parts.push(format!("{stopped} {} stopped", are(stopped)));
        }
        if merged > 0 {
            parts.push(format!("{merged} {} merged", are(merged)));
        }
    }
    let rest = if parts.is_empty() { String::new() } else { format!(" {}.", parts.join(", ")) };
    (lead, rest)
}

/// ago() in inbox.ts: "12s", "4m", "2h", "3d".
fn ago(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

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
    prefs: Prefs,
    sheet: Option<Sheet>,
    jobs: Vec<Job>,
    /// A loop types the pending jobs, only while there are any.
    typing: bool,
    menu: Option<Menu>,
    /// An agent waiting on "Remove?" to be confirmed.
    removing: Option<usize>,
    /// Projects whose stopped agents are folded away.
    folded: HashSet<String>,
    /// Per agent id: its state at the last tick and since when.
    since: HashMap<String, (State, u64)>,
    /// Agents that have worked on something: idle then reads as done.
    worked: HashSet<String>,
    _ticker: Task<()>,
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
        Self {
            fleet: Fleet::load(),
            prefs: Prefs::load(),
            sheet: None,
            jobs: Vec::new(),
            typing: false,
            menu: None,
            removing: None,
            folded: HashSet::new(),
            since: HashMap::new(),
            worked: HashSet::new(),
            _ticker: ticker,
        }
    }

    /// Reread every agent; redraw when a state moved or a minute rolled on.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let mut changed = self.fleet.absorb();
        let now = now_ms();
        for seat in &self.fleet.seats {
            let state = seat.state(cx);
            if state == State::Working {
                self.worked.insert(seat.spec.id.clone());
            }
            let entry = self.since.entry(seat.spec.id.clone()).or_insert((state, now));
            if entry.0 != state {
                *entry = (state, now);
                changed = true;
            }
        }
        // The times on the rows count in seconds under a minute.
        if changed || self.fleet.seats.iter().any(|s| self.since.get(&s.spec.id).is_some_and(|(_, at)| now - at < 60_000)) {
            cx.notify();
        }
    }

    fn view(&self, cx: &App) -> View {
        let statuses: Vec<TaskStatus> = self.fleet.seats.iter().map(|s| s.status(cx)).collect();
        let worked = self.fleet.seats.iter().map(|s| self.worked.contains(&s.spec.id)).collect();
        let since = self.fleet.seats.iter().map(|s| self.since.get(&s.spec.id).map(|(_, at)| *at).unwrap_or(s.spec.started_ms)).collect();
        View { statuses, worked, since }
    }

    /// cliMark: the CLI's logo on a dark tile of its colour, a prompt for a
    /// shell, a letter for a CLI with no logo.
    fn mark(&self, command: &str, size: f32, radius: f32) -> Div {
        let hex = brand::color_of(command);
        let base = div()
            .relative()
            .flex_none()
            .size(px(size))
            .rounded(px(radius))
            .bg(tile(hex))
            .shadow(vec![ring(tint(hex, 0.38))])
            .flex()
            .items_center()
            .justify_center();
        match brand::mark_of(command) {
            Mark::Logo { image, fills: true, .. } => base.child(img(image).size_full().rounded(px(radius))),
            Mark::Logo { image, .. } => base.child(img(image).size(px(size * 0.68))),
            Mark::Shell => base.child(icon("prompt", size * 0.6, color(hex))),
            Mark::Letter { letter, color: c } => base.child(div().text_size(px(size * 0.42)).font_weight(FontWeight::BOLD).text_color(color(c)).child(letter)),
        }
    }

    /// Type each waiting job once its agent is ready, as deliverTask does:
    /// after the start delay, not while it asks something, once it is quiet.
    fn deliver(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        let mut done: Vec<String> = Vec::new();
        for job in &mut self.jobs {
            let Some(seat) = self.fleet.seats.iter().find(|s| s.spec.id == job.id) else {
                done.push(job.id.clone());
                continue;
            };
            let Some(terminal) = seat.terminal.clone() else {
                done.push(job.id.clone());
                continue;
            };
            let t = terminal.read(cx);
            if t.exit_code().is_some() {
                done.push(job.id.clone());
                continue;
            }
            let screen = t.screen_text();
            if job.typer.is_none() {
                let waited = now.saturating_sub(job.started);
                let ready = waited >= JOB_DELAY_MS && t.quiet_for_ms() >= 1500 && parse_ask(&screen).is_none();
                if !ready && waited < PROMPT_WAIT_MS {
                    continue;
                }
                job.typer = Some(Typer::new(&job.text, t.bracketed_paste(), now));
            }
            if let Some(typer) = &mut job.typer {
                match typer.step(now, &screen) {
                    TypeStep::Write(bytes) => t.write_raw(bytes),
                    TypeStep::Wait => {}
                    TypeStep::Done(_) => done.push(job.id.clone()),
                }
            }
        }
        self.jobs.retain(|j| !done.contains(&j.id));
    }

    fn queue_job(&mut self, id: String, text: String, cx: &mut Context<Self>) {
        self.jobs.push(Job { id, text, started: now_ms(), typer: None });
        if self.typing {
            return;
        }
        self.typing = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let more = this.update(cx, |this, cx| {
                    this.deliver(cx);
                    this.typing = !this.jobs.is_empty();
                    this.typing
                });
                if !matches!(more, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn agent_row(&self, index: usize, seat: &Seat, status: &TaskStatus, worked: bool, since: u64, cx: &mut Context<Self>) -> AnyElement {
        let state = status.state;
        let current = self.fleet.focused == Some(index);
        let ring_ground = if current { RAISE } else { PANEL };
        let dot = div().absolute().right(px(-3.0)).bottom(px(-3.0)).size(px(13.0)).rounded_full().bg(color(ring_ground)).flex().items_center().justify_center().child(match state {
            s if ended(s) => div().size(px(9.0)).rounded_full().shadow(vec![ring(color(INK_3))]).into_any_element(),
            s if waits(s) => icon("diamond", 11.0, color(NEEDS)).into_any_element(),
            s => div().size(px(9.0)).rounded_full().bg(color(hue_of(s).unwrap_or(INK_3))).into_any_element(),
        });
        let group: SharedString = format!("row-{index}").into();
        let line = row_line(status, worked);
        div()
            .id(("seat", index))
            .group(group.clone())
            .relative()
            .flex()
            .gap(px(10.0))
            .p(px(10.0))
            .rounded(px(12.0))
            .text_color(color(INK))
            .when(waits(state), |row| {
                row.bg(tint(NEEDS, if current { 0.2 } else { 0.12 })).shadow(vec![ring(tint(NEEDS, 0.3))]).hover(|row| row.bg(tint(NEEDS, 0.17)))
            })
            .when(current && !waits(state), |row| row.bg(color(RAISE)).shadow(vec![ring(white(0.1))]))
            .when(!current && !waits(state), |row| row.hover(|row| row.bg(white(0.03))))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.fleet.focus(index, window, cx);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.menu = Some(Menu { index, at: event.position });
                    cx.notify();
                }),
            )
            .child(self.mark(&seat.command(), 24.0, 7.0).when(ended(state), |mark| mark.opacity(0.55)).child(dot))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(14.0))
                                    .font_weight(if state == State::Stopped { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                                    .text_color(color(if state == State::Stopped { INK_2 } else { INK }))
                                    // The job is the row's headline when there is one.
                                    .child(seat.spec.title.clone().unwrap_or_else(|| seat.spec.name.clone())),
                            )
                            .child(mono().flex_none().text_size(px(12.0)).text_color(color(INK_3)).group_hover(group.clone(), |time| time.opacity(0.0)).child(ago(now_ms().saturating_sub(since)))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .min_w_0()
                            .text_size(px(12.5))
                            .when(state == State::Stopped, |line| line.pr(px(72.0)))
                            .when(seat.spec.title.is_some(), |line| line.child(div().flex_none().text_color(color(INK_3)).child(format!("{} ·", seat.spec.name))))
                            .when_some(seat.spec.race.clone(), |line, race| {
                                line.child(mono().flex_none().px(px(6.0)).py(px(1.0)).rounded(px(5.0)).bg(white(0.07)).text_size(px(11.0)).text_color(color(INK_2)).child(format!("race {}/{}", race.n, race.of)))
                            })
                            .child(div().min_w_0().truncate().text_color(color(hue_of(state).unwrap_or(INK_3))).child(line)),
                    ),
            )
            .child(
                div()
                    .id(("more", index))
                    .absolute()
                    .top(px(8.0))
                    .right(px(8.0))
                    .size(px(26.0))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(color(0x1a1b20))
                    .shadow(vec![ring(white(0.1))])
                    .opacity(0.0)
                    .group_hover(group, |more| more.opacity(1.0))
                    .hover(|more| more.bg(color(0x24252b)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.menu = Some(Menu { index, at: event.position });
                            cx.notify();
                        }),
                    )
                    .child(icon("dots", 16.0, color(INK_2))),
            )
            .when(state == State::Stopped, |row| {
                row.child(
                    div()
                        .id(("resume-row", index))
                        .absolute()
                        .right(px(8.0))
                        .bottom(px(8.0))
                        .h(px(24.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(white(0.1))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color(INK))
                        .hover(|pill| pill.bg(color(INK)).text_color(color(GROUND)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, window, cx| this.resume(index, window, cx)))
                        .child("Resume"),
                )
            })
            .into_any_element()
    }

    fn project_head(&self, name: &str, count: usize, needs: usize, cx: &mut Context<Self>) -> AnyElement {
        let folded = self.folded.contains(name);
        let key = name.to_string();
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .mt(px(6.0))
            .pt(px(8.0))
            .px(px(4.0))
            .pb(px(4.0))
            .child(
                div()
                    .id(SharedString::from(format!("proj-{name}")))
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(7.0))
                    .min_h(px(32.0))
                    .pl(px(6.0))
                    .pr(px(8.0))
                    .rounded(px(9.0))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(INK_2))
                    .hover(|head| head.bg(white(0.04)).text_color(color(INK)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.folded.remove(&key) {
                            this.folded.insert(key.clone());
                        }
                        cx.notify();
                    }))
                    .child(icon(if folded { "chev" } else { "chev-open" }, 10.0, tint(INK_2, 0.7)))
                    .child(div().flex_1().min_w_0().truncate().child(name.to_string()))
                    .when(needs > 0, |head| {
                        head.child(
                            mono()
                                .flex_none()
                                .min_w(px(18.0))
                                .h(px(18.0))
                                .px(px(5.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(color(NEEDS))
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(color(0x1a1204))
                                .child(needs.to_string()),
                        )
                    })
                    .child(mono().flex_none().font_weight(FontWeight::NORMAL).text_size(px(12.0)).text_color(color(INK_3)).child(count.to_string())),
            )
            .child(
                div()
                    .id(SharedString::from(format!("mem-{name}")))
                    .flex_none()
                    .size(px(30.0))
                    .rounded(px(9.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.7)
                    .hover(|mem| mem.opacity(1.0).bg(white(0.06)))
                    .child(icon("memory", 15.0, color(INK_3))),
            )
            .into_any_element()
    }

    fn queue(&self, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // Projects in the order they first appear; nothing moves when a state
        // changes. Folding hides a project's stopped agents.
        let mut projects: Vec<String> = Vec::new();
        for seat in &self.fleet.seats {
            let name = seat.project();
            if !projects.contains(&name) {
                projects.push(name);
            }
        }
        let mut list = div().id("queue").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().px(px(8.0)).pt(px(6.0)).pb(px(12.0));
        for project in &projects {
            let members: Vec<usize> = (0..self.fleet.seats.len()).filter(|i| &self.fleet.seats[*i].project() == project).collect();
            let needs = members.iter().filter(|i| waits(view.statuses[**i].state)).count();
            let folded = self.folded.contains(project);
            list = list.child(self.project_head(project, members.len(), needs, cx));
            for index in members {
                if folded && ended(view.statuses[index].state) {
                    continue;
                }
                let row = self.agent_row(index, &self.fleet.seats[index], &view.statuses[index], view.worked[index], view.since[index], cx);
                list = list.child(row);
            }
        }
        if projects.is_empty() {
            list = list.child(div().mx(px(10.0)).mt(px(6.0)).mb(px(6.0)).text_size(px(12.5)).text_color(color(INK_3)).child("No agents yet."));
        }
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(QUEUE_W - 32.0))
            .mx(px(16.0))
            .mb(px(DOCK_H))
            .rounded(px(18.0))
            .bg(color(PANEL))
            .shadow(vec![ring(hairline())])
            .overflow_hidden()
            .child(list)
    }

    fn stage_head(&self, seat: &Seat, index: usize, status: &TaskStatus, cx: &mut Context<Self>) -> AnyElement {
        let state = status.state;
        let cli_label = cli_label(&seat.spec.cli);
        let lit = hue_of(state);
        let act = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .h(px(26.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_color(color(INK_2))
                .hover(|act| act.bg(white(0.06)).text_color(color(INK)))
                .child(label)
        };
        let has_model = matches!(seat.spec.cli.to_ascii_lowercase().as_str(), "claude" | "codex");
        let acts = div().flex().items_center().gap(px(2.0)).p(px(3.0)).rounded_full().bg(white(0.05)).text_size(px(12.5)).font_weight(FontWeight::MEDIUM);
        let acts = if state == State::Stopped {
            acts.child(
                div()
                    .id("go")
                    .h(px(26.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(color(INK))
                    .text_color(color(GROUND))
                    .hover(|go| go.bg(color(0xffffff)))
                    .on_click(cx.listener(move |this, _, window, cx| this.resume(index, window, cx)))
                    .child("Resume"),
            )
            .child(act("changes", "Changes"))
            .child(act("history", "History"))
        } else {
            let terminal = seat.terminal.clone();
            let model_terminal = seat.terminal.clone();
            acts.child(act("cmds", "Commands").on_click(cx.listener(move |_, _, window, cx| {
                // Type / so the CLI shows its own commands.
                if let Some(t) = &terminal {
                    t.read(cx).type_text("/");
                    t.read(cx).focus_handle().focus(window, cx);
                }
            })))
            .when(has_model, |acts| {
                acts.child(act("model", "Model").on_click(cx.listener(move |_, _, window, cx| {
                    if let Some(t) = &model_terminal {
                        t.read(cx).type_text("/model\r");
                        t.read(cx).focus_handle().focus(window, cx);
                    }
                })))
            })
            .child(act("changes", "Changes"))
            .child(act("history", "History"))
        };
        let control = |id: &'static str, name: &'static str| {
            div().id(id).size(px(30.0)).rounded_full().flex().items_center().justify_center().opacity(0.0).group_hover("stage", |c| c.opacity(1.0)).hover(|c| c.bg(white(0.06))).child(icon(name, 15.0, color(INK_3)))
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .pt(px(20.0))
            .pr(px(20.0))
            .pb(px(12.0))
            .pl(px(24.0))
            .child(self.mark(&seat.command(), 36.0, 10.0).mr(px(4.0)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .text_size(px(12.5))
                            // With a job, the agent's name moves up beside its CLI.
                            .when(seat.spec.title.is_some(), |meta| meta.child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(seat.spec.name.clone())))
                            .child(div().text_color(color(INK_2)).child(if seat.spec.title.is_some() { format!("· {cli_label}") } else { cli_label }))
                            .children(seat.branch.clone().map(|b| mono().min_w_0().truncate().text_color(color(INK_3)).child(b))),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(22.0))
                            .line_height(relative(1.25))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(INK))
                            .child(seat.spec.title.clone().unwrap_or_else(|| seat.spec.name.clone())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .h(px(28.0))
                    .px(px(12.0))
                    .rounded_full()
                    .bg(lit.map(|h| tint(h, 0.12)).unwrap_or(white(0.06)))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(lit.unwrap_or(INK_2)))
                    .child(if waits(state) { icon("diamond", 11.0, color(NEEDS)).into_any_element() } else { div().size(px(8.0)).rounded_full().bg(color(lit.unwrap_or(INK_2))).into_any_element() })
                    .child(label_of(state)),
            )
            .child(acts)
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(control("stage-more", "dots").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.menu = Some(Menu { index, at: event.position });
                            cx.notify();
                        }),
                    ))
                    .child(control("stage-remove", "x").on_click(cx.listener(move |this, _, _, cx| {
                        this.removing = Some(index);
                        cx.notify();
                    }))),
            )
            .into_any_element()
    }

    fn stage(&self, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let edge = |from: Hsla, to: Hsla| div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(from, 0.0), linear_color_stop(to, 1.0)));
        let lit = tint(WORK, 0.6);
        let clear = tint(WORK, 0.0);
        let panel = div()
            .id("stage")
            .group("stage")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .mr(px(16.0))
            .mb(px(DOCK_H))
            .rounded(px(18.0))
            .bg(color(PANEL))
            .shadow(vec![ring(hairline())])
            .overflow_hidden();
        let Some(index) = self.fleet.focused.filter(|i| *i < self.fleet.seats.len()) else {
            return panel.items_center().justify_center().child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(24.0))
                    .child(div().text_size(px(18.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("No agents yet."))
                    .child(div().text_size(px(14.0)).text_color(color(INK_3)).child("Start one with New agent."))
                    .child(
                        div()
                            .id("empty-new")
                            .mt(px(10.0))
                            .h(px(36.0))
                            .px(px(16.0))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .bg(color(0xffffff))
                            .text_size(px(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(0x000000))
                            .hover(|b| b.bg(color(0xe9e9ec)))
                            .on_click(cx.listener(|this, _, window, cx| this.open_sheet(window, cx)))
                            .child("New agent"),
                    ),
            );
        };
        let seat = &self.fleet.seats[index];
        let status = &view.statuses[index];
        // The edge glows while the CLI runs, whatever it is doing (pane.run).
        let running = seat.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
        let panel = panel
            .when(running, |panel| panel.child(div().absolute().top_0().left_0().right_0().h(px(2.0)).flex().child(edge(clear, lit)).child(edge(lit, clear))))
            .child(self.stage_head(seat, index, status, cx));
        // A stopped agent keeps its last screen; nothing to type into until Resume.
        match &seat.terminal {
            Some(terminal) => panel.child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .pt(px(4.0))
                    .px(px(24.0))
                    .pb(px(12.0))
                    .child(terminal.clone())
                    .child(div().absolute().top_0().left_0().right_0().h(px(8.0)).bg(linear_gradient(180.0, linear_color_stop(color(PANEL), 0.0), linear_color_stop(tint(PANEL, 0.0), 1.0)))),
            ),
            None => panel,
        }
    }

    fn top_bar(&self, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let stopped = view.statuses.iter().filter(|s| s.state == State::Stopped).count();
        let (lead, rest) = headline(&view.statuses, &view.worked);
        let window_button = |name: &'static str, area: WindowControlArea, id: &'static str, close: bool| {
            div()
                .id(id)
                .w(px(46.0))
                .h(px(38.0))
                .flex()
                .items_center()
                .justify_center()
                .window_control_area(area)
                .when(close, |b| b.hover(|b| b.bg(color(0xe81123))))
                .when(!close, |b| b.hover(|b| b.bg(white(0.08))))
                .child(icon(name, 14.0, color(INK_2)))
        };
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(68.0))
            .pl(px(12.0))
            .pr(px(4.0))
            .window_control_area(WindowControlArea::Drag)
            .child(div().id("home").size(px(30.0)).rounded(px(7.0)).flex().items_center().justify_center().hover(|h| h.bg(color(0x0e0f11))).child(icon("house", 15.0, color(INK_3))))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .ml(px(18.0))
                    .truncate()
                    .text_size(px(24.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(INK_3))
                    .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(lead))
                    .child(div().min_w_0().truncate().child(rest)),
            )
            .when(stopped > 0, |bar| {
                bar.child(
                    div()
                        .id("resume-all")
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .h(px(32.0))
                        .px(px(14.0))
                        .mr(px(146.0))
                        .rounded_full()
                        .bg(white(0.06))
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(color(INK))
                        .hover(|pill| pill.bg(white(0.12)))
                        .on_click(cx.listener(|this, _, window, cx| {
                            for index in 0..this.fleet.seats.len() {
                                if this.fleet.seats[index].state(cx) == State::Stopped {
                                    let _ = this.fleet.start(index, cx);
                                }
                            }
                            if let Some(index) = this.fleet.focused {
                                this.fleet.focus(index, window, cx);
                            }
                            cx.notify();
                        }))
                        .child(icon("rotate-ccw", 13.0, color(INK)))
                        .child("Resume all")
                        .child(mono().ml(px(2.0)).px(px(6.0)).rounded_full().bg(white(0.14)).text_size(px(11.0)).child(stopped.to_string())),
                )
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .flex()
                    .child(window_button("minus", WindowControlArea::Min, "win-min", false))
                    .child(window_button("square", WindowControlArea::Max, "win-max", false))
                    .child(window_button("x", WindowControlArea::Close, "win-close", true)),
            )
    }

    fn dock(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let seg = |name: &'static str, label: &'static str, on: bool| {
            div()
                .flex()
                .items_center()
                .gap(px(7.0))
                .h(px(32.0))
                .px(px(14.0))
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color(if on { INK } else { INK_2 }))
                .when(on, |seg| seg.bg(white(0.1)))
                .when(!on, |seg| seg.hover(|seg| seg.bg(white(0.05)).text_color(color(INK))))
                .child(icon(name, 15.0, color(if on { INK } else { INK_2 })))
                .child(label)
        };
        let round = |name: &'static str| {
            div()
                .size(px(40.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(color(RAISE))
                .shadow(float())
                .hover(|round| round.bg(color(0x1d1e23)))
                .child(icon(name, 17.0, color(INK_2)))
        };
        div()
            .absolute()
            .bottom(px(18.0))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .items_center()
            .gap(px(10.0))
            .child(div().flex().gap(px(2.0)).p(px(4.0)).rounded_full().bg(color(RAISE)).shadow(float()).child(seg("rectangle-horizontal", "Focus", true)).child(seg("layout-grid", "Grid", false)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .w(px(400.0))
                    .h(px(40.0))
                    .pl(px(14.0))
                    .pr(px(8.0))
                    .rounded_full()
                    .bg(color(RAISE))
                    .shadow(float())
                    .text_size(px(13.0))
                    .text_color(color(INK_3))
                    .child(icon("search", 14.0, color(INK_3)))
                    .child(div().flex_1().truncate().child("Jump to an agent or run a command"))
                    .child(mono().text_size(px(11.0)).px(px(6.0)).py(px(2.0)).rounded(px(6.0)).shadow(vec![ring(white(0.12))]).child("Ctrl+K")),
            )
            .child(
                div()
                    .id("new-agent")
                    .h(px(40.0))
                    .px(px(18.0))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(color(0xffffff))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(0x000000))
                    .shadow(float())
                    .hover(|button| button.bg(color(0xe9e9ec)))
                    .on_click(cx.listener(|this, _, window, cx| this.open_sheet(window, cx)))
                    .child("New agent"),
            )
            .child(round("history"))
            .child(round("circle-question-mark"))
            .child(round("settings"))
    }

    fn resume(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.fleet.start(index, cx);
        self.fleet.focus(index, window, cx);
        cx.notify();
    }

    /// selectmenu.css: the right-click menu of an agent.
    fn menu_view(&self, menu: &Menu, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let index = menu.index;
        let running = view.statuses.get(index).is_some_and(|s| !ended(s.state));
        let item = |id: &'static str, label: &'static str, danger: bool, disabled: bool| {
            div()
                .id(id)
                .min_h(px(32.0))
                .px(px(12.0))
                .py(px(6.0))
                .flex()
                .items_center()
                .rounded(px(9.0))
                .text_color(color(if disabled { INK_3 } else if danger { DANGER } else { INK }))
                .when(!disabled && !danger, |i| i.hover(|i| i.bg(white(0.08))))
                .when(!disabled && danger, |i| i.hover(|i| i.bg(tint(DANGER, 0.12))))
                .child(label)
        };
        let close = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            this.menu = None;
            cx.notify();
        });
        div().absolute().inset_0().on_mouse_down(MouseButton::Left, close).child(
            deferred(
                anchored().position(menu.at).snap_to_window().child(
                    div()
                        .id("menu")
                        .min_w(px(210.0))
                        .p(px(5.0))
                        .rounded(px(14.0))
                        .bg(color(0x16171b))
                        .shadow(vec![
                            BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.6), offset: point(px(0.0), px(22.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false },
                            BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.4), offset: point(px(0.0), px(2.0)), blur_radius: px(6.0), spread_radius: px(0.0), inset: false },
                            ring(white(0.1)),
                        ])
                        .text_size(px(13.5))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(item("menu-open", "Open", false, false).on_click(cx.listener(move |this, _, window, cx| {
                            this.menu = None;
                            this.fleet.focus(index, window, cx);
                            cx.notify();
                        })))
                        .when(!running, |m| {
                            m.child(item("menu-resume", "Resume", false, false).on_click(cx.listener(move |this, _, window, cx| {
                                this.menu = None;
                                this.resume(index, window, cx);
                            })))
                        })
                        .child(item("menu-stop", "Stop", false, !running).when(running, |i| {
                            i.on_click(cx.listener(move |this, _, _, cx| {
                                this.menu = None;
                                this.fleet.stop(index);
                                cx.notify();
                            }))
                        }))
                        .child(div().h(px(1.0)).mx(px(8.0)).my(px(5.0)).bg(white(0.08)))
                        .child(item("menu-remove", "Remove agent…", true, false).on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.removing = Some(index);
                            cx.notify();
                        }))),
                ),
            )
            .with_priority(1),
        )
    }

    /// confirmModal: "Remove Bob?" before an agent goes.
    fn confirm_view(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let seat = &self.fleet.seats[index];
        let running = seat.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
        let message = if running {
            "It stops and its terminal closes. Its branch and the changes on it stay in git."
        } else {
            "Its terminal closes. Its branch and the changes on it stay in git."
        };
        let button = |id: &'static str, label: &'static str, danger: bool| {
            div()
                .id(id)
                .h(px(36.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .when(danger, |b| b.bg(color(DANGER)).text_color(color(GROUND)).hover(|b| b.opacity(0.9)))
                .when(!danger, |b| b.text_color(color(INK_2)).hover(|b| b.bg(white(0.06)).text_color(color(INK))))
                .child(label)
        };
        div()
            .id("confirm-scrim")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .justify_center()
            .items_start()
            .pt(relative(0.14))
            .child(
                div()
                    .id("confirm")
                    .w(px(440.0))
                    .h_auto()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .p(px(22.0))
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(format!("Remove {}?", seat.spec.name)))
                    .child(div().text_size(px(14.0)).line_height(relative(1.5)).text_color(color(INK_2)).child(message))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(button("confirm-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                                this.removing = None;
                                cx.notify();
                            })))
                            .child(button("confirm-ok", "Remove", true).on_click(cx.listener(move |this, _, window, cx| {
                                this.removing = None;
                                this.fleet.remove(index);
                                if let Some(f) = this.fleet.focused {
                                    this.fleet.focus(f, window, cx);
                                }
                                cx.notify();
                            }))),
                    ),
            )
    }

    /// The folder New agent starts in: the stage's project, else the last one.
    fn project_dir(&self) -> String {
        self.fleet.focused_seat().or(self.fleet.seats.last()).map(|s| s.spec.cwd.clone()).or_else(|| std::env::current_dir().ok().map(|d| d.display().to_string())).unwrap_or_default()
    }

    fn open_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let missing: HashSet<&'static str> = PRESETS.iter().filter(|p| !presets::installed(p.program)).map(|p| p.id).collect();
        // The CLI it starts on is the last one used, if still installed.
        let last = self.prefs.last_cli.clone().unwrap_or_else(|| "claude".to_string());
        let choice = if last.starts_with("custom:") {
            Choice::Custom
        } else {
            PRESETS.iter().find(|p| p.id == last && !missing.contains(p.id)).or_else(|| PRESETS.iter().find(|p| !missing.contains(p.id))).map(|p| Choice::Preset(p.id)).unwrap_or(Choice::Custom)
        };
        let task = cx.new(|cx| TextareaState::new(window, cx).placeholder("Fix the flaky upload test and add a regression test. Leave empty to give it a job later.").auto_grow(4, 10));
        let custom_value = self.prefs.last_custom.clone();
        let custom = cx.new(|cx| InputState::new(window, cx).placeholder("Command, e.g. ollama run llama3.1").default_value(custom_value));
        task.update(cx, |input, cx| input.focus(window, cx));
        crate::keys::set_active(false);
        let skip_perms = self.prefs.skip_perms;
        self.sheet = Some(Sheet { task, choice, custom, count: 1, skip_perms, dir: self.project_dir(), missing, saved: false, error: None });
        cx.notify();
    }

    fn close_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sheet = None;
        if let Some(index) = self.fleet.focused {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    /// What the sheet would start: program, args, and how it is remembered.
    fn sheet_command(&self, sheet: &Sheet, cx: &App) -> Option<(String, Vec<String>, String)> {
        match sheet.choice {
            Choice::Preset(id) => {
                let p = presets::preset(id)?;
                let args = if sheet.skip_perms { p.skip_args.iter().map(|a| a.to_string()).collect() } else { Vec::new() };
                Some((p.program.to_string(), args, id.to_string()))
            }
            Choice::Custom => {
                let typed = sheet.custom.read(cx).value().trim().to_string();
                let mut words = typed.split_whitespace().map(str::to_string);
                let program = words.next()?;
                Some((program, words.collect(), format!("custom:{typed}")))
            }
        }
    }

    fn start_from_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sheet) = &self.sheet else { return };
        let Some((program, args, remembered)) = self.sheet_command(sheet, cx) else {
            if let Some(sheet) = &mut self.sheet {
                sheet.error = Some("Type the command to run, e.g. ollama run llama3.1".to_string());
            }
            cx.notify();
            return;
        };
        let job = sheet.task.read(cx).value().trim().to_string();
        let (count, dir, skip) = (sheet.count, sheet.dir.clone(), sheet.skip_perms);
        if let Choice::Custom = sheet.choice {
            self.prefs.last_custom = remembered.trim_start_matches("custom:").to_string();
        }
        self.prefs.last_cli = Some(remembered);
        self.prefs.skip_perms = skip;
        self.prefs.save();
        let title = presets::task_title(&job);
        let race_id = (count > 1).then(|| format!("race-{}", now_ms()));
        let mut first = None;
        for n in 1..=count {
            let race = race_id.clone().map(|id| Race { id, n, of: count });
            match self.fleet.add(&program, args.clone(), &dir, title.clone(), race, cx) {
                Ok(index) => {
                    first.get_or_insert(index);
                    if !job.is_empty() {
                        let id = self.fleet.seats[index].spec.id.clone();
                        self.queue_job(id, job.clone(), cx);
                    }
                }
                Err(e) => {
                    if let Some(sheet) = &mut self.sheet {
                        sheet.error = Some(format!("Could not start {program}: {e}"));
                    }
                    cx.notify();
                    return;
                }
            }
        }
        self.sheet = None;
        // The agent just started is the one on screen.
        if let Some(index) = first {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    /// Save as preset: keep this setup to start again later.
    fn save_preset(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = &self.sheet else { return };
        let Some((program, _, remembered)) = self.sheet_command(sheet, cx) else { return };
        let label = match sheet.choice {
            Choice::Preset(id) => presets::preset(id).map(|p| p.label.to_string()).unwrap_or(program),
            Choice::Custom => remembered.trim_start_matches("custom:").to_string(),
        };
        let project = crate::fleet::project_of(&sheet.dir);
        let count = sheet.count;
        let name = format!("{}{label} · {project}", if count > 1 { format!("{count}× ") } else { String::new() });
        self.prefs.templates.push(Template { name, cli: remembered, count, dir: sheet.dir.clone(), skip_perms: sheet.skip_perms });
        self.prefs.save();
        if let Some(sheet) = &mut self.sheet {
            sheet.saved = true;
        }
        cx.notify();
    }

    /// The project folder, until projects are opened from Home.
    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose the project folder".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                if let Some(path) = paths.first() {
                    let path = path.display().to_string();
                    let _ = this.update(cx, |this, cx| {
                        if let Some(sheet) = &mut this.sheet {
                            sheet.dir = path;
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// inboxnew.ts and the .inbox-modal rules in inbox.css.
    fn sheet_view(&self, sheet: &Sheet, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // The owner keeps each CLI's logo in its chip (today's chips are text only).
        let pill = |id: SharedString, on: bool, missing: bool| {
            div()
                .id(id)
                .h(px(32.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .gap(px(7.0))
                .rounded_full()
                .text_size(px(13.0))
                .when(on, |p| p.bg(color(0xffffff)).text_color(color(0x000000)).font_weight(FontWeight::SEMIBOLD))
                .when(!on, |p| p.text_color(color(INK_2)).shadow(vec![ring(white(0.1))]))
                .when(missing, |p| p.opacity(0.4))
        };
        let mut chips: Vec<AnyElement> = Vec::new();
        for p in PRESETS {
            let missing = sheet.missing.contains(p.id);
            let on = sheet.choice == Choice::Preset(p.id);
            let id = p.id;
            let chip = pill(SharedString::from(format!("cli-{id}")), on, missing).pl(px(6.0)).child(self.mark(p.program, 20.0, 6.0)).child(p.label);
            chips.push(
                if missing {
                    chip.tooltip(move |window, cx| Tooltip::new(format!("{id} is not installed")).build(window, cx))
                } else {
                    chip.on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(sheet) = &mut this.sheet {
                            sheet.choice = Choice::Preset(id);
                        }
                        cx.notify();
                    }))
                }
                .into_any_element(),
            );
        }
        chips.push(
            pill("cli-custom".into(), sheet.choice == Choice::Custom, false)
                .child("Custom…")
                .on_click(cx.listener(|this, _, window, cx| {
                    if let Some(sheet) = &mut this.sheet {
                        sheet.choice = Choice::Custom;
                        sheet.custom.update(cx, |input, cx| input.focus(window, cx));
                    }
                    cx.notify();
                }))
                .into_any_element(),
        );
        let label = |text: &'static str| div().mb(px(8.0)).text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(color(INK_2)).child(text);
        let button = |id: &'static str, text: String, primary: bool| {
            div()
                .id(id)
                .h(px(36.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .when(primary, |b| b.bg(color(0xffffff)).text_color(color(0x000000)).hover(|b| b.bg(color(0xe9e9ec))))
                .when(!primary, |b| b.bg(white(0.06)).text_color(color(INK_2)).hover(|b| b.bg(white(0.12)).text_color(color(INK))))
                .child(text)
        };
        let seg = (1..=3usize).map(|n| {
            let on = sheet.count == n;
            mono()
                .id(("count", n))
                .w(px(36.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .text_size(px(13.0))
                .text_color(color(if on { INK } else { INK_2 }))
                .when(on, |s| s.bg(white(0.14)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(sheet) = &mut this.sheet {
                        sheet.count = n;
                    }
                    cx.notify();
                }))
                .child(n.to_string())
        });
        let field = |content: Div| content.rounded(px(12.0)).bg(hsla(0.0, 0.0, 0.0, 0.35)).shadow(vec![ring(white(0.09))]);
        let project = crate::fleet::project_of(&sheet.dir);
        let start = if sheet.count > 1 { format!("Start {} agents", sheet.count) } else { "Start agent".to_string() };
        let checked = sheet.skip_perms;
        div()
            .id("sheet-scrim")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .justify_center()
            .items_start()
            .pt(relative(0.14))
            .px(px(16.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_sheet(window, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let k = &event.keystroke;
                if k.key == "escape" {
                    this.close_sheet(window, cx);
                } else if k.key == "enter" && k.modifiers.control {
                    // Ctrl+Enter starts from inside the text box; Enter is a newline.
                    this.start_from_sheet(window, cx);
                }
            }))
            .child(
                div()
                    .id("sheet")
                    .w(px(560.0))
                    .h_auto()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .pt(px(22.0))
                    .px(px(22.0))
                    .pb(px(18.0))
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(4.0))
                                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("New agent"))
                                    .child(
                                        div()
                                            .id("sheet-where")
                                            .flex()
                                            .gap(px(4.0))
                                            .min_w_0()
                                            .text_size(px(12.5))
                                            .text_color(color(INK_3))
                                            .hover(|w| w.text_color(color(INK_2)))
                                            .tooltip(|window, cx| Tooltip::new("Choose another project folder").build(window, cx))
                                            .on_click(cx.listener(|this, _, window, cx| this.browse(window, cx)))
                                            .child(format!("In {project} ·"))
                                            .child(mono().min_w_0().truncate().child(sheet.dir.clone())),
                                    ),
                            )
                            .child(
                                div()
                                    .id("sheet-x")
                                    .size(px(30.0))
                                    .flex_none()
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(white(0.05))
                                    .hover(|x| x.bg(white(0.1)))
                                    .on_click(cx.listener(|this, _, window, cx| this.close_sheet(window, cx)))
                                    .child(icon("x", 14.0, color(INK_3))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(label("What should it do?"))
                            .child(field(div().px(px(4.0)).py(px(4.0)).text_size(px(14.0)).line_height(relative(1.5)).min_h(px(96.0))).child(Textarea::new(&sheet.task).appearance(false))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(label("Agent"))
                            .child(div().flex().flex_wrap().gap(px(6.0)).children(chips))
                            .when(sheet.choice == Choice::Custom, |f| f.child(field(div().mt(px(8.0)).h(px(36.0)).px(px(12.0)).flex().items_center().rounded(px(10.0)).font_family("Geist Mono").text_size(px(13.5))).child(Input::new(&sheet.custom).appearance(false)))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(16.0))
                            .child(div().flex().flex_col().child(label("How many")).child(div().flex().gap(px(2.0)).p(px(3.0)).rounded_full().bg(white(0.05)).children(seg)))
                            .when(sheet.count > 1, |row| row.child(div().mb(px(6.0)).text_size(px(12.5)).text_color(color(INK_3)).child("They all get the same job. Compare their changes in Review and keep the best."))),
                    )
                    .child(
                        div()
                            .id("skip")
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .text_size(px(13.0))
                            .text_color(color(INK_2))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(sheet) = &mut this.sheet {
                                    sheet.skip_perms = !sheet.skip_perms;
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .size(px(16.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(checked, |b| b.bg(color(0xffffff)).child(icon("check", 12.0, color(0x000000))))
                                    .when(!checked, |b| b.shadow(vec![ring(white(0.3))])),
                            )
                            .child("Don't ask before running commands or editing files"),
                    )
                    .when_some(sheet.error.clone(), |s, error| s.child(div().text_size(px(12.5)).text_color(color(DANGER)).child(error)))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .mt(px(4.0))
                            .child(
                                div()
                                    .id("save-preset")
                                    .h(px(36.0))
                                    .px(px(16.0))
                                    .flex()
                                    .items_center()
                                    .rounded_full()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(color(INK_3))
                                    .when(!sheet.saved, |b| b.hover(|b| b.bg(white(0.06)).text_color(color(INK))).on_click(cx.listener(|this, _, _, cx| this.save_preset(cx))))
                                    .child(if sheet.saved { "Saved" } else { "Save as preset" }),
                            )
                            .child(div().flex_1())
                            .child(button("cancel", "Cancel".to_string(), false).on_click(cx.listener(|this, _, window, cx| this.close_sheet(window, cx))))
                            .child(button("start", start, true).tooltip(|window, cx| Tooltip::new("Start (Ctrl+Enter)").build(window, cx)).on_click(cx.listener(|this, _, window, cx| this.start_from_sheet(window, cx)))),
                    ),
            )
    }
}

impl Render for Inbox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.view(cx);
        let queue = self.queue(&view, cx);
        let stage = self.stage(&view, cx);
        let top = self.top_bar(&view, cx);
        let dock = self.dock(cx);
        let sheet = self.sheet.as_ref().map(|sheet| self.sheet_view(sheet, cx));
        let menu = self.menu.as_ref().filter(|m| m.index < self.fleet.seats.len()).map(|m| self.menu_view(m, &view, cx));
        let confirm = self.removing.filter(|i| *i < self.fleet.seats.len()).map(|i| self.confirm_view(i, cx));
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(color(GROUND))
            .font_family("Geist")
            .line_height(relative(1.2))
            .child(top)
            .child(div().flex().flex_1().min_h_0().child(queue).child(stage))
            .child(dock)
            .children(sheet)
            .children(menu)
            .children(confirm)
    }
}
