//! The sky behind the town (setting.ts): a painted sheet, as behind a model
//! photographed on a table, the stars over it and the moon low on the left.
//! The sheet's picture is painted here into pixels, as the office paints it on
//! a canvas: the dusk in bands, and long clouds blurred soft.

use glam::{DVec3, Vec3};

use super::colour::linear;
use super::geom::Geom;
use super::glow::flare;
use super::palette::office as o;
use super::prims::{circle, plane};
use super::props::mv;
use super::seed::Seeded;

/// The sheet: how far back, how wide, and its bottom and top.
pub const SKY_Z: f32 = -330.0;
pub const SKY_W: f32 = 1000.0;
pub const SKY_Y0: f32 = -92.0;
pub const SKY_Y1: f32 = 72.0;
/// Where the sky's painted light sits, as a share of the sheet from the bottom.
const SKY_GLOW: f64 = 0.37;
const MOON: (f32, f32, f32) = (-50.0, 5.0, 1.7);

/// The sheet, to carry the painted picture (u across, v up from its bottom).
pub fn sky_sheet() -> Geom {
    mv(plane(SKY_W, SKY_Y1 - SKY_Y0).tint("#ffffff", None), 0.0, (SKY_Y0 + SKY_Y1) / 2.0, SKY_Z)
}

/// A star: where, and its colour, linear.
pub struct Star {
    pub at: Vec3,
    pub colour: Vec3,
}

/// 340 stars, fewer and fainter toward the glow low down.
pub fn star_field() -> Vec<Star> {
    let mut rand = Seeded::new(5);
    let (warm, cold) = (linear("#ffe9c9"), linear("#cdd9ff"));
    (0..340)
        .map(|_| {
            let height = rand.next().powf(0.7);
            let x = (rand.next() - 0.5) * SKY_W as f64 * 0.7;
            let at = Vec3::new(x as f32, (-20.0 + height * 92.0) as f32, SKY_Z + 3.0);
            let base = if rand.next() < 0.3 { warm } else { cold };
            let colour: DVec3 = base * ((0.25 + rand.next().powi(3) * 0.75) * (height * 2.2).min(1.0));
            Star { at, colour: colour.as_vec3() }
        })
        .collect()
}

/// The moon, with two darker patches, set in the sky.
pub fn moon() -> Geom {
    let r = MOON.2;
    let face = circle(r, 40).tint(o::MOON, None);
    let shade = mv(circle(r * 0.24, 16).tint("#c9c4b8", None), -r * 0.26, r * 0.22, 0.05);
    let shade2 = mv(circle(r * 0.15, 14).tint("#c9c4b8", None), r * 0.3, -r * 0.33, 0.05);
    mv(Geom::merged([face, shade, shade2]), MOON.0, MOON.1, SKY_Z + 5.0)
}

/// The moon's light on the air round it.
pub fn moon_halo() -> Geom {
    flare(MOON.0, MOON.1, SKY_Z + 4.0, 16.0, "#7f92d8", 0.2)
}

fn srgb(hex: &str) -> [f64; 3] {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    [((n >> 16) & 0xff) as f64, ((n >> 8) & 0xff) as f64, (n & 0xff) as f64]
}

/// The canvas gradient: stops in sRGB, mixed in sRGB, as a 2D canvas does.
fn gradient(stops: &[(f64, [f64; 3])], k: f64) -> [f64; 3] {
    let k = k.clamp(0.0, 1.0);
    let mut prev = stops[0];
    for &s in stops {
        if k <= s.0 {
            let span = s.0 - prev.0;
            let t = if span <= 0.0 { 1.0 } else { (k - prev.0) / span };
            return [0, 1, 2].map(|i| prev.1[i] + (s.1[i] - prev.1[i]) * t);
        }
        prev = s;
    }
    prev.1
}

/// The sky's picture, `w` by `h`, RGBA in sRGB, top row first.
pub fn paint_sky(w: usize, h: usize) -> Vec<u8> {
    let glow = 1.0 - SKY_GLOW;
    let stops = [(0.0, srgb(o::SKY_TOP)), (glow - 0.42, srgb(o::SKY_HIGH)), (glow - 0.2, srgb(o::SKY_MID)), (glow - 0.1, srgb(o::SKY_LOW)), (glow, srgb(o::SKY_GLOW)), (1.0, srgb(o::SKY_GLOW))];
    let mut px: Vec<[f64; 3]> = Vec::with_capacity(w * h);
    for y in 0..h {
        let c = gradient(&stops, (y as f64 + 0.5) / h as f64);
        px.extend(std::iter::repeat_n(c, w));
    }
    // Long clouds holding what light is left. Lit from under, so warmer the lower they lie.
    let mut rand = Seeded::new(11);
    let (wf, hf) = (w as f64, h as f64);
    let blur = hf * 0.012;
    for _ in 0..26 {
        let cy = hf * (glow - 0.02 - rand.next() * 0.34);
        let low = 1.0 - (glow - cy / hf) / 0.36;
        let (colour, alpha) = if low > 0.6 { ([190.0, 130.0, 150.0], 0.1) } else { ([100.0, 120.0, 190.0], 0.08) };
        let cx = rand.next() * wf;
        let rx = wf * (0.04 + rand.next() * 0.09);
        let ry = hf * (0.004 + rand.next() * 0.012);
        cloud(&mut px, w, h, (cx, cy, rx, ry), blur, colour, alpha);
    }
    px.iter().flat_map(|c| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8, 255]).collect()
}

/// One ellipse, its edge blurred by a gaussian of `sigma`, laid over the picture.
fn cloud(px: &mut [[f64; 3]], w: usize, h: usize, e: (f64, f64, f64, f64), sigma: f64, colour: [f64; 3], alpha: f64) {
    let (cx, cy, rx, ry) = e;
    let pad = (sigma * 3.0).ceil();
    let (x0, x1) = (((cx - rx - pad).floor().max(0.0)) as usize, ((cx + rx + pad).ceil().min(w as f64)) as usize);
    let (y0, y1) = (((cy - ry - pad).floor().max(0.0)) as usize, ((cy + ry + pad).ceil().min(h as f64)) as usize);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let (bw, bh) = (x1 - x0, y1 - y0);
    // Coverage, four samples a pixel, then the blur one way and the other.
    let mut cover = vec![0.0f64; bw * bh];
    for j in 0..bh {
        for i in 0..bw {
            let mut n = 0.0;
            for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let dx = ((x0 + i) as f64 + sx - cx) / rx;
                let dy = ((y0 + j) as f64 + sy - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    n += 0.25;
                }
            }
            cover[j * bw + i] = n;
        }
    }
    let radius = pad as isize;
    let kernel: Vec<f64> = (-radius..=radius).map(|d| (-(d * d) as f64 / (2.0 * sigma * sigma)).exp()).collect();
    let total: f64 = kernel.iter().sum();
    let pass = |src: &[f64], along_x: bool| -> Vec<f64> {
        let mut dst = vec![0.0; bw * bh];
        for j in 0..bh {
            for i in 0..bw {
                let mut s = 0.0;
                for (t, k) in kernel.iter().enumerate() {
                    let d = t as isize - radius;
                    let (ii, jj) = if along_x { (i as isize + d, j as isize) } else { (i as isize, j as isize + d) };
                    if ii >= 0 && jj >= 0 && (ii as usize) < bw && (jj as usize) < bh {
                        s += k * src[jj as usize * bw + ii as usize];
                    }
                }
                dst[j * bw + i] = s / total;
            }
        }
        dst
    };
    let soft = pass(&pass(&cover, true), false);
    for j in 0..bh {
        for i in 0..bw {
            let a = soft[j * bw + i] * alpha;
            let p = &mut px[(y0 + j) * w + x0 + i];
            for c in 0..3 {
                p[c] += (colour[c] - p[c]) * a;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sky_runs_from_night_to_the_glow() {
        let (w, h) = (64, 32);
        let px = paint_sky(w, h);
        assert_eq!(px.len(), w * h * 4);
        let at = |y: usize| [px[y * w * 4] as f64, px[y * w * 4 + 1] as f64, px[y * w * 4 + 2] as f64];
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() <= 3.0);
        assert!(near(at(0), srgb(o::SKY_TOP)), "top {:?}", at(0));
        assert!(near(at(h - 1), srgb(o::SKY_GLOW)), "bottom {:?}", at(h - 1));
        assert_eq!(star_field().len(), 340);
    }

    /// Writes the sky to BYTEHIVE_SKY_SHOT (a .jpg) to look at by eye.
    #[test]
    #[ignore]
    fn shoot_the_sky() {
        let Ok(path) = std::env::var("BYTEHIVE_SKY_SHOT") else { return };
        let (w, h) = (1024, 512);
        let rgb: Vec<u8> = paint_sky(w, h).chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        image::save_buffer(path, &rgb, w as u32, h as u32, image::ColorType::Rgb8).unwrap();
    }
}
