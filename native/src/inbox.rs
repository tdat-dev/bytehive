//! The Agent Inbox, Direction D Cinema: black stage, floating panels, colour
//! only for state. Sizes and colours follow src/styles/inbox.css one to one.
//! The queue is the live fleet; the stage shows the focused agent's terminal.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::fleet::{Fleet, Seat, State, now_ms};

const GROUND: u32 = 0x050506;
const PANEL: u32 = 0x0e0f11;
const RAISE: u32 = 0x15161a;
const INK: u32 = 0xf4f4f5;
const INK_2: u32 = 0xb4b4bc;
const INK_3: u32 = 0x8c8c96;
const WORK: u32 = 0x6fd6a0;
const QUEUE_W: f32 = 320.0;
const DOCK_H: f32 = 84.0;
/// How often the queue rereads who is working.
const TICK: Duration = Duration::from_millis(700);

/// The CLIs New agent offers: program, label, brand colour.
const CLIS: [(&str, &str, u32); 4] = [("claude", "Claude", 0xd97757), ("codex", "Codex", 0xe5e5e5), ("opencode", "OpenCode", 0xcfcfcf), ("powershell.exe", "PowerShell", 0x5391fe)];

const LOGOS: [(&str, &[u8]); 3] = [
    ("claude", include_bytes!("../assets/clis/claude.png")),
    ("codex", include_bytes!("../assets/clis/codex.png")),
    ("opencode", include_bytes!("../assets/clis/opencode.png")),
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

/// `--ib-float`: the shadow every floating dock piece stands on.
fn float() -> Vec<BoxShadow> {
    vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(18.0)), blur_radius: px(40.0), spread_radius: px(0.0), inset: false }]
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

fn state_look(state: State) -> (&'static str, u32) {
    match state {
        State::Working => ("Working", WORK),
        State::Idle => ("Idle", INK_3),
        State::Stopped => ("Stopped", INK_3),
    }
}

/// "12s", "4m", "2h", "3d" since `ms`.
fn age(ms: u64) -> String {
    let s = now_ms().saturating_sub(ms) / 1000;
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

pub struct Inbox {
    fleet: Fleet,
    logos: HashMap<&'static str, Arc<Image>>,
    sheet: Option<Sheet>,
    /// Each agent's state at the last tick, to redraw only on a change.
    seen: Vec<State>,
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
        let logos = LOGOS.iter().map(|(cli, bytes)| (*cli, Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec())))).collect();
        Self { fleet: Fleet::load(), logos, sheet: None, seen: Vec::new(), _ticker: ticker }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let learned = self.fleet.absorb();
        let now: Vec<State> = self.fleet.seats.iter().map(|s| s.state(cx)).collect();
        if learned || now != self.seen {
            self.seen = now;
            cx.notify();
        }
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
            .border_1()
            .border_color(tint(hex, 0.38))
            .flex()
            .items_center()
            .justify_center();
        match self.logos.get(key.as_str()) {
            Some(logo) => base.child(img(logo.clone()).size(px(size * 0.68))),
            None => base.child(div().text_size(px(size * 0.42)).font_weight(FontWeight::BOLD).text_color(color(hex)).child(label[..1].to_string())),
        }
    }

    fn agent_row(&self, index: usize, seat: &Seat, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let state = seat.state(cx);
        let (label, hue) = state_look(state);
        let current = self.fleet.focused == Some(index);
        let ring = if current { RAISE } else { PANEL };
        let dot = div().absolute().right(px(-3.0)).bottom(px(-3.0)).size(px(13.0)).rounded_full().bg(color(ring)).flex().items_center().justify_center().child(
            match state {
                State::Stopped => div().size(px(9.0)).rounded_full().border_1().border_color(color(INK_3)),
                _ => div().size(px(9.0)).rounded_full().bg(color(hue)),
            },
        );
        let (cli_label, _) = brand(&seat.spec.cli);
        div()
            .id(("seat", index))
            .flex()
            .gap(px(10.0))
            .p(px(10.0))
            .rounded(px(12.0))
            .text_color(color(if state == State::Stopped { INK_2 } else { INK }))
            .when(current, |row| row.bg(color(RAISE)).border_1().border_color(white(0.1)))
            .when(!current, |row| row.hover(|row| row.bg(white(0.03))))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.fleet.focus(index, window, cx);
                cx.notify();
            }))
            .child(self.mark(&seat.spec.cli, 24.0, 7.0).when(state == State::Stopped, |mark| mark.opacity(0.55)).child(dot))
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
                            .child(div().font_weight(if state == State::Stopped { FontWeight::NORMAL } else { FontWeight::MEDIUM }).text_size(px(14.0)).truncate().child(seat.spec.name.clone()))
                            .child(mono().text_size(px(12.0)).text_color(color(INK_3)).child(age(seat.spec.started_ms))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_size(px(12.5))
                            .text_color(color(INK_3))
                            .child(div().text_color(color(hue)).child(label))
                            .child(format!(" · {cli_label}")),
                    ),
            )
    }

    fn project_head(name: &str, count: usize) -> impl IntoElement + use<> {
        div().flex().items_center().gap(px(2.0)).mt(px(6.0)).pt(px(8.0)).px(px(4.0)).pb(px(4.0)).child(
            div()
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
                .child(icon("chevron-down", 10.0, tint(INK_2, 0.7)))
                .child(div().flex_1().min_w_0().truncate().child(name.to_string()))
                .child(mono().font_weight(FontWeight::NORMAL).text_size(px(12.0)).text_color(color(INK_3)).child(count.to_string())),
        )
    }

    fn queue(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // Projects in the order they first appear, each over its agents.
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
            list = list.child(Self::project_head(project, members.len()));
            for index in members {
                let row = self.agent_row(index, &self.fleet.seats[index], cx);
                list = list.child(row);
            }
        }
        if projects.is_empty() {
            list = list.child(div().mx(px(10.0)).mt(px(14.0)).text_size(px(12.5)).text_color(color(INK_3)).child("No agents yet. New agent starts one."));
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
            .border_1()
            .border_color(hairline())
            .overflow_hidden()
            .child(list)
    }

    fn stage_head(&self, seat: &Seat, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let state = seat.state(cx);
        let (label, hue) = state_look(state);
        let (cli_label, _) = brand(&seat.spec.cli);
        let tab = |label: &'static str| div().h(px(28.0)).px(px(13.0)).flex().items_center().rounded_full().hover(|tab| tab.bg(white(0.06)).text_color(color(INK))).child(label);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .pt(px(20.0))
            .pr(px(20.0))
            .pb(px(12.0))
            .pl(px(20.0))
            .child(self.mark(&seat.spec.cli, 42.0, 12.0).when(state == State::Working, |mark| mark.border_color(tint(WORK, 0.6))))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(div().flex().gap(px(8.0)).text_size(px(12.5)).child(div().text_color(color(INK_2)).child(cli_label)).child(mono().text_color(color(INK_3)).truncate().child(seat.spec.cwd.clone())))
                    .child(div().text_size(px(22.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(seat.spec.name.clone())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .h(px(28.0))
                    .px(px(12.0))
                    .rounded_full()
                    .bg(if state == State::Working { tint(WORK, 0.12) } else { white(0.06) })
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(if state == State::Working { WORK } else { INK_2 }))
                    .child(div().size(px(8.0)).rounded_full().bg(color(hue)))
                    .child(label),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .p(px(1.0))
                    .rounded_full()
                    .bg(white(0.05))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(INK_2))
                    .child(tab("Commands"))
                    .child(tab("Model"))
                    .child(tab("Changes"))
                    .child(tab("History")),
            )
            .when(state != State::Stopped, |head| {
                head.child(
                    div()
                        .id("stop")
                        .size(px(30.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|control| control.bg(white(0.06)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.fleet.stop(index);
                            cx.notify();
                        }))
                        .child(icon("square", 14.0, color(INK_3))),
                )
            })
    }

    fn stopped_panel(&self, seat: &Seat, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ended = seat.terminal.as_ref().and_then(|t| t.read(cx).exit_code());
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.0))
            .child(div().text_size(px(18.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(format!("{} is stopped.", seat.spec.name)))
            .child(div().text_size(px(13.0)).text_color(color(INK_3)).child(match (ended, &seat.spec.session_id) {
                (Some(code), _) => format!("It ended with code {code}. Resume carries on the same conversation."),
                (None, Some(_)) => "Resume carries on the same conversation.".to_string(),
                (None, None) => "Resume starts it again in the same folder.".to_string(),
            }))
            .child(
                div()
                    .id("resume")
                    .h(px(40.0))
                    .px(px(20.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .rounded_full()
                    .bg(color(0xffffff))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(0x000000))
                    .hover(|button| button.bg(color(0xe9e9ec)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let _ = this.fleet.start(index, cx);
                        this.fleet.focus(index, window, cx);
                        cx.notify();
                    }))
                    .child(icon("rotate-ccw", 14.0, color(0x000000)))
                    .child("Resume"),
            )
    }

    fn stage(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let edge = |from: Hsla, to: Hsla| div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(from, 0.0), linear_color_stop(to, 1.0)));
        let lit = tint(WORK, 0.45);
        let clear = tint(WORK, 0.0);
        let panel = div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .mr(px(16.0))
            .mb(px(DOCK_H))
            .rounded(px(18.0))
            .bg(color(PANEL))
            .border_1()
            .border_color(hairline())
            .overflow_hidden();
        let Some(index) = self.fleet.focused.filter(|i| *i < self.fleet.seats.len()) else {
            return panel
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .child(div().text_size(px(18.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Nothing on stage."))
                .child(div().text_size(px(13.0)).text_color(color(INK_3)).child("Start an agent with New agent, below."));
        };
        let seat = &self.fleet.seats[index];
        let state = seat.state(cx);
        let panel = panel
            .when(state == State::Working, |panel| panel.child(div().absolute().top_0().left_0().right_0().h(px(2.0)).flex().child(edge(clear, lit)).child(edge(lit, clear))))
            .child(self.stage_head(seat, index, cx));
        match (&seat.terminal, state) {
            (Some(terminal), State::Working | State::Idle) => panel.child(div().flex_1().min_h_0().pt(px(4.0)).px(px(24.0)).pb(px(12.0)).child(terminal.clone())),
            _ => panel.child(self.stopped_panel(seat, index, cx)),
        }
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let states: Vec<State> = self.fleet.seats.iter().map(|s| s.state(cx)).collect();
        let working = states.iter().filter(|s| **s == State::Working).count();
        let stopped = states.iter().filter(|s| **s == State::Stopped).count();
        let rest = match working {
            0 => String::new(),
            1 => "1 is working.".to_string(),
            n => format!("{n} are working."),
        };
        let window_button = |name: &'static str, area: WindowControlArea, id: &'static str| {
            div().id(id).w(px(46.0)).h(px(38.0)).flex().items_center().justify_center().window_control_area(area).hover(|button| button.bg(white(0.08))).child(icon(name, 14.0, color(INK_2)))
        };
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(68.0))
            .pl(px(20.0))
            .pr(px(4.0))
            .window_control_area(WindowControlArea::Drag)
            .child(icon("house", 15.0, color(INK_2)))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(7.0))
                    .ml(px(20.0))
                    .text_size(px(24.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(INK_3))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Nothing needs you."))
                    .child(rest),
            )
            .when(stopped > 0, |bar| {
                bar.child(
                    div()
                        .id("resume-all")
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .h(px(32.0))
                        .px(px(16.0))
                        .mr(px(146.0))
                        .rounded_full()
                        .bg(color(RAISE))
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(color(INK))
                        .hover(|pill| pill.bg(color(0x1d1e23)))
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
                        .child(mono().px(px(6.0)).rounded_full().bg(white(0.14)).text_size(px(11.0)).child(stopped.to_string())),
                )
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .flex()
                    .child(window_button("minus", WindowControlArea::Min, "win-min"))
                    .child(window_button("square", WindowControlArea::Max, "win-max"))
                    .child(window_button("x", WindowControlArea::Close, "win-close")),
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
                .border_1()
                .border_color(white(0.08))
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
            .child(
                div()
                    .flex()
                    .gap(px(2.0))
                    .p(px(4.0))
                    .rounded_full()
                    .bg(color(RAISE))
                    .border_1()
                    .border_color(white(0.08))
                    .shadow(float())
                    .child(seg("rectangle-horizontal", "Focus", true))
                    .child(seg("layout-grid", "Grid", false)),
            )
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
                    .border_1()
                    .border_color(white(0.08))
                    .shadow(float())
                    .text_size(px(13.0))
                    .text_color(color(INK_3))
                    .child(icon("search", 14.0, color(INK_3)))
                    .child(div().flex_1().truncate().child("Jump to an agent or run a command"))
                    .child(mono().text_size(px(11.0)).px(px(6.0)).py(px(2.0)).rounded(px(6.0)).border_1().border_color(white(0.12)).child("Ctrl+K")),
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
                    .border_1()
                    .border_color(white(0.09))
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
                            .when_some(sheet.error.clone(), |field, error| field.child(div().text_size(px(12.5)).text_color(color(0xf2766b)).child(error))),
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
        let queue = self.queue(cx);
        let stage = self.stage(cx);
        let top = self.top_bar(cx);
        let dock = self.dock(cx);
        let sheet = self.sheet.as_ref().map(|sheet| self.sheet_view(sheet, cx));
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
    }
}
