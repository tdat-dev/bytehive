//! An open card (.kb-detail, renderDetail in kanban.ts): title, labels, due
//! date, its agent, description, checklist, and Delete card.

use super::look::{MUTED_2, SURFACE_0, TICK_INK, accent_dim, field_box, line, line_strong, quiet_btn, sec_label};
use super::*;

use gpui_kit::component::Sizable as _;

impl Inbox {
    pub(super) fn board_detail(&self, b: &BoardView, card: &model::Card, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let f = b.focused;
        let back = div()
            .id("kb-detail-back")
            .self_start()
            .py(px(5.0))
            .px(px(9.0))
            .rounded(px(8.0))
            .text_size(px(11.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color(INK_3))
            .hover(|s| s.bg(color(RAISE)).text_color(color(INK_2)))
            .on_click(cx.listener(|this, _, window, cx| this.board_back(window, cx)))
            .child("← Board");
        let title = div()
            .w_full()
            .py(px(5.0))
            .px(px(8.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(if f == Some(Field::Title) { accent_dim() } else { transparent_black() })
            .when(f == Some(Field::Title), |d| d.bg(color(SURFACE_0)))
            .when(f != Some(Field::Title), |d| d.hover(|s| s.bg(color(PANEL))))
            .font_weight(FontWeight::SEMIBOLD)
            .line_height(relative(1.3))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(Textarea::new(&b.inputs.title).appearance(false).with_size(px(17.0 / 0.875)));
        div()
            .id("kb-detail")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .pt(px(12.0))
            .px(px(16.0))
            .pb(px(20.0))
            .child(back)
            .child(title)
            .child(self.board_labels(card, cx))
            .child(self.board_due(b, card, cx))
            .child(self.board_agent(b, card, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(sec_label("Description"))
                    .child(field_box(f == Some(Field::Desc), line_strong(), true).w_full().py(px(8.0)).px(px(10.0)).rounded(px(10.0)).line_height(relative(1.5)).child(Textarea::new(&b.inputs.desc).appearance(false).with_size(px(12.5 / 0.875)))),
            )
            .child(self.board_checklist(b, card, cx))
            .child(
                div()
                    .id("kb-detail-del")
                    .self_start()
                    .mt(px(2.0))
                    .py(px(7.0))
                    .px(px(13.0))
                    .rounded(px(8.0))
                    .bg(tint(DANGER, 0.1))
                    .border_1()
                    .border_color(tint(DANGER, 0.3))
                    .text_size(px(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color(DANGER))
                    .hover(|s| s.bg(tint(DANGER, 0.2)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(b) = &mut this.board {
                            b.asking = b.open.clone().map(Ask::Card);
                        }
                        cx.notify();
                    }))
                    .child("Delete card"),
            )
    }

    /// .kb-swatches: the six labels, ticked when on.
    fn board_labels(&self, card: &model::Card, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let swatches: Vec<AnyElement> = model::LABELS
            .iter()
            .map(|&(key, hex, name)| {
                let on = card.labels.iter().any(|k| k == key);
                div()
                    .id(SharedString::from(format!("kb-swatch-{key}")))
                    .w(px(42.0))
                    .h(px(26.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(color(hex))
                    .border_1()
                    .border_color(hsla(0.0, 0.0, 0.0, 0.25))
                    .when(on, |s| s.shadow(vec![BoxShadow { color: color(INK), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(4.0), inset: false }, BoxShadow { color: color(GROUND), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(2.0), inset: false }]))
                    .tooltip(tip(name))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.board_edit_card(
                            move |c| {
                                if c.labels.iter().any(|k| k == key) {
                                    c.labels.retain(|k| k != key);
                                } else {
                                    c.labels.push(key.to_string());
                                }
                            },
                            window,
                            cx,
                        )
                    }))
                    .when(on, |s| s.child(svg().path("icons/kb-swatch-on.svg").size(px(12.0)).text_color(color(TICK_INK))))
                    .into_any_element()
            })
            .collect();
        div().flex().flex_col().gap(px(8.0)).child(sec_label("Labels")).child(div().flex().flex_wrap().gap(px(7.0)).children(swatches))
    }

    /// .kb-due-row: the date, and Clear when there is one.
    fn board_due(&self, b: &BoardView, card: &model::Card, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = card.id.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(sec_label("Due date"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(field_box(b.focused == Some(Field::Due), line_strong(), false).w(px(140.0)).py(px(6.0)).px(px(9.0)).rounded(px(8.0)).font_family("Geist Mono").child(Input::new(&b.inputs.due).appearance(false).with_size(px(12.0 / 0.875))))
                    .when(card.due.is_some(), |r| r.child(quiet_btn("kb-due-clear", "Clear").on_click(cx.listener(move |this, _, window, cx| this.board_set_due(id.clone(), None, window, cx))))),
            )
    }

    /// Agent: who works it (focus or Unassign), and Send to agent….
    fn board_agent(&self, b: &BoardView, card: &model::Card, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let running = self.board_agents(cx).into_iter().any(|(_, _, r)| r);
        let mut row = div().flex().flex_wrap().items_center().gap(px(8.0));
        if let Some(name) = card.assignee.clone() {
            let who = name.clone();
            row = row
                .child(
                    div()
                        .id("kb-assignee-big")
                        .max_w_full()
                        .py(px(4.0))
                        .px(px(10.0))
                        .rounded_full()
                        .border_1()
                        .border_color(white(0.08))
                        .bg(white(0.08))
                        .text_size(px(12.0))
                        .text_color(color(INK))
                        .truncate()
                        .hover(|s| s.bg(white(0.2)))
                        .tooltip(tip("Focus this agent's pane"))
                        .on_click(cx.listener(move |this, _, window, cx| this.board_focus_agent(&who, window, cx)))
                        .child(format!("⚡ {name}")),
                )
                .child(quiet_btn("kb-unassign", "Unassign").on_click(cx.listener(|this, _, window, cx| this.board_unassign(window, cx))));
        }
        if running {
            let spot = b.pick_spot.clone();
            row = row.child(
                div()
                    .id("kb-agent-pick")
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(13.5))
                    .text_color(color(INK))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(b) = &mut this.board {
                            b.picking = !b.picking;
                        }
                        cx.notify();
                    }))
                    .child(super::super::dropdown::track(&spot))
                    .child("Send to agent…")
                    .child(icon("chev-down", 10.0, color(INK)).opacity(0.7)),
            );
        } else if card.assignee.is_none() {
            row = row.child(div().text_size(px(12.0)).text_color(color(MUTED_2)).child("No running agents"));
        }
        div().flex().flex_col().gap(px(8.0)).child(sec_label("Agent")).child(row)
    }

    /// Checklist: done/total, the bar, each item (tick, ×), and Add an item….
    fn board_checklist(&self, b: &BoardView, card: &model::Card, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let total = card.checklist.len();
        let done = card.checklist.iter().filter(|i| i.done).count();
        let share = if total == 0 { 0.0 } else { ((done as f32 / total as f32) * 100.0).round() / 100.0 };
        let items: Vec<AnyElement> = card
            .checklist
            .iter()
            .enumerate()
            .map(|(n, item)| {
                let group: SharedString = format!("kb-cl-{n}").into();
                let (tick, drop) = (item.id.clone(), item.id.clone());
                div()
                    .id(("kb-cl-item", n))
                    .group(group.clone())
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .py(px(5.0))
                    .px(px(4.0))
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(color(PANEL)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let tick = tick.clone();
                        this.board_edit_card(
                            move |c| {
                                if let Some(i) = c.checklist.iter_mut().find(|i| i.id == tick) {
                                    i.done = !i.done;
                                }
                            },
                            window,
                            cx,
                        )
                    }))
                    .child(
                        div()
                            .flex_none()
                            .size(px(17.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.0))
                            .border(px(1.5))
                            .border_color(if item.done { color(WORK) } else { line_strong() })
                            .when(item.done, |x| x.bg(color(WORK)).child(svg().path("icons/kb-box.svg").size(px(12.0)).text_color(color(TICK_INK)))),
                    )
                    .child(div().flex_1().min_w_0().text_size(px(12.5)).text_color(color(if item.done { MUTED_2 } else { INK })).when(item.done, |t| t.line_through()).child(item.text.clone()))
                    .child(
                        div()
                            .id(("kb-cl-x", n))
                            .flex_none()
                            .size(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.0))
                            .text_size(px(15.0))
                            .line_height(relative(1.0))
                            .text_color(color(MUTED_2))
                            .opacity(0.0)
                            .group_hover(group, |s| s.opacity(1.0))
                            .hover(|s| s.bg(tint(DANGER, 0.16)).text_color(color(DANGER)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                let drop = drop.clone();
                                this.board_edit_card(move |c| c.checklist.retain(|i| i.id != drop), window, cx)
                            }))
                            .child("×"),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(div().flex().items_baseline().child(sec_label("Checklist")).child(mono().ml(px(4.0)).text_size(px(11.5)).text_color(color(MUTED_2)).child(format!("{done}/{total}"))))
            .child(div().h(px(6.0)).rounded(px(4.0)).bg(color(RAISE)).overflow_hidden().child(div().h_full().w(relative(share)).rounded(px(4.0)).bg(color(WORK))))
            .children(items)
            .child(field_box(b.focused == Some(Field::Item), line(), true).mt(px(4.0)).w_full().py(px(6.0)).px(px(9.0)).rounded(px(8.0)).child(Input::new(&b.inputs.item).appearance(false).with_size(px(12.0 / 0.875))))
    }
}
