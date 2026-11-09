//! The board's two floating asks: "Send to agent…" (the app's drop-down,
//! selectmenu.css, listing the project's running agents) and the delete
//! confirm kanban.ts asks with confirm().

use super::*;

impl Inbox {
    /// .sm-pop under "Send to agent…".
    pub(super) fn board_pick(&self, b: &BoardView, cx: &mut Context<Self>) -> Option<AnyElement> {
        let agents: Vec<(usize, String)> = self.board_agents(cx).into_iter().filter(|(_, _, running)| *running).map(|(i, name, _)| (i, name)).collect();
        if agents.is_empty() {
            return None;
        }
        let under = b.pick_spot.get();
        let lines: Vec<AnyElement> = agents
            .into_iter()
            .map(|(index, name)| {
                div()
                    .id(("kb-pick", index))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .min_h(px(34.0))
                    .pl(px(8.0))
                    .pr(px(12.0))
                    .py(px(6.0))
                    .rounded(px(9.0))
                    .text_color(color(INK))
                    .hover(|l| l.bg(white(0.08)))
                    .on_click(cx.listener(move |this, _, window, cx| this.board_send(index, window, cx)))
                    .child(div().flex_none().size(px(16.0)))
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .into_any_element()
            })
            .collect();
        let close = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            if let Some(b) = &mut this.board {
                b.picking = false;
            }
            cx.notify();
        });
        Some(
            div()
                .absolute()
                .inset_0()
                .on_mouse_down(MouseButton::Left, close)
                .child(
                    deferred(
                        anchored().position(point(under.left(), under.bottom() + px(6.0))).snap_to_window().child(
                            div()
                                .id("kb-pick-pop")
                                .min_w(px(220.0))
                                .max_w(px(360.0))
                                .p(px(5.0))
                                .rounded(px(14.0))
                                .bg(color(0x16171b))
                                .shadow(vec![
                                    BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.6), offset: point(px(0.0), px(22.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false },
                                    BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.4), offset: point(px(0.0), px(2.0)), blur_radius: px(6.0), spread_radius: px(0.0), inset: false },
                                    ring(white(0.1)),
                                ])
                                .text_size(px(13.5))
                                .line_height(relative(1.3))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .children(lines),
                        ),
                    )
                    .with_priority(1),
                )
                .into_any_element(),
        )
    }

    /// "Delete this card?" / "Delete list … and its N card(s)?", with OK.
    pub(super) fn board_ask(&self, b: &BoardView, ask: Ask, cx: &mut Context<Self>) -> AnyElement {
        let text = match &ask {
            Ask::Card(_) => "Delete this card?".to_string(),
            Ask::List(id) => {
                let (title, n) = b.board.lists.iter().find(|l| &l.id == id).map_or((String::new(), 0), |l| (l.title.clone(), l.cards.len()));
                format!("Delete list \"{title}\" and its {n} card(s)?")
            }
        };
        let btn = |id: &'static str, label: &'static str, danger: bool| {
            div()
                .id(id)
                .flex_none()
                .h(px(36.0))
                .px(px(14.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .when(danger, |b| b.bg(color(DANGER)).text_color(color(GROUND)))
                .when(!danger, |b| b.bg(white(0.07)).text_color(color(INK)).hover(|b| b.bg(white(0.12))))
                .child(label)
        };
        div()
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .items_center()
            .justify_center()
            .p(px(16.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w(px(440.0))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .p(px(22.0))
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .child(div().text_size(px(14.0)).line_height(relative(1.5)).text_color(color(INK)).child(text))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(btn("kb-ask-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                                if let Some(b) = &mut this.board {
                                    b.asking = None;
                                }
                                cx.notify();
                            })))
                            .child(btn("kb-ask-ok", "OK", true).on_click(cx.listener(|this, _, window, cx| this.board_confirm(window, cx)))),
                    ),
            )
            .into_any_element()
    }
}
