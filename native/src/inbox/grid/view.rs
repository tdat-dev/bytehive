//! The Grid on screen: its terminal on the left, the cards on the right
//! (inbox.css .split-pin and .ib-rail). Every terminal in it wears the same
//! small header; the one you are talking to only gets a brighter edge.

use crate::gridfit::{CARD_GAP, CARD_W, GAP};

use super::*;

/// .ib-pill, as the Grid's header and its cards size it: 24px tall, 12px words.
pub(super) fn small_pill(state: State) -> Div {
    let lit = hue_of(state);
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(7.0))
        .h(px(24.0))
        .px(px(10.0))
        .rounded_full()
        .bg(lit.map(|h| tint(h, 0.12)).unwrap_or(white(0.06)))
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color(lit.unwrap_or(INK_2)))
        .child(if waits(state) { icon("diamond", 11.0, color(NEEDS)).into_any_element() } else { div().size(px(8.0)).rounded_full().bg(color(lit.unwrap_or(INK_2))).into_any_element() })
        .child(label_of(state))
}

impl Inbox {
    /// The list, unless it stepped aside for the Grid.
    pub(in crate::inbox) fn queue_unless_wide(&self, view: &View, cx: &mut Context<Self>) -> Option<AnyElement> {
        (!(self.grid.wide && self.grid.shown.is_some())).then(|| self.queue(view, cx).into_any_element())
    }

    /// The stage: one agent full size, or the Grid.
    pub(in crate::inbox) fn stage_or_grid(&self, view: &View, cx: &mut Context<Self>) -> AnyElement {
        let Some(shown) = self.grid.shown.clone() else { return self.stage(view, cx).into_any_element() };
        let this = cx.entity().downgrade();
        // How much room the Grid has, measured as drawn, for the next layout.
        let measure = canvas(
            move |bounds, _, cx| {
                let area = Area { width: f32::from(bounds.size.width), height: f32::from(bounds.size.height) };
                let _ = this.update(cx, |this, cx| {
                    if this.grid.area != Some(area) {
                        this.grid.area = Some(area);
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        let rail_w = shown.cols as f32 * CARD_W + (shown.cols as f32 - 1.0) * CARD_GAP;
        div()
            .id("grid")
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .gap(px(GAP))
            .mr(px(16.0))
            .mb(px(DOCK_H))
            .when(self.grid.wide, |g| g.ml(px(16.0)))
            .child(measure)
            .child(self.grid_terminal(shown.big, view, cx))
            .when(!shown.cards.is_empty(), |g| g.child(div().flex_none().w(px(rail_w)).h_full().child(self.rail(&shown, view, cx))))
            .into_any_element()
    }

    /// The agent you work with, as a real terminal.
    fn grid_terminal(&self, index: usize, view: &View, cx: &mut Context<Self>) -> AnyElement {
        let seat = &self.fleet.seats[index];
        let status = &view.statuses[index];
        let needs = waits(status.state);
        let running = seat.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
        let edge = |from: Hsla, to: Hsla| div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(from, 0.0), linear_color_stop(to, 1.0)));
        let (lit, clear) = (tint(WORK, 0.6), tint(WORK, 0.0));
        // In the Grid's terminal: the project first, as the cards are grouped by it, then the job.
        let job = seat.spec.title.clone().or_else(|| seat.branch.clone());
        let place = [Some(seat.project()), job].into_iter().flatten().collect::<Vec<_>>().join(" \u{00b7} ");
        let name = seat.spec.name.clone();
        let button = |id: &'static str, glyph: &'static str, title: String| {
            div()
                .id(id)
                .tooltip(tip(title))
                .size(px(30.0))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .text_size(px(14.0))
                .text_color(color(INK_3))
                .hover(|b| b.text_color(color(INK)).bg(white(0.06)))
                .child(glyph)
        };
        let head = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.0))
            .pt(px(12.0))
            .pr(px(12.0))
            .pb(px(8.0))
            .pl(px(14.0))
            .child(self.mark(&seat.command(), 24.0, 7.0).mr(px(4.0)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.0))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(6.0))
                            .min_w_0()
                            .child(div().flex_none().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(name.clone()))
                            .child(div().min_w_0().truncate().text_size(px(13.0)).text_color(color(INK_2)).child(format!("\u{00b7} {}", cli_label(&seat.spec.cli)))),
                    )
                    .when(!place.is_empty(), |c| c.child(div().truncate().text_size(px(12.5)).text_color(color(INK_3)).child(place))),
            )
            .child(small_pill(status.state))
            .child(button("grid-full", "\u{2922}", "Open full size".to_string()).on_click(cx.listener(move |this, _, window, cx| this.open_full(index, window, cx))))
            .child(button("grid-close", "\u{2715}", "Take out of Grid".to_string()).on_click(cx.listener(move |this, _, window, cx| this.close_from_grid(index, window, cx))));
        let ring_color = if needs { color(NEEDS) } else { white(0.24) };
        let mut shadow = vec![ring(ring_color)];
        if needs {
            // 0 20px 50px rgba(255, 190, 110, 0.07)
            shadow.push(BoxShadow { color: rgba(0xffbe6e12).into(), offset: point(px(0.0), px(20.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false });
        }
        div()
            .id("grid-terminal")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(px(18.0))
            .bg(color(PANEL))
            .shadow(shadow)
            .overflow_hidden()
            .when(running, |p| p.child(div().absolute().top_0().left_0().right_0().h(px(2.0)).flex().child(edge(clear, lit)).child(edge(lit, clear))))
            .child(head)
            .children(seat.terminal.clone().map(|terminal| {
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .pt(px(4.0))
                    .px(px(14.0))
                    .pb(px(10.0))
                    .child(terminal)
                    .child(div().absolute().top_0().left_0().right_0().h(px(8.0)).bg(linear_gradient(180.0, linear_color_stop(color(PANEL), 0.0), linear_color_stop(tint(PANEL, 0.0), 1.0))))
            }))
            .into_any_element()
    }
}
