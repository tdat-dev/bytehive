//! The two pictures in the office that come from files (textures.ts): the
//! honey cell on the roof sign, drawn from design/logo.svg, and the CLI logo
//! on each badge, multiplied by white or, for a logo drawn in white, by ink.

use glam::Vec3;

use super::geom::hex;
use super::render::Renderer;
use super::render_types::TexId;

const LOGO_SIZE: u32 = 512;
/// The most a badge's logo is kept at.
const BADGE_PX: u32 = 128;
const INK: &str = "#14161c";
/// Logos drawn in white, for a dark tile: on the white card they take the ink instead.
const WHITE_LOGOS: [&str; 12] = ["amp", "cline", "copilot", "cursor", "goose", "grok", "hermes", "kilo", "kimi", "ollama", "opencode", "roo"];

/// The colour a logo is multiplied by on the white card, linear.
pub fn badge_ink(cli: &str) -> Vec3 {
    hex(if WHITE_LOGOS.contains(&cli) { INK } else { "#ffffff" })
}

/// Premultiplied pixels to straight, as a canvas hands them to a texture.
fn straight(mut rgba: Vec<u8>) -> Vec<u8> {
    for p in rgba.chunks_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    rgba
}

/// The logo at 512 px, RGBA, straight alpha; None if the file will not draw.
pub fn paint_logo() -> Option<Vec<u8>> {
    let tree = resvg::usvg::Tree::from_str(include_str!("../../../design/logo.svg"), &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(LOGO_SIZE, LOGO_SIZE)?;
    let k = LOGO_SIZE as f32 / tree.size().width();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pixmap.as_mut());
    Some(straight(pixmap.take()))
}

pub fn logo_texture(r: &mut Renderer) -> TexId {
    match paint_logo() {
        Some(px) => r.picture(px, LOGO_SIZE as usize, LOGO_SIZE as usize),
        None => r.picture(vec![0; 4], 1, 1),
    }
}

/// A CLI's badge picture and its ink. A CLI with no logo shows the bare card.
pub fn badge_texture(r: &mut Renderer, cli: &str) -> (TexId, Vec3) {
    let key = crate::brand::brand_of(cli).map(|b| b.key).unwrap_or(cli);
    // The badge is a few pixels across even close to: no more than this is ever seen.
    let picture = crate::brand::logo_png(key).and_then(|bytes| image::load_from_memory(bytes).ok()).map(|i| {
        let i = if i.width() > BADGE_PX || i.height() > BADGE_PX { i.thumbnail(BADGE_PX, BADGE_PX) } else { i };
        i.to_rgba8()
    });
    match picture {
        Some(p) => {
            let (w, h) = (p.width() as usize, p.height() as usize);
            (r.picture(p.into_raw(), w, h), badge_ink(key))
        }
        None => (r.picture(vec![0; 4], 1, 1), badge_ink(key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_draws_a_honey_cell_with_clear_corners() {
        let px = paint_logo().expect("logo.svg draws");
        let at = |x: usize, y: usize| &px[(y * 512 + x) * 4..(y * 512 + x) * 4 + 4];
        assert_eq!(at(2, 2)[3], 0, "a corner is clear");
        let side = at(100, 256);
        assert!(side[3] == 255 && side[0] > side[2], "honey on the cell's left: {side:?}");
    }

    #[test]
    fn logos_drawn_white_take_the_ink() {
        assert_eq!(badge_ink("cursor"), hex(INK));
        assert_eq!(badge_ink("claude"), Vec3::ONE);
        assert!(crate::brand::logo_png("claude").is_some_and(|b| image::load_from_memory(b).is_ok()));
    }
}
