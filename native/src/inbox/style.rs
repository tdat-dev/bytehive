//! Colours, rings, shadows, icons and tooltips every part of the Inbox draws with (tokens.css, inbox.css).

use super::*;

pub(super) fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub(super) fn white(alpha: f32) -> Hsla {
    hsla(0.0, 0.0, 1.0, alpha)
}

pub(super) fn tint(hex: u32, alpha: f32) -> Hsla {
    color(hex).opacity(alpha)
}

pub(super) fn hairline() -> Hsla {
    white(0.07)
}

/// `color-mix(in oklch, brand 22%, #111114)`, close enough in sRGB.
pub(super) fn tile(hex: u32) -> Hsla {
    let mix = |shift: u32| ((hex >> shift & 0xff) as f32 * 0.22 + (0x11 as f32) * 0.78) as u32;
    color((mix(16) << 16) | (mix(8) << 8) | mix(0))
}

/// A native tooltip, today's title="".
pub(super) fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text: SharedString = text.into();
    move |window, cx| Tooltip::new(text.clone()).build(window, cx)
}

pub(super) fn haze() -> std::sync::Arc<Image> {
    static HAZE: std::sync::OnceLock<std::sync::Arc<Image>> = std::sync::OnceLock::new();
    HAZE.get_or_init(|| std::sync::Arc::new(Image::from_bytes(ImageFormat::Png, include_bytes!("../../assets/haze.png").to_vec()))).clone()
}

/// `inset 0 0 0 1px`: a ring that takes no room, so rows never shift.
pub(super) fn ring(c: Hsla) -> BoxShadow {
    BoxShadow { color: c, offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: true }
}

/// `--ib-float`: the shadow every floating dock piece stands on.
pub(super) fn float() -> Vec<BoxShadow> {
    vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(18.0)), blur_radius: px(40.0), spread_radius: px(0.0), inset: false }, ring(white(0.08))]
}

pub(super) fn icon(name: &'static str, size: f32, tone: Hsla) -> Svg {
    svg().path(format!("icons/{name}.svg")).size(px(size)).flex_none().text_color(tone)
}

pub(super) fn mono() -> Div {
    div().font_family("Geist Mono")
}
