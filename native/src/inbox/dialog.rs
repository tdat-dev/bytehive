//! The dialog look of modal.css that Scheduled agents and Usage open in:
//! .backdrop, .modal, .modal-head, .modal-body, .modal-foot, .btn, .btn-ghost
//! and the .input field box.

use super::*;

impl Inbox {
    /// The dialogs in this look that are open, to draw over everything else.
    /// Also starts the Scheduled agents loop on the first render.
    pub(super) fn dialogs(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.start_scheduler(window, cx);
        let mut out = Vec::new();
        if let Some(s) = &self.schedules {
            out.push(self.schedules_view(s, cx).into_any_element());
        }
        if let Some(u) = &self.usage_view {
            out.push(self.usage_modal(u, cx).into_any_element());
        }
        out
    }

    pub(super) fn dialog_open(&self) -> bool {
        self.schedules.is_some() || self.usage_view.is_some()
    }

    /// Esc in one of them: Scheduled agents shuts an open list first.
    pub(super) fn dialog_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.schedules.is_some() {
            self.schedules_escape(window, cx);
        } else if self.usage_view.is_some() {
            self.close_usage(window, cx);
        }
    }
}

/// --muted-2: the quietest words.
pub(super) const MUTED_2: u32 = 0x83838c;
/// --surface-0, the inside of a row.
pub(super) const SURFACE_0: u32 = 0x0a0a0c;
/// --surface-3, an off switch.
pub(super) const SURFACE_3: u32 = 0x1d1e23;

/// .backdrop.open: the dimmed page, the dialog centred on it. A press on
/// the dim (not the dialog) closes it.
pub(super) fn backdrop(id: &'static str, close: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    div().id(id).absolute().inset_0().bg(hsla(0.0, 0.0, 0.0, 0.6)).flex().items_center().justify_center().px(px(16.0)).py(px(20.0)).on_mouse_down(MouseButton::Left, close)
}

/// .modal, at its width: 460 for most, 560 for .modal.usage.
pub(super) fn modal(id: &'static str, width: f32, focus: &FocusHandle) -> Stateful<Div> {
    div()
        .id(id)
        .track_focus(focus)
        .w(px(width))
        // max-width: calc(100vw - 32px); max-height: calc(100vh - 40px),
        // as the backdrop's padding.
        .max_w_full()
        .max_h_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(18.0))
        .bg(color(PANEL))
        .text_color(color(INK))
        .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// .modal-head with its .t title; the caller adds anything between, then x().
pub(super) fn head(title: &'static str) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.0))
        .pt(px(18.0))
        .pr(px(18.0))
        .pb(px(8.0))
        .pl(px(22.0))
        .child(div().flex_none().text_size(px(18.0)).font_weight(FontWeight::SEMIBOLD).child(title))
}

/// .modal-head .x: the round close button at the head's right end.
pub(super) fn x(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .ml_auto()
        .flex_none()
        .size(px(30.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(white(0.05))
        .hover(|b| b.bg(white(0.1)))
        .child(icon("x", 15.0, color(INK_3)))
}

/// .modal-body: the column of what the dialog holds, scrolling when long.
pub(super) fn body(id: &'static str) -> Stateful<Div> {
    div().id(id).flex().flex_col().gap(px(15.0)).pt(px(10.0)).px(px(22.0)).pb(px(18.0)).overflow_y_scroll().min_h_0()
}

/// .modal-foot: the buttons, right-aligned.
pub(super) fn foot() -> Div {
    div().flex().flex_none().gap(px(8.0)).justify_end().pt(px(12.0)).px(px(18.0)).pb(px(18.0))
}

/// .btn-ghost.
pub(super) fn ghost(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(36.0))
        .px(px(16.0))
        .flex()
        .items_center()
        .rounded_full()
        .bg(white(0.06))
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(INK_2))
        .hover(|b| b.bg(white(0.12)).text_color(color(INK)))
        .child(label)
}

/// .btn: the white one.
pub(super) fn btn(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(36.0))
        .px(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(color(0xffffff))
        .text_color(color(0x000000))
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|b| b.bg(color(0xe9e9ec)))
        .child(label)
}

/// .input: the dark field box (a text box, or a drop-down's button).
pub(super) fn input_box() -> Div {
    div()
        .h(px(36.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .rounded(px(10.0))
        .bg(hsla(0.0, 0.0, 0.0, 0.3))
        .shadow(vec![ring(white(0.09))])
        .text_size(px(13.0))
        .text_color(color(INK))
}
