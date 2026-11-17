//! Where the tower stands (setting.ts): a small plot at the blue hour, with a
//! lawn, a kerb, stepping stones, trees, a bench and two street lamps, and the
//! rooms' light spilt on the ground in front. The sky and the town behind are
//! in sky.rs and town.rs; `FACING` turns those, and the field, to the viewer.

use std::f32::consts::PI;

use glam::Vec3;

use super::colour::linear;
use super::geom::Geom;
use super::glow::{flare, pool};
use super::layout::{LIFT_W, PLOT, ROOM_D, ROOM_W, VIEW};
use super::palette::office as o;
use super::props::{ball, bx, cyl, mv, soft};
use super::seed::Seeded;
use super::slab::rounded_slab;
use super::town::PLATE_TOP;

/// Everything far is laid out from the viewer: +x the screen's right, -z away.
pub fn facing() -> f32 {
    VIEW.x.atan2(VIEW.z) as f32
}

const STREET_LAMP_H: f32 = 3.5;
const LAMPS: [(f32, f32); 2] = [(9.8, -0.8), (-6.6, 6.4)];
const TREES: [(f32, f32, f32); 6] = [(-8.0, -3.4, 6.2), (-7.8, 2.2, 4.8), (-5.6, -6.2, 5.4), (9.2, -5.2, 6.0), (7.6, -6.6, 4.8), (-7.9, 6.5, 3.8)];
/// Stones from the edge of the plot to the foot of the lift.
const PATH: [(f32, f32); 6] = [(7.1, 4.6), (7.7, 5.3), (8.3, 5.9), (8.9, 6.5), (9.5, 7.1), (10.2, 7.8)];

/// A tree of a few round clumps. Cold on the side of the sky, warm on the side of the tower.
pub fn tree(rand: &mut Seeded, toward: (f64, f64), h: f32) -> Geom {
    let (leaf, deep, warm) = (linear(o::LEAF), linear(o::LEAF_DEEP), linear("#a98a4a"));
    let lit = |g: Geom, ox: f32, oy: f32, oz: f32| {
        let g = g.paint(|_, n| {
            let up = n.y as f64 * 0.5 + 0.5;
            let to = (n.x as f64 * toward.0 + n.z as f64 * toward.1).max(0.0);
            deep.lerp(leaf, up).lerp(warm, to * to * 0.42 * (1.0 - up * 0.6)).as_vec3()
        });
        super::props::shifted(g, ox, oy, oz)
    };
    let blob = |r: f32| ball(r, 12, 9, o::LEAF);
    let r = h * 0.34;
    let trunk = mv(cyl(0.16, 0.26, h * 0.6, 8, o::BARK).ramp("#2a2228", o::BARK), 0.0, h * 0.3, 0.0);
    let first = lit(blob(r), 0.0, h * 0.72, 0.0);
    let reach = r * (0.5 + rand.next() as f32 * 0.3);
    Geom::merged([trunk, first, lit(blob(r * 0.78), reach, h * 0.6, r * 0.3), lit(blob(r * 0.74), -r * 0.6, h * 0.6, -r * 0.2), lit(blob(r * 0.64), r * 0.1, h * 0.92, r * 0.2)])
}

pub fn bench() -> Geom {
    let slat = |y: f32, z: f32| mv(soft(1.7, 0.07, 0.15, 0.03, o::DESK_TOP), 0.0, y, z);
    let leg = |x: f32| mv(bx(0.09, 0.46, 0.5, o::DESK_BODY), x, 0.23, 0.0);
    Geom::merged([
        slat(0.47, -0.17),
        slat(0.47, 0.0),
        slat(0.47, 0.17),
        slat(0.8, -0.27),
        slat(0.64, -0.27),
        leg(-0.7),
        leg(0.7),
        mv(bx(0.09, 0.45, 0.09, o::DESK_BODY), -0.7, 0.72, -0.27),
        mv(bx(0.09, 0.45, 0.09, o::DESK_BODY), 0.7, 0.72, -0.27),
    ])
}

pub fn street_lamp() -> Geom {
    Geom::merged([
        mv(cyl(0.2, 0.26, 0.24, 10, o::DESK_BODY), 0.0, 0.12, 0.0),
        mv(cyl(0.06, 0.09, STREET_LAMP_H, 8, o::DESK_BODY), 0.0, STREET_LAMP_H / 2.0, 0.0),
        mv(cyl(0.24, 0.06, 0.1, 10, o::DESK_BODY), 0.0, STREET_LAMP_H + 0.34, 0.0),
    ])
}

/// A low bush of three clumps, lit from above.
pub fn shrub(rand: &mut Seeded, r: f32) -> Geom {
    let (up, down) = (linear(o::GRASS_LIT), linear(o::LEAF_DEEP));
    let blob = |k: f32| ball(r * k, 10, 7, o::LEAF).paint(|_, n| down.lerp(up, n.y as f64 * 0.5 + 0.5).as_vec3());
    let reach = r * (0.5 + rand.next() as f32 * 0.4);
    Geom::merged([mv(blob(1.0), 0.0, r * 0.7, 0.0), mv(blob(0.74), reach, r * 0.5, r * 0.3), mv(blob(0.66), -r * 0.6, r * 0.45, -r * 0.2)])
}

/// The plot: lawn, kerb, stones, trees and a bench, as one geometry that takes the shadows.
pub fn plot() -> Geom {
    let mut rand = Seeded::new(41);
    let (w, d, px, pz) = PLOT;
    let mut parts = vec![
        mv(rounded_slab(w, d, 0.55, 1.6, None).tint(o::KERB, Some(o::GRASS)), px as f32, PLATE_TOP - 0.08, pz as f32),
        mv(rounded_slab(ROOM_W + LIFT_W + 2.6, ROOM_D + 2.6, 0.42, 0.5, None).tint(o::SHELL_EDGE, Some(o::STONE)), (LIFT_W / 2.0) as f32, PLATE_TOP, 0.0),
    ];
    for (x, z, h) in TREES {
        let l = (x as f64).hypot(z as f64);
        parts.push(mv(tree(&mut rand, (-x as f64 / l, -z as f64 / l), h), x, PLATE_TOP - 0.08, z));
    }
    for (x, z) in PATH {
        let r = 0.42 + rand.next() as f32 * 0.08;
        parts.push(mv(cyl(r, r, 0.08, 6, o::STONE).moved(0.0, 0.0, 0.0, 0.5), x, PLATE_TOP - 0.05, z));
    }
    parts.push(bench().moved(9.9, PLATE_TOP - 0.08, 3.0, -PI / 2.0));
    for (x, z) in LAMPS {
        parts.push(mv(street_lamp(), x, PLATE_TOP - 0.08, z));
    }
    for i in 0..9 {
        // In the order JavaScript reads the arguments: the size, the bush, then x and z.
        let r = 0.34 + rand.next() as f32 * 0.24;
        let bush = shrub(&mut rand, r);
        let x = -5.2 + i as f32 * 1.04 + rand.next() as f32 * 0.2;
        let z = (ROOM_D / 2.0) as f32 + 1.95 + rand.next() as f32 * 0.3;
        parts.push(mv(bush, x, PLATE_TOP - 0.08, z));
    }
    for i in 0..3 {
        parts.push(mv(shrub(&mut rand, 0.44), 8.5 + i as f32 * 0.7, PLATE_TOP - 0.08, 0.6 + i as f32 * 1.1));
    }
    Geom::merged(parts)
}

/// The street lamps' light: a glow round each globe and a pool under it.
pub fn lamp_glows() -> Geom {
    Geom::merged(LAMPS.iter().flat_map(|&(x, z)| [flare(x, PLATE_TOP + STREET_LAMP_H + 0.5, z, 3.4, "#ffe2c0", 0.4), pool(x, PLATE_TOP - 0.02, z, 8.0, 8.0, "#ffd9b0", 0.26)]))
}

/// The lamps' globes.
pub fn lamp_globes() -> Geom {
    Geom::merged(LAMPS.iter().map(|&(x, z)| mv(ball(0.24, 12, 9, o::GLOW), x, PLATE_TOP + STREET_LAMP_H + 0.5, z)))
}

/// The rooms' light on the ground in front, and on the lawn round it. Off when no room is lit.
pub fn spill() -> Geom {
    let (lw, rw, rd) = (LIFT_W as f32, ROOM_W as f32, ROOM_D as f32);
    Geom::merged([pool(lw / 2.0, PLATE_TOP + 0.01, rd / 2.0 + 2.0, rw + lw + 5.0, 6.4, "#ffb066", 0.34), pool(rw / 2.0 + lw + 2.0, PLATE_TOP + 0.01, 0.0, 6.0, rd + 4.0, "#ffb066", 0.24)])
}

/// Where a street lamp's globe hangs, for the renderer's own light.
pub fn lamp_globe_spots() -> [Vec3; 2] {
    LAMPS.map(|(x, z)| Vec3::new(x, PLATE_TOP + STREET_LAMP_H + 0.5, z))
}
