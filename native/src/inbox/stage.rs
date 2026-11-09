//! The stage: the focused agent's header and its terminal.

use super::*;

impl Inbox {
    pub(super) fn stage_head(&self, seat: &Seat, index: usize, status: &TaskStatus, cx: &mut Context<Self>) -> AnyElement {
        let state = status.state;
        let cli_label = cli_label(&seat.spec.cli);
        let lit = hue_of(state);
        let name = seat.spec.name.clone();
        let act = |id: &'static str, label: &'static str, title: String| {
            div()
                .id(id)
                .tooltip(tip(title))
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
                    .tooltip(tip(format!("Start {name} again, in the conversation it was last in")))
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
            .child(act("changes", "Changes", format!("What {name} changed (Alt+R)")).on_click(cx.listener(move |this, _, window, cx| this.open_changes(index, window, cx))))
            .child(act("history", "History", format!("What {name} has done (Alt+H)")))
        } else {
            let terminal = seat.terminal.clone();
            let model_terminal = seat.terminal.clone();
            acts.child(act("cmds", "Commands", format!("Type / so {name}'s CLI shows its own commands")).on_click(cx.listener(move |_, _, window, cx| {
                // Type / so the CLI shows its own commands.
                if let Some(t) = &terminal {
                    t.read(cx).type_text("/");
                    t.read(cx).focus_handle().focus(window, cx);
                }
            })))
            .when(has_model, |acts| {
                acts.child(act("model", "Model", "Open the CLI's own model picker".to_string()).on_click(cx.listener(move |_, _, window, cx| {
                    if let Some(t) = &model_terminal {
                        t.read(cx).type_text("/model\r");
                        t.read(cx).focus_handle().focus(window, cx);
                    }
                })))
            })
            .child(act("changes", "Changes", format!("What {name} changed (Alt+R)")).on_click(cx.listener(move |this, _, window, cx| this.open_changes(index, window, cx))))
            .child(act("history", "History", format!("What {name} has done (Alt+H)")))
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

    pub(super) fn stage(&self, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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

    pub(super) fn resume(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.start_seat(index, cx);
        self.fleet.focus(index, window, cx);
        cx.notify();
    }
}
