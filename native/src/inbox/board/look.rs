//! The Board's own tokens and pieces (tokens.css values the kanban rules in
//! dock.css draw with), shared by its list, card and detail views.

use super::*;

/// --surface-0: the inside of a field.
pub(super) const SURFACE_0: u32 = 0x0a0a0c;
/// --surface-3: counts, due badges, file chips.
pub(super) const SURFACE_3: u32 = 0x1d1e23;
/// --muted-2.
pub(super) const MUTED_2: u32 = 0x83838c;
/// .kb-badge.due.soon, and the Yellow label.
pub(super) const SOON: u32 = 0xe6c84a;
/// The ink drawn on a ticked box and a picked swatch.
pub(super) const TICK_INK: u32 = 0x0d1014;
/// .kb-list and .kb-addlist width.
pub(super) const LIST_W: f32 = 248.0;

/// --line.
pub(super) fn line() -> Hsla {
    white(0.07)
}

/// --line-strong.
pub(super) fn line_strong() -> Hsla {
    white(0.12)
}

/// --accent-dim: a focused field's border.
pub(super) fn accent_dim() -> Hsla {
    white(0.45)
}

/// --accent-glow.
pub(super) fn glow() -> Hsla {
    white(0.08)
}

/// `box-shadow: 0 0 0 2px var(--accent-glow)` around a focused field.
pub(super) fn glow_ring() -> Vec<BoxShadow> {
    vec![BoxShadow { color: glow(), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(2.0), inset: false }]
}

/// A field's box: --surface-0, a 1px border that turns --accent-dim when
/// focused (with the glow ring when `ring`). Clicks inside stay with the
/// field: the board's own mouse-down would take its focus away.
pub(super) fn field_box(focused: bool, rest: Hsla, ring: bool) -> Div {
    div()
        .bg(color(SURFACE_0))
        .border_1()
        .border_color(if focused { accent_dim() } else { rest })
        .when(focused && ring, |d| d.shadow(glow_ring()))
        .text_color(color(INK))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// .kb-add-btn: the white pill.
pub(super) fn add_btn(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(30.0))
        .px(px(14.0))
        .flex()
        .items_center()
        .rounded_full()
        .bg(color(0xffffff))
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(0x000000))
        .hover(|b| b.bg(color(0xe9e9ec)))
        .child(label)
}

/// .kb-due-clear: Clear, Unassign.
pub(super) fn quiet_btn(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .py(px(5.0))
        .px(px(9.0))
        .rounded(px(8.0))
        .text_size(px(11.0))
        .text_color(color(MUTED_2))
        .hover(|b| b.text_color(color(DANGER)).bg(tint(DANGER, 0.12)))
        .child(label)
}

/// .kb-sec-l: a section's small label.
pub(super) fn sec_label(text: &'static str) -> Div {
    div().text_size(px(11.5)).font_weight(FontWeight::MEDIUM).text_color(color(INK_3)).child(text)
}

/// The card being dragged, under the pointer.
pub(super) struct Ghost {
    pub title: SharedString,
    /// A list header rather than a card.
    pub list: bool,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let w = if self.list { LIST_W } else { LIST_W - 18.0 };
        div()
            .w(px(w))
            .py(px(if self.list { 9.0 } else { 8.0 }))
            .px(px(if self.list { 12.0 } else { 10.0 }))
            .rounded(px(if self.list { 12.0 } else { 10.0 }))
            .bg(color(if self.list { PANEL } else { RAISE }))
            .border_1()
            .border_color(accent_dim())
            .opacity(0.9)
            .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.6), offset: point(px(0.0), px(3.0)), blur_radius: px(12.0), spread_radius: px(-5.0), inset: false }])
            .font_family("Geist")
            .text_size(px(12.5))
            .line_height(relative(1.4))
            .text_color(color(INK))
            .when(self.list, |d| d.font_weight(FontWeight::SEMIBOLD).truncate())
            .child(self.title.clone())
    }
}

/// What a dragged card carries.
#[derive(Clone)]
pub(super) struct CardDrag {
    pub id: String,
    pub title: SharedString,
}

/// What a dragged list carries.
#[derive(Clone)]
pub(super) struct ListDrag {
    pub id: String,
    pub title: SharedString,
}
