//! The things on a room's walls and its lights (props.ts): art, a whiteboard,
//! a clock, a window out on the dusk, the cat, the hexagon rug and the
//! pendant lamps.

use std::f32::consts::PI;

use glam::Mat4;

use super::colour::{dim, linear, shift};
use super::geom::Geom;
use super::layout::{SLAB, STOREY};
use super::palette::office as o;
use super::prims::{circle, cone, plane_split, sphere_part, torus};
use super::props::{Glow, Prop, ball, bx, cyl, flat, mv, scaled, shifted, soft};
use super::seed::Seeded;

pub const ROOM_H: f32 = (STOREY - SLAB) as f32;
pub const PENDANT_Y: f32 = ROOM_H - 0.74;

pub fn wall_art(accent: &str, rand: &mut Seeded) -> Prop {
    let frame = |w: f32, h: f32, x: f32, y: f32, paintings: Vec<Geom>| {
        let mut out = vec![mv(bx(w + 0.1, h + 0.1, 0.05, "#2a2730"), x, y, 0.03), mv(flat(w, h, "#e9dfcc"), x, y, 0.06)];
        out.extend(paintings.into_iter().map(|p| mv(p, x, y, 0.065)));
        out
    };
    let blocks = |w: f32, h: f32| {
        vec![
            shifted(flat(w * 0.72, h * 0.34, accent), 0.0, -h * 0.14, 0.0),
            shifted(flat(w * 0.34, h * 0.42, "#3a4670"), -w * 0.12, h * 0.16, 0.002),
            shifted(circle(w * 0.15, 20).tint("#e0937a", None), w * 0.2, h * 0.24, 0.003),
        ]
    };
    let two = rand.next() < 0.5;
    let solid = if two {
        let mut s = frame(0.86, 1.16, -0.56, 2.24, blocks(0.86, 1.16));
        s.extend(frame(0.62, 0.62, 0.42, 2.46, vec![circle(0.2, 20).tint(accent, None)]));
        s
    } else {
        frame(1.5, 0.96, 0.0, 2.3, blocks(1.5, 0.96))
    };
    Prop { solid, ..Default::default() }
}

pub fn whiteboard(accent: &str, rand: &mut Seeded) -> Prop {
    let w = 2.5f32;
    let mut lines = Vec::new();
    for i in 0..5 {
        let len = (0.5 + rand.next() * 0.8) as f32;
        let x = (-0.5 + rand.next() * 0.24) as f32;
        lines.push(mv(flat(len, 0.035, if i % 2 == 1 { accent } else { "#3a4670" }), x, 2.6 - i as f32 * 0.2, 0.06));
    }
    let note = shift(accent, "#ffffff", 0.45);
    let box2 = |x: f32, y: f32| mv(flat(0.42, 0.26, &note), x, y, 0.06);
    let mut solid = vec![
        mv(soft(w + 0.14, 1.42, 0.06, 0.02, o::STEEL), 0.0, 2.16, 0.03),
        mv(bx(w, 1.28, 0.02, "#eef0ef"), 0.0, 2.16, 0.055),
        mv(bx(w * 0.6, 0.04, 0.09, o::STEEL), 0.0, 1.46, 0.09),
        mv(bx(0.14, 0.03, 0.03, accent), -0.2, 1.495, 0.1),
    ];
    solid.extend(lines);
    solid.push(box2(0.68, 2.5));
    solid.push(box2(0.68, 2.1));
    Prop { solid, ..Default::default() }
}

pub fn cat(hex: &str) -> Prop {
    let fur = |r: f32| ball(r, 12, 9, hex);
    let ear = || cone(0.05, 0.1, 4, 1, false).tint(hex, None);
    let tail = torus(0.2, 0.038, 6, 12, PI * 1.1).tint(hex, None).transform(Mat4::from_rotation_x(PI / 2.0));
    Prop {
        solid: vec![
            mv(scaled(fur(0.24), 1.25, 0.72, 1.0), 0.0, 0.17, 0.0),
            mv(fur(0.15), 0.26, 0.24, 0.08),
            mv(ear(), 0.2, 0.39, 0.06),
            mv(ear(), 0.33, 0.39, 0.1),
            mv(tail, -0.02, 0.06, 0.04),
        ],
        ..Default::default()
    }
}

pub fn clock() -> Prop {
    let face = cyl(0.34, 0.34, 0.06, 24, "#efe9dc").transform(Mat4::from_rotation_x(PI / 2.0));
    let ring = torus(0.34, 0.03, 8, 24, PI * 2.0).tint("#2a2730", None);
    Prop {
        solid: vec![mv(face, 0.0, 2.6, 0.04), mv(ring, 0.0, 2.6, 0.06), mv(bx(0.03, 0.22, 0.01, "#2a2730"), 0.0, 2.69, 0.08), mv(bx(0.15, 0.03, 0.01, "#2a2730"), 0.06, 2.6, 0.08)],
        ..Default::default()
    }
}

/// A window out on the dusk, lit by the sky, not by the room.
fn dusk(w: f32, h: f32, cx: f32, cy: f32) -> Vec<Geom> {
    let (top, mid, low) = (linear("#17204e"), linear("#47457f"), linear("#a8708c"));
    let glass = plane_split(w, h, 1, 6).tint("#000000", None).paint(|p, _| {
        let k = p.y as f64 / h as f64 + 0.5;
        (if k > 0.45 { mid.lerp(top, (k - 0.45) / 0.55) } else { low.lerp(mid, k / 0.45) }).as_vec3()
    });
    let glass = shifted(glass, cx, cy, 0.07);
    let mut rand = Seeded::new((w as f64 * 100.0 + cy as f64 * 10.0).round() as u32);
    let mut out = vec![glass];
    let (wd, hd) = (w as f64, h as f64);
    let mut x = -wd / 2.0;
    while x < wd / 2.0 - 0.1 {
        let bw = 0.16 + rand.next() * 0.22;
        let bh = 0.14 + rand.next() * hd * 0.3;
        let span = bw.min(wd / 2.0 - x);
        out.push(mv(flat(span as f32, bh as f32, "#1a1c40"), (cx as f64 + x + span / 2.0) as f32, (cy as f64 - hd / 2.0 + bh / 2.0) as f32, 0.073));
        x += bw;
    }
    out
}

pub fn window_bay(rand: &mut Seeded) -> Prop {
    let (w, h, y) = (2.5f32, 1.6f32, 2.14f32);
    let bar = |x: f32| mv(bx(0.05, h, 0.03, "#2a2730"), x, y, 0.09);
    let mut solid = vec![mv(soft(w + 0.18, h + 0.18, 0.08, 0.02, "#2a2730"), 0.0, y, 0.02), bar(-w / 6.0), bar(w / 6.0), mv(soft(w + 0.34, 0.08, 0.3, 0.03, o::DESK_TOP), 0.0, y - h / 2.0 - 0.1, 0.14)];
    if rand.next() < 0.6 {
        solid.push(mv(cyl(0.13, 0.1, 0.2, 12, o::POT).ramp(&dim(o::POT, 0.7), o::POT), -0.8, y - h / 2.0 + 0.03, 0.14));
        solid.push(mv(ball(0.2, 9, 7, o::PLANT), -0.8, y - h / 2.0 + 0.26, 0.14));
    }
    Prop { solid, lit: dusk(w, h, 0.0, y), ..Default::default() }
}

/// A hexagon rug, in the project's colour, under the middle of the room.
pub fn hex_rug(accent: &str, x: f32, z: f32, r: f32) -> Vec<Geom> {
    let plate = |radius: f32, y: f32, hex: &str| cyl(radius, radius, 0.02, 6, hex).moved(x, y, z, PI / 6.0);
    vec![plate(r, 0.018, &dim(accent, 0.74)), plate(r - 0.2, 0.024, &shift(accent, "#e9dfcc", 0.2)), plate(r - 0.38, 0.03, &dim(accent, 0.92))]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pendant {
    Dome,
    Globe,
    Cone,
}

/// A lamp over the room: its cord and hood, its bulb, and its glow when on.
pub fn pendant(kind: Pendant, accent: &str, x: f32, z: f32, on: bool) -> (Vec<Geom>, Vec<Geom>, Vec<Glow>) {
    let cord = mv(cyl(0.012, 0.012, ROOM_H - PENDANT_Y, 6, "#1d1f28"), x, (ROOM_H + PENDANT_Y) / 2.0, z);
    let bulb = if on { o::GLOW } else { "#39405e" };
    let mut solid = vec![cord];
    let mut lit = Vec::new();
    if kind == Pendant::Globe {
        lit.push(mv(ball(0.19, 14, 10, bulb).ramp(if on { "#ffc78f" } else { bulb }, bulb), x, PENDANT_Y, z));
    } else {
        let hood = if kind == Pendant::Dome {
            shifted(sphere_part(0.34, 16, 8, (0.0, PI * 2.0), (0.0, PI / 2.0)).tint(&dim(accent, 0.8), None), 0.0, -0.14, 0.0)
        } else {
            cone(0.32, 0.34, 16, 1, true).tint("#23252f", None)
        };
        solid.push(mv(hood, x, PENDANT_Y + 0.08, z));
        lit.push(mv(ball(0.13, 10, 8, bulb), x, PENDANT_Y - 0.1, z));
    }
    let glows = if on { vec![Glow { x, y: PENDANT_Y - 0.06, z, size: 1.5, hex: "#ffd8a0", level: 0.2 }] } else { Vec::new() };
    (solid, lit, glows)
}
