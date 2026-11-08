//! The Agent Inbox, Direction D Cinema: black stage, floating panels, colour
//! only for state. Sizes, colours and words follow today's app one to one:
//! src/styles/inbox.css, src/inbox.ts (rowLine, headline, renderQueue,
//! renderHeaders) and src/styles/selectmenu.css for the right-click menu.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use supervise::ask::AskKind;
use supervise::task_state::{TaskState as State, TaskStatus};

use crate::fleet::{Fleet, Seat, now_ms};

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

/// The CLIs New agent offers: program, label, brand colour (clibrand.ts).
const CLIS: [(&str, &str, u32); 4] = [("claude", "Claude", 0xd97757), ("codex", "Codex", 0x10a37f), ("opencode", "OpenCode", 0xf0883e), ("powershell.exe", "PowerShell", 0x5391fe)];

/// Logos, and whether the logo fills its whole tile (clibrand.ts `tile`).
const LOGOS: [(&str, &[u8], bool); 3] = [
    ("claude", include_bytes!("../assets/clis/claude.png"), false),
    ("codex", include_bytes!("../assets/clis/codex.png"), true),
    ("opencode", include_bytes!("../assets/clis/opencode.png"), false),
];

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

fn brand(cli: &str) -> (&'static str, u32) {
    let key = cli.to_ascii_lowercase();
    CLIS.iter().find(|(program, _, _)| *program == key || key.starts_with(program.trim_end_matches(".exe"))).map(|(_, label, hex)| (*label, *hex)).unwrap_or(("Terminal", 0x8c8c96))
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

/// New agent: which CLI, in which folder.
struct Sheet {
    cli: &'static str,
    folder: Entity<InputState>,
    error: Option<String>,
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
    logos: HashMap<&'static str, (Arc<Image>, bool)>,
    sheet: Option<Sheet>,
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
        let logos = LOGOS.iter().map(|(cli, bytes, fills)| (*cli, (Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec())), *fills))).collect();
        Self {
            fleet: Fleet::load(),
            logos,
            sheet: None,
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

    fn mark(&self, cli: &str, size: f32, radius: f32) -> Div {
        let (label, hex) = brand(cli);
        let key = cli.to_ascii_lowercase();
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
        match self.logos.get(key.as_str()) {
            Some((logo, true)) => base.child(img(logo.clone()).size_full().rounded(px(radius))),
            Some((logo, false)) => base.child(img(logo.clone()).size(px(size * 0.68))),
            None => base.child(div().text_size(px(size * 0.42)).font_weight(FontWeight::BOLD).text_color(color(hex)).child(label[..1].to_string())),
        }
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
            .child(self.mark(&seat.spec.cli, 24.0, 7.0).when(ended(state), |mark| mark.opacity(0.55)).child(dot))
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
                                    .child(seat.spec.name.clone()),
                            )
                            .child(mono().flex_none().text_size(px(12.0)).text_color(color(INK_3)).group_hover(group.clone(), |time| time.opacity(0.0)).child(ago(now_ms().saturating_sub(since)))),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(color(hue_of(state).unwrap_or(INK_3)))
                            .when(state == State::Stopped, |line| line.pr(px(72.0)))
                            .child(line),
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
        let (cli_label, _) = brand(&seat.spec.cli);
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
            .child(self.mark(&seat.spec.cli, 36.0, 10.0).mr(px(4.0)))
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
                            .child(div().text_color(color(INK_2)).child(cli_label))
                            .children(seat.branch.clone().map(|b| mono().min_w_0().truncate().text_color(color(INK_3)).child(b))),
                    )
                    .child(div().truncate().text_size(px(22.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(seat.spec.name.clone())),
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

    fn open_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let last = self.fleet.focused_seat().map(|s| s.spec.cwd.clone()).or_else(|| std::env::current_dir().ok().map(|d| d.display().to_string())).unwrap_or_default();
        let folder = cx.new(|cx| InputState::new(window, cx).default_value(last));
        folder.update(cx, |input, cx| input.focus(window, cx));
        crate::keys::set_active(false);
        self.sheet = Some(Sheet { cli: CLIS[0].0, folder, error: None });
        cx.notify();
    }

    fn close_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sheet = None;
        if let Some(index) = self.fleet.focused {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    fn start_from_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sheet) = &self.sheet else { return };
        let cli = sheet.cli;
        let dir = sheet.folder.read(cx).value().trim().trim_matches('"').to_string();
        if !std::path::Path::new(&dir).is_dir() {
            if let Some(sheet) = &mut self.sheet {
                sheet.error = Some("That folder does not exist.".to_string());
            }
            cx.notify();
            return;
        }
        match self.fleet.add(cli, &dir, cx) {
            Ok(index) => {
                self.sheet = None;
                self.fleet.focus(index, window, cx);
            }
            Err(e) => {
                if let Some(sheet) = &mut self.sheet {
                    sheet.error = Some(format!("Could not start {cli}: {e}"));
                }
            }
        }
        cx.notify();
    }

    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose the folder".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                if let Some(path) = paths.first() {
                    let path = path.display().to_string();
                    let _ = this.update_in(cx, |this, window, cx| {
                        if let Some(sheet) = &this.sheet {
                            sheet.folder.update(cx, |input, cx| input.set_value(path, window, cx));
                        }
                    });
                }
            }
        })
        .detach();
    }

    fn sheet_view(&self, sheet: &Sheet, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let chosen = sheet.cli;
        let chips = CLIS.iter().map(|(program, label, _)| {
            let on = *program == chosen;
            div()
                .id(*program)
                .flex()
                .items_center()
                .gap(px(8.0))
                .h(px(36.0))
                .px(px(12.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(if on { white(0.22) } else { hairline() })
                .bg(if on { white(0.08) } else { white(0.0) })
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color(if on { INK } else { INK_2 }))
                .hover(|chip| chip.bg(white(0.05)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(sheet) = &mut this.sheet {
                        sheet.cli = program;
                    }
                    cx.notify();
                }))
                .child(self.mark(program, 20.0, 6.0))
                .child(*label)
                .into_any_element()
        });
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(38.0))
                .px(px(18.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .when(primary, |b| b.bg(color(0xffffff)).text_color(color(0x000000)).hover(|b| b.bg(color(0xe9e9ec))))
                .when(!primary, |b| b.text_color(color(INK_2)).hover(|b| b.bg(white(0.06)).text_color(color(INK))))
                .child(label)
        };
        div()
            .id("sheet-scrim")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .items_center()
            .justify_center()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| match event.keystroke.key.as_str() {
                "escape" => this.close_sheet(window, cx),
                "enter" => this.start_from_sheet(window, cx),
                _ => {}
            }))
            .child(
                div()
                    .id("sheet")
                    .w(px(460.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .p(px(24.0))
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(float())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(div().text_size(px(18.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("New agent"))
                    .child(div().flex().flex_wrap().gap(px(8.0)).children(chips))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(div().text_size(px(12.5)).text_color(color(INK_3)).child("Folder"))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.0))
                                    .items_center()
                                    .child(div().flex_1().child(Input::new(&sheet.folder)))
                                    .child(button("browse", "Browse", false).on_click(cx.listener(|this, _, window, cx| this.browse(window, cx)))),
                            )
                            .when_some(sheet.error.clone(), |field, error| field.child(div().text_size(px(12.5)).text_color(color(DANGER)).child(error))),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(button("cancel", "Cancel", false).on_click(cx.listener(|this, _, window, cx| this.close_sheet(window, cx))))
                            .child(button("start", "Start", true).on_click(cx.listener(|this, _, window, cx| this.start_from_sheet(window, cx)))),
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
