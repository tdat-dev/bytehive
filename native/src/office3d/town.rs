//! The edge of a town behind the tower (setting.ts townParts): three rows of
//! blocks, misted at the foot, the nearer the darker, with a few windows still
//! on. Built once; laid out from the viewer, so the scene turns it by FACING.

use glam::{DVec3, Vec3};

use super::colour::{hex_of, linear};
use super::geom::Geom;
use super::layout::{GROUND, SLAB};
use super::palette::office as o;
use super::props::{bx, cyl, flat, mv};
use super::seed::Seeded;

pub const PLATE_TOP: f32 = (GROUND - SLAB) as f32;
/// How far back each row stands, and where the mist ends.
pub const TOWN_NEAR: f64 = -30.0;
pub const TOWN_FAR: f64 = -74.0;
pub const TOWN_EDGE: f64 = -92.0;

struct Layer {
    z: f64,
    low: f64,
    high: f64,
    top: &'static str,
    foot: &'static str,
    lit: f64,
    glow: f64,
}

/// One row of blocks, and its lit windows into `windows`.
fn town_row(l: &Layer, rand: &mut Seeded, windows: &mut Vec<Geom>) -> Vec<Geom> {
    let mut out = Vec::new();
    let (z, pt) = (l.z as f32, PLATE_TOP as f64);
    let mut x = -260.0f64;
    while x < 260.0 {
        let w = 3.0 + rand.next() * 6.0;
        let h = l.low + (l.high - l.low) * rand.next().powf(1.8);
        let d = 5.0;
        out.push(mv(bx(w as f32, h as f32, d as f32, l.top).ramp(l.foot, l.top), x as f32, (h / 2.0 + pt) as f32, z));
        let roof = rand.next();
        if roof < 0.28 {
            let tall = 0.9 + rand.next() * 1.6;
            let at = x + (rand.next() - 0.5) * w * 0.3;
            out.push(mv(bx((w * 0.5) as f32, tall as f32, (d * 0.7) as f32, l.top), at as f32, (h + 0.4 + pt) as f32, z));
        } else if roof < 0.42 {
            let at = x + (rand.next() - 0.5) * w * 0.5;
            out.push(mv(cyl(0.5, 0.5, 0.9, 8, l.top), at as f32, (h + 0.75 + pt) as f32, z));
            out.push(mv(bx(0.8, 0.3, 0.8, l.top), x as f32, (h + 0.15 + pt) as f32, z));
        } else if roof < 0.5 {
            out.push(mv(bx(0.07, 2.4, 0.07, l.top), (x + w * 0.2) as f32, (h + 1.2 + pt) as f32, z));
        }
        let (cols, rows) = ((w / 0.8).floor() as i64, (h / 0.95).floor() as i64);
        for r in 1..rows.max(1) {
            for c in 0..cols {
                if rand.next() > l.lit {
                    continue;
                }
                let hue = if rand.next() < 0.75 { "#ffcf94" } else { "#a9c6ef" };
                let k = (0.4 + rand.next() * 0.5) * l.glow;
                // three goes through a hex string here, so the colour is rounded to 8 bits.
                let colour = hex_of(linear(hue) * k);
                windows.push(mv(flat(0.34, 0.46, &colour), (x - w / 2.0 + 0.5 + c as f64 * 0.8) as f32, (r as f64 * 0.95 + pt) as f32, (l.z + d / 2.0 + 0.03) as f32));
            }
        }
        x += w + rand.next() * 1.6;
    }
    out
}

/// The three rows, then every window, as one geometry.
pub fn town() -> Geom {
    let mut rand = Seeded::new(23);
    let mut windows = Vec::new();
    let layers = [
        Layer { z: TOWN_FAR, low: 5.0, high: 17.0, top: "#26316a", foot: o::HAZE, lit: 0.04, glow: 0.55 },
        Layer { z: (TOWN_FAR + TOWN_NEAR) / 2.0, low: 3.5, high: 11.0, top: "#1a2452", foot: "#343f7c", lit: 0.07, glow: 0.7 },
        Layer { z: TOWN_NEAR, low: 2.2, high: 6.5, top: "#111838", foot: "#26305e", lit: 0.09, glow: 0.85 },
    ];
    let mut out = Vec::new();
    for l in &layers {
        out.extend(town_row(l, &mut rand, &mut windows));
    }
    out.extend(windows);
    Geom::merged(out)
}

/// The floor of the world, into the mist at the foot of the town.
pub fn field() -> Geom {
    let (near, mid, far) = (linear("#0e1430"), linear("#161e46"), linear(o::HAZE));
    let plane = super::prims::plane_split(1000.0, 300.0, 1, 24)
        .transform(glam::Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2))
        .transform(glam::Mat4::from_translation(Vec3::new(0.0, PLATE_TOP - 0.36, (150.0 + TOWN_EDGE) as f32)));
    plane.tint("#000000", None).paint(|p, _| {
        let k = (20.0 - p.z as f64) / (20.0 - TOWN_EDGE);
        let c: DVec3 = if k < 0.0 { near } else { near.lerp(mid, (k * 1.6).min(1.0)).lerp(far, (k - 0.6).max(0.0) / 0.4) };
        c.as_vec3()
    })
}
