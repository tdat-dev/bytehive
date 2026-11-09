//! The pictures Home draws: the ByteHive logo (hivelogo.ts, still, its comb
//! cells dark), its glow and the page's haze, baked since GPUI draws no
//! radial gradients (tools/bake_home.py).

use std::sync::{Arc, OnceLock};

use gpui_kit::{Image, ImageFormat};

const HEX: &str = "M12 .8 21.699 6.4V17.6L12 23.2 2.301 17.6V6.4Z";

/// hiveLogo(): honey hexagon, faint comb, the ›_ cut out. Drawn at twice
/// the size it shows so it stays sharp.
fn logo_svg() -> String {
    format!(
        concat!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="168" height="168" viewBox="0 0 24 24" fill="none"><defs>"##,
            r##"<linearGradient id="h" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffd85c"/><stop offset=".5" stop-color="#f5a524"/><stop offset="1" stop-color="#c86a12"/></linearGradient>"##,
            r##"<pattern id="c" width="4.5033" height="7.8" patternUnits="userSpaceOnUse" x="9.748" y="9.4"><path d="M2.2517 0 4.5033 1.3V3.9L2.2517 5.2 0 3.9V1.3ZM2.2517 5.2V7.8" fill="none" stroke="#0e0f12" stroke-opacity=".22" stroke-width=".32"/></pattern>"##,
            r##"<mask id="p" maskUnits="userSpaceOnUse" x="0" y="0" width="24" height="24"><rect width="24" height="24" fill="#fff"/><g fill="none" stroke="#000" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"><path d="M8 8.8 11.39 12.19 8 15.58"/><path d="M13.65 15.58H17.49"/></g></mask></defs>"##,
            r##"<g mask="url(#p)"><path d="{hex}" fill="url(#h)"/><path d="{hex}" fill="url(#c)"/></g></svg>"##
        ),
        hex = HEX
    )
}

fn cached(cell: &'static OnceLock<Arc<Image>>, make: impl FnOnce() -> Image) -> Arc<Image> {
    cell.get_or_init(|| Arc::new(make())).clone()
}

pub(super) fn logo() -> Arc<Image> {
    static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
    cached(&LOGO, || Image::from_bytes(ImageFormat::Svg, logo_svg().into_bytes()))
}

/// .logo-glow::before: a 150px amber circle at opacity .19.
pub(super) fn glow() -> Arc<Image> {
    static GLOW: OnceLock<Arc<Image>> = OnceLock::new();
    cached(&GLOW, || Image::from_bytes(ImageFormat::Png, include_bytes!("../../assets/logo_glow.png").to_vec()))
}

/// .home: radial-gradient(900px 420px at 50% -8%, warm .09, transparent 62%).
pub(super) fn home_haze() -> Arc<Image> {
    static HAZE: OnceLock<Arc<Image>> = OnceLock::new();
    cached(&HAZE, || Image::from_bytes(ImageFormat::Png, include_bytes!("../../assets/home_haze.png").to_vec()))
}
