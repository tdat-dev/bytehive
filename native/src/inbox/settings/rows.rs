//! The pieces a Settings row is built from (modal.css .srow, .toggle, .seg,
//! .stepper, .sbtn and the inbox.css overrides on .set-shell).

use super::*;

/// .sr-txt: a bold line and a quiet one under it.
pub(super) fn text(title: &'static str, sub: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(3.0))
        .child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(title))
        .child(div().text_size(px(12.0)).line_height(relative(1.45)).text_color(color(INK_3)).child(sub.into()))
}

/// .srow: words on the left, the control on the right, a hairline under.
pub(super) fn row(title: &'static str, sub: impl Into<SharedString>) -> Div {
    div().flex().items_center().gap(px(18.0)).py(px(13.0)).border_b_1().border_color(hairline()).child(text(title, sub))
}

/// .toggle: the whole row flips the switch on its right.
pub(super) fn toggle(id: &'static str, on: bool, title: &'static str, sub: &'static str) -> Stateful<Div> {
    let switch = div()
        .relative()
        .flex_none()
        .w(px(38.0))
        .h(px(22.0))
        .rounded(px(11.0))
        .bg(if on { color(0xffffff) } else { white(0.08) })
        .child(div().absolute().top(px(3.0)).left(px(if on { 19.0 } else { 3.0 })).size(px(16.0)).rounded_full().bg(color(if on { 0x000000 } else { INK_3 })));
    div().id(id).flex().items_center().gap(px(18.0)).py(px(13.0)).border_b_1().border_color(hairline()).child(text(title, sub)).child(switch)
}

/// .seg: a pill of choices, the chosen one lit.
pub(super) fn seg() -> Div {
    div().flex().flex_none().p(px(3.0)).rounded_full().bg(white(0.05))
}

pub(super) fn seg_button(id: (&'static str, usize), label: String, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .min_w(px(34.0))
        .py(px(6.0))
        .px(px(12.0))
        .flex()
        .justify_center()
        .rounded_full()
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(if on { INK } else { INK_3 }))
        .when(on, |b| b.bg(white(0.14)))
        .when(!on, |b| b.hover(|b| b.text_color(color(INK))))
        .child(label)
}

/// .stepper: − value +, each end stopping at its limit.
pub(super) fn stepper(value: String, less: Stateful<Div>, more: Stateful<Div>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(2.0))
        .p(px(3.0))
        .rounded_full()
        .bg(white(0.05))
        .child(less)
        .child(mono().min_w(px(40.0)).flex().justify_center().text_size(px(13.0)).text_color(color(INK)).child(value))
        .child(more)
}

pub(super) fn step_button(id: &'static str, label: &'static str, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_size(px(16.0))
        .text_color(color(INK_3))
        .when(!enabled, |b| b.opacity(0.35))
        .when(enabled, |b| b.hover(|b| b.bg(white(0.08)).text_color(color(INK))))
        .child(label)
}

/// .sbtn: a quiet pill button.
pub(super) fn button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .py(px(8.0))
        .px(px(15.0))
        .rounded_full()
        .bg(white(0.08))
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(INK))
        .hover(|b| b.bg(white(0.14)))
        .child(label)
}

/// .set-select: the button a drop-down opens from.
pub(super) fn select(id: &'static str, label: String, open: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .h(px(34.0))
        .px(px(12.0))
        .rounded_full()
        .bg(white(if open { 0.12 } else { 0.06 }))
        .hover(|b| b.bg(white(0.09)))
        .text_size(px(13.0))
        .text_color(color(INK))
        .child(label)
        .child(icon("chev-down", 10.0, color(INK_2)).opacity(0.7))
}
