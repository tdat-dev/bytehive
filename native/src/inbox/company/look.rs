//! The Company's look, from tokens.css, parts.css and office.css, exact values.
//! Honey goes on one thing only: what waits for the owner.

use gpui_kit::*;

pub(super) const GROUND: u32 = 0x0f1117;
pub(super) const PANEL: u32 = 0x151821;
pub(super) const RAISED: u32 = 0x1c202b;
pub(super) const OVER: u32 = 0x252a37;
pub(super) const INK: u32 = 0xeceae4;
pub(super) const INK_2: u32 = 0xb9bbc4;
pub(super) const INK_3: u32 = 0x9095a3;
pub(super) const HONEY: u32 = 0xf5b83d;
pub(super) const HONEY_HOVER: u32 = 0xffc85c;
pub(super) const HONEY_INK: u32 = 0x17130a;
pub(super) const WORK: u32 = 0x6fd6a0;
pub(super) const REVIEW: u32 = 0x8fb8f2;
pub(super) const STOP: u32 = 0xa7abb8;
pub(super) const DANGER: u32 = 0xf2857a;

pub(super) const S1: f32 = 4.0;
pub(super) const S2: f32 = 8.0;
pub(super) const S3: f32 = 12.0;
pub(super) const S4: f32 = 16.0;
pub(super) const S6: f32 = 24.0;
pub(super) const R_CONTROL: f32 = 6.0;
pub(super) const R_CARD: f32 = 10.0;
/// --office-bar-h, --office-rail-w (280 under 1000px wide).
pub(super) const BAR_H: f32 = 44.0;
pub(super) const RAIL_W: f32 = 340.0;
pub(super) const RAIL_W_NARROW: f32 = 280.0;

pub(super) fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// --line: rgb(236 234 228 / .1).
pub(super) fn line() -> Hsla {
    c(INK).opacity(0.1)
}

/// --line-control: rgb(236 234 228 / .38).
pub(super) fn line_control() -> Hsla {
    c(INK).opacity(0.38)
}

/// --danger-soft.
pub(super) fn danger_soft() -> Hsla {
    c(DANGER).opacity(0.12)
}

/// `inset 0 0 0 1px`.
pub(super) fn ring(color: Hsla) -> BoxShadow {
    BoxShadow { color, offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: true }
}

/// --lift: 0 8px 24px rgb(0 0 0 / .4).
pub(super) fn lift() -> BoxShadow {
    BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.4), offset: point(px(0.0), px(8.0)), blur_radius: px(24.0), spread_radius: px(0.0), inset: false }
}

/// --t-meta, --t-body, --t-row, --t-lead, --t-title, --t-page: (size, line height, weight).
pub(super) const META: (f32, f32, FontWeight) = (12.0, 16.0, FontWeight::NORMAL);
pub(super) const BODY: (f32, f32, FontWeight) = (13.0, 20.0, FontWeight::NORMAL);
pub(super) const ROW: (f32, f32, FontWeight) = (13.0, 20.0, FontWeight::MEDIUM);
pub(super) const LEAD: (f32, f32, FontWeight) = (14.0, 22.0, FontWeight::NORMAL);
pub(super) const TITLE: (f32, f32, FontWeight) = (16.0, 24.0, FontWeight::SEMIBOLD);
pub(super) const PAGE: (f32, f32, FontWeight) = (20.0, 28.0, FontWeight::SEMIBOLD);

/// parts.css .btn: Secondary and Primary, and Danger, a word in red.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Btn {
    Secondary,
    Primary,
    Danger,
}

/// A .btn (32 high), or .btn.sm (24 high). `off`: aria-disabled, at half strength.
pub(super) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: Btn, small: bool, off: bool) -> Stateful<Div> {
    let (bg, ink, hover_bg, hover_ink) = match kind {
        Btn::Secondary => (Some(RAISED), INK, Some(OVER), INK),
        Btn::Primary => (Some(HONEY), HONEY_INK, Some(HONEY_HOVER), HONEY_INK),
        Btn::Danger => (None, DANGER, Some(RAISED), DANGER),
    };
    let mut b = div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(if small { S1 } else { S2 }))
        .h(px(if small { 24.0 } else { 32.0 }))
        .px(px(if small { S2 } else { S3 }))
        .rounded(px(R_CONTROL))
        .text_size(px(ROW.0))
        .line_height(px(ROW.1))
        .font_weight(if kind == Btn::Primary { FontWeight::SEMIBOLD } else { ROW.2 })
        .text_color(c(ink))
        .whitespace_nowrap()
        .child(label.into());
    if let Some(bg) = bg {
        b = b.bg(c(bg));
    }
    if kind == Btn::Secondary {
        b = b.shadow(vec![ring(line_control())]);
    }
    if off {
        return b.opacity(0.5);
    }
    b.cursor_pointer().hover(move |s| {
        let s = s.text_color(c(hover_ink));
        match hover_bg {
            Some(h) => s.bg(c(h)),
            None => s,
        }
    })
}

/// A card: raised, a hairline, 10 and 12. A card never holds a card.
pub(super) fn card() -> Div {
    div().flex().flex_col().gap(px(S2)).p(px(S3)).rounded(px(R_CARD)).bg(c(RAISED)).shadow(vec![ring(line())])
}

/// A text box's frame: --ground, --ring-control.
pub(super) fn field() -> Div {
    div().rounded(px(R_CONTROL)).bg(c(GROUND)).shadow(vec![ring(line_control())]).px(px(S3)).py(px(S2))
}

/// A text in one of the type scale's steps.
pub(super) fn text(t: (f32, f32, FontWeight), tone: u32) -> Div {
    div().text_size(px(t.0)).line_height(px(t.1)).font_weight(t.2).text_color(c(tone))
}
