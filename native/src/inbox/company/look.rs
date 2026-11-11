//! The Company's look: the Inbox's own (inbox/mod.rs, style.rs), so the page
//! reads as the same app. Near-black ground, panels rounded 18 on a hairline,
//! pill buttons, white for the one action, amber only for what waits on you.

use gpui_kit::*;

pub(super) const GROUND: u32 = 0x050506;
pub(super) const PANEL: u32 = 0x0e0f11;
pub(super) const RAISED: u32 = 0x15161a;
pub(super) const OVER: u32 = 0x1d1e23;
pub(super) const INK: u32 = 0xf4f4f5;
pub(super) const INK_2: u32 = 0xb4b4bc;
pub(super) const INK_3: u32 = 0x8c8c96;
/// The Inbox's --ib-needs.
pub(super) const HONEY: u32 = 0xf0bf6a;
pub(super) const WORK: u32 = 0x6fd6a0;
pub(super) const REVIEW: u32 = 0x9ab8ee;
pub(super) const STOP: u32 = 0x8c8c96;
pub(super) const DANGER: u32 = 0xf2766b;

pub(super) const S1: f32 = 4.0;
pub(super) const S2: f32 = 8.0;
pub(super) const S3: f32 = 12.0;
pub(super) const S4: f32 = 16.0;
pub(super) const S6: f32 = 24.0;
/// A row or a small box, as the Inbox's rows (12) and pills' insides.
pub(super) const R_CONTROL: f32 = 10.0;
pub(super) const R_CARD: f32 = 14.0;
/// The Inbox's panels.
pub(super) const R_PANEL: f32 = 18.0;
/// How much of a card's foot may be out of sight before it counts as below: its padding and half a button.
pub(super) const CUT: f32 = 16.0;
/// The rail's width (280 under 1000px wide).
pub(super) const RAIL_W: f32 = 340.0;
pub(super) const RAIL_W_NARROW: f32 = 280.0;

pub(super) fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// The Inbox's hairline.
pub(super) fn line() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.07)
}

/// A text box's ring, as the Inbox draws them.
pub(super) fn line_control() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.12)
}

/// White at some strength: the Inbox's quiet fills.
pub(super) fn white_at(alpha: f32) -> Hsla {
    hsla(0.0, 0.0, 1.0, alpha)
}

/// --danger-soft.
pub(super) fn danger_soft() -> Hsla {
    c(DANGER).opacity(0.12)
}

/// `inset 0 0 0 1px`.
pub(super) fn ring(color: Hsla) -> BoxShadow {
    BoxShadow { color, offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: true }
}

/// The Inbox's --ib-float: what a floating piece stands on.
pub(super) fn lift() -> BoxShadow {
    BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(18.0)), blur_radius: px(40.0), spread_radius: px(0.0), inset: false }
}

/// --t-meta, --t-body, --t-row, --t-lead, --t-title, --t-page: (size, line height, weight).
pub(super) const META: (f32, f32, FontWeight) = (12.0, 16.0, FontWeight::NORMAL);
pub(super) const BODY: (f32, f32, FontWeight) = (13.0, 20.0, FontWeight::NORMAL);
pub(super) const ROW: (f32, f32, FontWeight) = (13.0, 20.0, FontWeight::MEDIUM);
pub(super) const LEAD: (f32, f32, FontWeight) = (14.0, 22.0, FontWeight::NORMAL);
pub(super) const TITLE: (f32, f32, FontWeight) = (16.0, 24.0, FontWeight::SEMIBOLD);
pub(super) const PAGE: (f32, f32, FontWeight) = (20.0, 28.0, FontWeight::SEMIBOLD);

/// The Inbox's pills: Secondary on a quiet fill, Primary white (New agent,
/// Start), and Danger, a word in red.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Btn {
    Secondary,
    Primary,
    Danger,
}

/// A pill 32 high, or 24 when small. `off`: disabled, at half strength.
pub(super) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: Btn, small: bool, off: bool) -> Stateful<Div> {
    let (bg, ink, hover_bg, hover_ink) = match kind {
        Btn::Secondary => (Some(white_at(0.06)), c(INK), Some(white_at(0.12)), c(INK)),
        Btn::Primary => (Some(c(0xffffff)), c(0x000000), Some(c(0xe9e9ec)), c(0x000000)),
        Btn::Danger => (None, c(DANGER), Some(white_at(0.06)), c(DANGER)),
    };
    let mut b = div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(if small { S1 } else { S2 }))
        .h(px(if small { 24.0 } else { 32.0 }))
        .px(px(if small { 10.0 } else { 14.0 }))
        .rounded_full()
        .text_size(px(if small { 12.0 } else { 12.5 }))
        .line_height(px(ROW.1))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .whitespace_nowrap()
        .child(label.into());
    if let Some(bg) = bg {
        b = b.bg(bg);
    }
    if off {
        return b.opacity(0.5);
    }
    b.cursor_pointer().hover(move |s| {
        let s = s.text_color(hover_ink);
        match hover_bg {
            Some(h) => s.bg(h),
            None => s,
        }
    })
}

/// A card: raised on a hairline, as the Inbox's current row. A card never holds a card.
pub(super) fn card() -> Div {
    div().flex().flex_col().gap(px(S2)).p(px(S3)).rounded(px(R_CARD)).bg(c(RAISED)).shadow(vec![ring(line())])
}

/// A text box's frame, as the Home composer's. The textarea inside brings 8 of
/// the 12 the box's text sits in from its side.
pub(super) fn field() -> Div {
    div().rounded(px(12.0)).bg(c(GROUND)).shadow(vec![ring(line_control())]).px(px(S1)).py(px(S2))
}

/// A text in one of the type scale's steps.
pub(super) fn text(t: (f32, f32, FontWeight), tone: u32) -> Div {
    div().text_size(px(t.0)).line_height(px(t.1)).font_weight(t.2).text_color(c(tone))
}
