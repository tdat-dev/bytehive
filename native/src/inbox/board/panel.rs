//! The drawer the board sits in (.dock-panel, .dp-head in dock.css): a
//! floating panel on the right, above the stage, clear of the dock.

use super::*;

impl Inbox {
    pub(in crate::inbox) fn board_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let b = self.board.as_ref()?;
        let body = match b.open.as_ref().and_then(|id| model::find_card(&b.board, id)) {
            Some((l, c)) => self.board_detail(b, &b.board.lists[l].cards[c], cx).into_any_element(),
            None if b.dir.is_none() => div().py(px(40.0)).px(px(20.0)).text_center().text_size(px(12.5)).text_color(color(look::MUTED_2)).child("Open a project to start its board.").into_any_element(),
            None => self.board_lists(b, cx).into_any_element(),
        };
        let ask = b.asking.clone().map(|a| self.board_ask(b, a, cx));
        let pick = b.picking.then(|| self.board_pick(b, cx)).flatten();
        Some(
            div()
                .id("kb-panel")
                .track_focus(&b.focus)
                .absolute()
                .top(px(60.0))
                .right(px(16.0))
                .bottom(px(DOCK_H))
                .w(px(820.0))
                .max_w(relative(0.92))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(18.0))
                .bg(color(PANEL))
                .text_color(color(INK))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.7), offset: point(px(0.0), px(30.0)), blur_radius: px(80.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                // A click anywhere but in a field ends typing there, as a
                // field losing focus does in today's app.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.board_blur(window, cx);
                        cx.stop_propagation();
                    }),
                )
                .child(head(cx))
                .child(div().flex_1().min_h_0().flex().flex_col().overflow_hidden().child(body))
                .children(ask)
                .children(pick)
                .into_any_element(),
        )
    }
}

/// .dp-head: "Board" and Close.
fn head(cx: &mut Context<Inbox>) -> impl IntoElement + use<> {
    div()
        .flex_none()
        .h(px(56.0))
        .pl(px(18.0))
        .pr(px(12.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .border_b_1()
        .border_color(hairline())
        .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Board"))
        .child(
            div()
                .id("dp-close")
                .group("dp-close")
                .ml_auto()
                .size(px(30.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .text_color(color(INK_3))
                .hover(|b| b.bg(white(0.08)).text_color(color(INK)))
                .tooltip(tip("Close (Esc)"))
                .on_click(cx.listener(|this, _, window, cx| this.close_board(window, cx)))
                .child(svg().path("icons/kb-close.svg").size(px(14.0)).text_color(color(INK_3)).group_hover("dp-close", |s| s.text_color(color(INK)))),
        )
}
