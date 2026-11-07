//! The Agent Inbox, Direction D Cinema: black stage, floating panels, colour
//! only for state. Sizes and colours follow src/styles/inbox.css one to one.
//!
//! The stage's terminal is real. The queue and the plan limits are a fixture
//! copied from the running app, there to compare the look side by side; the
//! live fleet replaces them once the look passes.

use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::terminal::Terminal;

const GROUND: u32 = 0x050506;
const PANEL: u32 = 0x0e0f11;
const RAISE: u32 = 0x15161a;
const INK: u32 = 0xf4f4f5;
const INK_2: u32 = 0xb4b4bc;
const INK_3: u32 = 0x8c8c96;
const WORK: u32 = 0x6fd6a0;
const CLAUDE_TILE: u32 = 0x2f201c;
const QUEUE_W: f32 = 320.0;
const DOCK_H: f32 = 84.0;

const CLAUDE_LOGO: &[u8] = include_bytes!("../assets/clis/claude.png");

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

enum State {
    Idle,
    Working,
}

struct Agent {
    name: &'static str,
    who: Option<&'static str>,
    state: State,
    label: &'static str,
    cost: &'static str,
    age: &'static str,
    current: bool,
}

enum Group {
    Folded(&'static str, u32),
    Open(&'static str, u32, Vec<Agent>),
}

fn fixture() -> Vec<Group> {
    let agent = |name, who, state, label, cost, age, current| Agent { name, who, state, label, cost, age, current };
    vec![
        Group::Folded("ByteWaker", 3),
        Group::Folded("WhaleloSource", 2),
        Group::Open(
            "zoldify",
            3,
            vec![
                agent("Bob", None, State::Idle, "Idle · Done", "$84", "27s", false),
                agent("Ana", None, State::Idle, "Idle · Done", "$68", "27s", false),
            ],
        ),
        Group::Folded("FacebookAuto", 3),
        Group::Open(
            "bytehive",
            4,
            vec![
                agent("Handed to Eve", Some("Ana"), State::Working, "Working", "$53", "1h", false),
                agent("Bob", None, State::Working, "Working", "$7.30", "2m", true),
            ],
        ),
    ]
}

pub struct Inbox {
    terminal: Entity<Terminal>,
    logo: Arc<Image>,
}

impl Inbox {
    pub fn new(terminal: Entity<Terminal>) -> Self {
        Self { terminal, logo: Arc::new(Image::from_bytes(ImageFormat::Png, CLAUDE_LOGO.to_vec())) }
    }

    fn mark(&self, size: f32, radius: f32) -> Div {
        div()
            .relative()
            .flex_none()
            .size(px(size))
            .rounded(px(radius))
            .bg(color(CLAUDE_TILE))
            .border_1()
            .border_color(tint(0xd97757, 0.38))
            .flex()
            .items_center()
            .justify_center()
            .child(img(self.logo.clone()).size(px(size * 0.68)))
    }

    fn agent_row(&self, agent: &Agent) -> impl IntoElement {
        let tone = match agent.state {
            State::Working => color(WORK),
            State::Idle => color(INK_3),
        };
        let ring = if agent.current { RAISE } else { PANEL };
        let dot = div().absolute().right(px(-3.0)).bottom(px(-3.0)).size(px(13.0)).rounded_full().bg(color(ring)).flex().items_center().justify_center().child(
            div().size(px(9.0)).rounded_full().bg(tone),
        );
        let state_line = div()
            .flex()
            .items_center()
            .text_size(px(12.5))
            .text_color(color(INK_3))
            .when_some(agent.who, |line, who| line.child(format!("{who} · ")))
            .child(div().text_color(tone).child(agent.label))
            .child(mono().text_size(px(11.5)).ml(px(5.0)).child(format!("· {}", agent.cost)));
        div()
            .flex()
            .gap(px(10.0))
            .p(px(10.0))
            .rounded(px(12.0))
            .text_color(color(INK))
            .when(agent.current, |row| row.bg(color(RAISE)).border_1().border_color(white(0.1)))
            .when(!agent.current, |row| row.hover(|row| row.bg(white(0.03))))
            .child(self.mark(24.0, 7.0).child(dot))
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
                            .child(div().font_weight(FontWeight::MEDIUM).text_size(px(14.0)).child(agent.name))
                            .child(mono().text_size(px(12.0)).text_color(color(INK_3)).child(agent.age)),
                    )
                    .child(state_line),
            )
    }

    fn project(&self, name: &'static str, count: u32, open: bool) -> impl IntoElement {
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
                    .hover(|row| row.bg(white(0.04)).text_color(color(INK)))
                    .child(icon(if open { "chevron-down" } else { "chevron-right" }, 10.0, tint(INK_2, 0.7)))
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .child(mono().font_weight(FontWeight::NORMAL).text_size(px(12.0)).text_color(color(INK_3)).child(count.to_string())),
            )
            .child(div().size(px(30.0)).rounded(px(9.0)).flex().items_center().justify_center().child(icon("notebook-text", 14.0, tint(INK_3, 0.7))))
    }

    fn limit(label: &'static str, percent: u32, note: Option<&'static str>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(5.0))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_baseline()
                    .text_size(px(12.5))
                    .child(div().text_color(color(INK_2)).child(label))
                    .child(mono().font_weight(FontWeight::MEDIUM).text_size(px(12.0)).text_color(color(INK)).child(format!("{percent}%"))),
            )
            .child(
                div().h(px(4.0)).rounded_full().bg(white(0.08)).overflow_hidden().child(
                    div().h_full().min_w(px(2.0)).w(relative(percent as f32 / 100.0)).rounded_full().bg(color(INK_2)),
                ),
            )
            .when_some(note, |limit, note| limit.child(div().text_size(px(11.5)).text_color(color(INK_3)).child(note)))
    }

    fn limits_head(name: &'static str, stamp: &'static str) -> Div {
        div()
            .flex()
            .gap(px(6.0))
            .items_baseline()
            .text_size(px(11.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color(INK_3))
            .child(name)
            .child(div().font_weight(FontWeight::NORMAL).child(stamp))
    }

    fn queue(&self) -> impl IntoElement {
        let mut list = div().flex().flex_col().flex_1().min_h_0().px(px(8.0)).pt(px(6.0));
        for group in fixture() {
            list = match group {
                Group::Folded(name, count) => list.child(self.project(name, count, false)),
                Group::Open(name, count, agents) => {
                    let mut open = list.child(self.project(name, count, true));
                    for agent in &agents {
                        open = open.child(self.agent_row(agent));
                    }
                    open
                }
            };
        }
        let plan = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .px(px(14.0))
            .pt(px(12.0))
            .pb(px(14.0))
            .border_t_1()
            .border_color(hairline())
            .child(Self::limits_head("Claude Max limits", "as of Nov 8, 12:52 AM"))
            .child(Self::limit("This week", 17, Some("resets Sat 01:00 AM")))
            .child(div().h(px(1.0)).bg(hairline()))
            .child(Self::limits_head("Codex Plus limits", "as of Nov 6, 02:10 PM"))
            .child(Self::limit("5 hours", 0, None))
            .child(Self::limit("This week", 8, Some("resets in 22h 47m")));
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
            .child(plan)
    }

    fn stage_head(&self) -> impl IntoElement {
        let tab = |label: &'static str| div().h(px(28.0)).px(px(13.0)).flex().items_center().rounded_full().hover(|tab| tab.bg(white(0.06)).text_color(color(INK))).child(label);
        let control = |name: &'static str| {
            div().size(px(30.0)).rounded_full().flex().items_center().justify_center().hover(|control| control.bg(white(0.06))).child(icon(name, 15.0, color(INK_3)))
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .pt(px(20.0))
            .pr(px(20.0))
            .pb(px(12.0))
            .pl(px(20.0))
            .child(self.mark(42.0, 12.0).border_color(tint(WORK, 0.6)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(div().text_size(px(12.5)).text_color(color(INK_2)).child("claude"))
                    .child(div().text_size(px(22.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Bob")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .h(px(28.0))
                    .px(px(12.0))
                    .rounded_full()
                    .bg(tint(WORK, 0.12))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(WORK))
                    .child(div().size(px(8.0)).rounded_full().bg(color(WORK)))
                    .child("Working"),
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
            .child(div().flex().items_center().child(control("ellipsis")).child(control("pencil")).child(control("x")))
    }

    fn stage(&self) -> impl IntoElement {
        let edge = |from: Hsla, to: Hsla| div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(from, 0.0), linear_color_stop(to, 1.0)));
        let lit = tint(WORK, 0.45);
        let clear = tint(WORK, 0.0);
        div()
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
            .overflow_hidden()
            .child(div().absolute().top_0().left_0().right_0().h(px(2.0)).flex().child(edge(clear, lit)).child(edge(lit, clear)))
            .child(self.stage_head())
            .child(div().flex_1().min_h_0().pt(px(4.0)).px(px(24.0)).pb(px(12.0)).child(self.terminal.clone()))
    }

    fn top_bar(&self) -> impl IntoElement {
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
                    .child("2 are working."),
            )
            .child(
                div()
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
                    .child(icon("rotate-ccw", 13.0, color(INK)))
                    .child("Resume all")
                    .child(mono().px(px(6.0)).rounded_full().bg(white(0.14)).text_size(px(11.0)).child("11")),
            )
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

    fn dock(&self) -> impl IntoElement {
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
                    .child("New agent"),
            )
            .child(round("history"))
            .child(round("circle-question-mark"))
            .child(round("settings"))
    }
}

impl Render for Inbox {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(color(GROUND))
            .font_family("Geist")
            .line_height(relative(1.2))
            .child(self.top_bar())
            .child(div().flex().flex_1().min_h_0().child(self.queue()).child(self.stage()))
            .child(self.dock())
    }
}
