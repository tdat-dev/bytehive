//! The top bar and the floating dock.

use super::*;

/// Minimise, maximise, close: Windows' own caption buttons, drawn. Each one
/// occludes: GPUI answers Windows' hit test with the first control area under
/// the mouse in paint order, so the bar's Drag behind a button would win and
/// turn every click on it into a window drag.
pub(super) fn window_buttons() -> Div {
    let button = |name: &'static str, area: WindowControlArea, id: &'static str, close: bool| {
        div()
            .id(id)
            .occlude()
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
        .flex()
        .child(button("minus", WindowControlArea::Min, "win-min", false))
        .child(button("square", WindowControlArea::Max, "win-max", false))
        .child(button("x", WindowControlArea::Close, "win-close", true))
}

impl Inbox {
    pub(super) fn top_bar(&self, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let stopped = view.statuses.iter().filter(|s| s.state == State::Stopped).count();
        let (lead, rest) = headline(&view.statuses, &view.worked);
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(68.0))
            .pl(px(12.0))
            .pr(px(4.0))
            .window_control_area(WindowControlArea::Drag)
            .child(div().id("home").occlude().tooltip(tip("Home (keep agents running)")).on_click(cx.listener(|this, _, window, cx| this.open_home(window, cx))).size(px(30.0)).rounded(px(7.0)).flex().items_center().justify_center().hover(|h| h.bg(color(0x0e0f11))).child(icon("home", 16.0, color(INK_3))))
            .children(self.co_crumb(cx))
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
            // The door to the Company, as Office was in today's bar: a focused
            // terminal keeps Ctrl+Shift+O, so the mouse needs a way in.
            .child(
                div()
                    .id("company-door")
                    .occlude()
                    .tooltip(tip("Company (Ctrl+Shift+O)"))
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px(px(14.0))
                    .when(stopped > 0, |d| d.mr(px(8.0)))
                    .when(stopped == 0, |d| d.mr(px(146.0)))
                    .rounded_full()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color(INK_2))
                    .hover(|d| d.bg(white(0.06)).text_color(color(INK)))
                    .on_click(cx.listener(|this, _, window, cx| this.open_company(window, cx)))
                    .child("Company"),
            )
            .when(stopped > 0, |bar| {
                bar.child(
                    div()
                        .id("resume-all")
                        .occlude()
                        .tooltip(tip("Resume every stopped agent"))
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
                                    this.start_seat(index, cx);
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
            .child(div().absolute().top_0().right_0().child(window_buttons()))
    }

    pub(super) fn dock(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let seg = |id: &'static str, name: &'static str, label: &'static str, on: bool, title: &'static str| {
            div()
                .id(id)
                .tooltip(tip(title))
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
        let round = |id: &'static str, title: &'static str| {
            div()
                .id(id)
                .tooltip(tip(title))
                .size(px(40.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(color(RAISE))
                .shadow(float())
                .hover(|round| round.bg(color(0x1d1e23)))
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
            .child(div().flex().gap(px(2.0)).p(px(4.0)).rounded_full().bg(color(RAISE)).shadow(float()).child(seg("dock-focus", "focus", "Focus", !self.grid.on, "Focus: one agent at a time, full size (Alt+S switches)").on_click(cx.listener(|this, _, window, cx| this.set_grid(false, window, cx)))).child(seg("dock-grid", "grid", "Grid", self.grid.on, "Grid: the agent you work with as a terminal, the others as cards beside it (Alt+S switches, Alt+P adds one)").on_click(cx.listener(|this, _, window, cx| this.set_grid(!this.grid.on, window, cx)))))
            .child(
                div()
                    .id("dock-search")
                    .tooltip(tip("Jump to an agent or run a command (Ctrl+K)"))
                    .hover(|s| s.text_color(color(INK_2)))
                    .on_click(cx.listener(|this, _, window, cx| this.open_palette(window, cx)))
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
                    .tooltip(tip("New agent (Ctrl+Shift+T)"))
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
            .child(round("dock-sessions", "Sessions: read back earlier conversations (Alt+O)").on_click(cx.listener(|this, _, window, cx| this.open_sessions(window, cx))).child(icon("sessions", 17.0, color(INK_2))))
            .child(round("dock-help", "What ByteHive can do (?)").text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK_2)).child("?"))
            .child(round("dock-settings", "Settings (Ctrl+,)").on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))).child(icon("settings", 17.0, color(INK_2))))
    }
}
