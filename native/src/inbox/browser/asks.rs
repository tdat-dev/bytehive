//! Clicks held for your OK (sending, posting, paying, deleting): a card per
//! click, top right, above everything but dialogs (renderAsks in
//! browserview.ts, .bv-asks in browserview.css). The agent's call waits
//! until you answer, or gives up after ten minutes.

use super::live::{self, ask_line, host};
use super::*;

impl Inbox {
    fn browser_answer(&mut self, id: u64, ok: bool, cx: &mut Context<Self>) {
        let Some(hub) = self.hub_or_say(cx) else { return };
        hub.answer(id, ok);
        self.browser.asks.retain(|a| a.id != id);
        cx.notify();
    }

    pub(in crate::inbox) fn browser_asks_view(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let now = now_ms();
        let list: Vec<&Ask> = self.browser.asks.iter().filter(|a| live::live(a, now)).collect();
        if list.is_empty() {
            return None;
        }
        let width = (f32::from(window.viewport_size().width) - 48.0).min(380.0);
        let button = |id: (&'static str, usize), label: &'static str| div().id(id).flex_none().py(px(6.0)).px(px(12.0)).rounded(px(8.0)).text_size(px(12.0)).font_weight(FontWeight::SEMIBOLD).child(label);
        let cards = list.into_iter().map(|a| {
            let id = a.id;
            let at = host(&a.url);
            let sub = if at.is_empty() { "in the browser".to_string() } else { format!("on {at}") };
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .py(px(12.0))
                .pr(px(12.0))
                .pl(px(14.0))
                .rounded(px(12.0))
                .bg(color(PANEL))
                .border_1()
                .border_color(tint(NEEDS, 0.45))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(18.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false }])
                .child(div().flex_none().size(px(7.0)).rounded_full().bg(color(NEEDS)).shadow(vec![BoxShadow { color: tint(NEEDS, 0.22), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(3.0), inset: false }]))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(ask_line(&a.agent, &a.what)))
                        .child(div().text_size(px(12.0)).text_color(color(INK_3)).child(sub)),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .gap(px(6.0))
                        .child(
                            button(("bv-no", id as usize), "Don't allow")
                                .text_color(color(INK))
                                .bg(color(RAISE))
                                .border_1()
                                .border_color(white(0.12))
                                .hover(|b| b.border_color(white(0.24)))
                                .on_click(cx.listener(move |this, _, _, cx| this.browser_answer(id, false, cx))),
                        )
                        .child(button(("bv-ok", id as usize), "Allow").text_color(color(GROUND)).bg(color(INK)).hover(|b| b.opacity(0.9)).on_click(cx.listener(move |this, _, _, cx| this.browser_answer(id, true, cx)))),
                )
                .with_animation(("bv-ask", id as usize), Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()), |el, d| el.relative().top(px(-6.0 * (1.0 - d))))
        });
        Some(
            div()
                .absolute()
                .top(px(64.0))
                .right(px(24.0))
                .w(px(width))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(cards)
                .into_any_element(),
        )
    }
}
