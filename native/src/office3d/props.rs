//! What makes a floor somebody's room (props.ts): the things that stand on the
//! floor, each built facing +z with its back at z = 0. Plain geometry with its
//! colours on the vertices, so a floor of it is still one draw. Wall things and
//! lamps are in props_wall.rs; how a room is put together is in furnish.rs.

use glam::{Mat4, Vec3};

use super::colour::{dim, linear, shift};
use super::geom::Geom;
use super::light::{Lamp, rgb};
use super::palette::{ACCENTS, office as o};
use super::prims::{box_geom, cone, cylinder, plane, rounded_box, sphere};
use super::seed::Seeded;

pub const WARM_LAMP: &str = "#ffe0bd";

/// A glow round a light, turned to the viewer when the room is put together.
#[derive(Clone, Debug)]
pub struct Glow {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub size: f32,
    pub hex: &'static str,
    pub level: f32,
}

/// One thing in a room, `foot` its (w, d) on the floor where light does not get under.
#[derive(Default)]
pub struct Prop {
    pub solid: Vec<Geom>,
    pub lit: Vec<Geom>,
    pub glows: Vec<Glow>,
    pub lamps: Vec<Lamp>,
    pub foot: Option<(f64, f64)>,
}

/* ---- the office's shape helpers (geometry.ts) ---- */

pub fn mv(g: Geom, x: f32, y: f32, z: f32) -> Geom {
    g.moved(x, y, z, 0.0)
}

pub fn bx(w: f32, h: f32, d: f32, hex: &str) -> Geom {
    box_geom(w, h, d).tint(hex, None)
}

pub fn soft(w: f32, h: f32, d: f32, r: f32, hex: &str) -> Geom {
    rounded_box(w, h, d, 1, r).tint(hex, None)
}

pub fn cyl(top: f32, bottom: f32, h: f32, radial: u32, hex: &str) -> Geom {
    cylinder(top, bottom, h, radial, 1, false).tint(hex, None)
}

pub fn ball(r: f32, ws: u32, hs: u32, hex: &str) -> Geom {
    sphere(r, ws, hs).tint(hex, None)
}

pub fn flat(w: f32, h: f32, hex: &str) -> Geom {
    plane(w, h).tint(hex, None)
}

pub fn scaled(g: Geom, x: f32, y: f32, z: f32) -> Geom {
    g.transform(Mat4::from_scale(Vec3::new(x, y, z)))
}

pub fn shifted(g: Geom, x: f32, y: f32, z: f32) -> Geom {
    g.transform(Mat4::from_translation(Vec3::new(x, y, z)))
}

/* ---- the things ---- */

pub fn sofa(accent: &str) -> Prop {
    let cloth = dim(accent, 0.72);
    let light = shift(accent, "#f2e4cf", 0.18);
    let cushion = |x: f32| mv(soft(1.02, 0.2, 0.74, 0.09, &light), x, 0.52, 0.56);
    Prop {
        solid: vec![
            mv(soft(2.5, 0.34, 0.92, 0.12, &cloth), 0.0, 0.3, 0.52),
            mv(soft(2.5, 0.78, 0.3, 0.13, &cloth), 0.0, 0.76, 0.17),
            mv(soft(0.3, 0.56, 0.96, 0.13, &cloth), -1.2, 0.46, 0.5),
            mv(soft(0.3, 0.56, 0.96, 0.13, &cloth), 1.2, 0.46, 0.5),
            cushion(-0.52),
            cushion(0.52),
            mv(soft(0.46, 0.42, 0.16, 0.08, "#ead9bd"), -0.72, 0.8, 0.42),
            mv(soft(0.42, 0.4, 0.15, 0.08, &dim(ACCENTS[0], 1.0)), 0.76, 0.8, 0.42),
            mv(bx(0.09, 0.14, 0.09, o::DESK_BODY), -1.15, 0.07, 0.86),
            mv(bx(0.09, 0.14, 0.09, o::DESK_BODY), 1.15, 0.07, 0.86),
        ],
        foot: Some((2.6, 1.0)),
        ..Default::default()
    }
}

pub fn coffee_counter() -> Prop {
    let my = 0.96;
    let mut solid = vec![
        mv(soft(2.5, 0.92, 0.68, 0.05, o::DESK_BODY), 0.0, 0.46, 0.36),
        mv(soft(2.6, 0.07, 0.76, 0.03, o::DESK_TOP), 0.0, 0.96, 0.38),
        mv(bx(0.02, 0.6, 0.02, o::STEEL), 0.42, 0.5, 0.71),
        mv(bx(0.02, 0.6, 0.02, o::STEEL), -0.42, 0.5, 0.71),
        mv(soft(0.56, 0.66, 0.46, 0.06, "#c7ccd6"), -0.62, my + 0.36, 0.3),
        mv(soft(0.42, 0.1, 0.3, 0.03, o::DESK_BODY), -0.62, my + 0.1, 0.42),
        mv(cyl(0.075, 0.06, 0.13, 12, o::PAPER), -0.62, my + 0.22, 0.46),
    ];
    for (i, x) in [0.16, 0.42, 0.68].into_iter().enumerate() {
        solid.push(mv(cyl(0.075, 0.065, 0.14, 12, [o::PAPER, "#c4605a", "#6d8bc0"][i]), x, my + 0.1, 0.48));
    }
    solid.push(mv(cyl(0.12, 0.12, 0.34, 12, "#6b4a34"), 0.98, my + 0.2, 0.3));
    // A shelf over it, for jars.
    solid.push(mv(bx(2.3, 0.07, 0.34, o::DESK_TOP), 0.0, 2.36, 0.17));
    for (i, x) in [-0.86, -0.46, -0.08, 0.34, 0.76].into_iter().enumerate() {
        let tall = (i % 2) as f32;
        solid.push(mv(cyl(0.13, 0.13, 0.3 + tall * 0.1, 12, ["#d7b58a", "#8f5c38", "#e9dfcc", "#7fa58f", "#b98d58"][i]), x, 2.56 + tall * 0.05, 0.18));
    }
    Prop { solid, lit: vec![mv(flat(0.28, 0.1, "#7fe0b8"), -0.62, my + 0.52, 0.536)], foot: Some((2.6, 0.84)), ..Default::default() }
}

const BOOK_COLOURS: [&str; 10] = ["#c4605a", "#3f8f8a", "#d7b58a", "#4f7fc0", "#e9dfcc", "#7a5fa8", "#5d8a4e", "#a8664a", "#2f3a5e", "#c9a7a0"];

fn shelf_row(rand: &mut Seeded, width: f64, y: f64, depth: f64) -> Vec<Geom> {
    let mut out = Vec::new();
    let mut x = -width / 2.0 + 0.05;
    let end = width / 2.0 - 0.06;
    while x < end - 0.1 {
        if rand.next() < 0.16 {
            x += 0.16 + rand.next() * 0.22;
            continue;
        }
        let t = 0.06 + rand.next() * 0.08;
        let h = 0.34 + rand.next() * 0.2;
        if x + t > end {
            break;
        }
        let d = depth * (0.74 + rand.next() * 0.2);
        let colour = BOOK_COLOURS[(rand.next() * BOOK_COLOURS.len() as f64).floor() as usize];
        out.push(mv(bx(t as f32, h as f32, d as f32, colour), (x + t / 2.0) as f32, (y + h / 2.0) as f32, (depth / 2.0 + 0.02) as f32));
        x += t + 0.008;
    }
    out
}

pub fn bookshelf(rand: &mut Seeded, w: f64, h: f64, rows: u32) -> Prop {
    let d = 0.42;
    let wood = "#4a3428";
    let (wf, hf, df) = (w as f32, h as f32, d as f32);
    let mut parts = vec![mv(bx(wf, hf, 0.05, &dim(wood, 0.7)), 0.0, hf / 2.0, 0.03), mv(bx(0.07, hf, df, wood), -wf / 2.0, hf / 2.0, df / 2.0), mv(bx(0.07, hf, df, wood), wf / 2.0, hf / 2.0, df / 2.0)];
    for i in 0..=rows {
        let y = 0.08 + (i as f64 * (h - 0.16)) / rows as f64;
        parts.push(mv(bx(wf, 0.07, df, wood), 0.0, y as f32, df / 2.0));
        if i < rows {
            parts.extend(shelf_row(rand, w - 0.1, y + 0.035, d - 0.06));
        }
    }
    Prop { solid: parts, foot: Some((w + 0.1, d + 0.1)), ..Default::default() }
}

/// A big soft plant in a pot: round leaves, the toy kind.
pub fn plant(rand: &mut Seeded, h: f64) -> Prop {
    let (up, low) = (linear(o::PLANT), linear(o::PLANT_DEEP));
    let blob = |r: f64| ball(r as f32, 10, 7, o::PLANT).paint(|_, n| low.lerp(up, n.y as f64 * 0.5 + 0.5).as_vec3());
    let top = h * 0.62;
    let solid = vec![
        mv(cyl(0.28, 0.2, 0.42, 14, o::POT).ramp(&dim(o::POT, 0.7), o::POT), 0.0, 0.21, 0.32),
        mv(cyl(0.035, 0.05, (h * 0.5) as f32, 7, "#4a3428"), 0.0, (0.42 + h * 0.22) as f32, 0.32),
        mv(blob(h * 0.25), 0.0, (0.42 + top * 0.8) as f32, 0.32),
        mv(blob(h * 0.19), (h * 0.15 + rand.next() * 0.06) as f32, (0.42 + top * 0.58) as f32, 0.42),
        mv(blob(h * 0.17), (-h * 0.15) as f32, (0.42 + top * 0.64) as f32, 0.26),
        mv(blob(h * 0.15), (h * 0.02) as f32, (0.42 + top * 1.06) as f32, 0.36),
    ];
    Prop { solid, foot: Some((0.5, 0.5)), ..Default::default() }
}

/// Tall straight leaves, the kind that lives on any office floor.
pub fn snake_plant() -> Prop {
    let mut solid = vec![mv(cyl(0.24, 0.19, 0.36, 12, "#d9d2c4").ramp("#a59f92", "#e4dccd"), 0.0, 0.18, 0.3)];
    for i in 0..7 {
        let turn = (i as f64 / 7.0 * std::f64::consts::PI * 2.0) as f32;
        let leaf = cone(0.08, 0.9 + (i % 3) as f32 * 0.24, 5, 1, false).tint(o::PLANT, None).ramp(o::PLANT_DEEP, "#7fb56a");
        let leaf = shifted(leaf, 0.0, 0.5, 0.0).transform(Mat4::from_rotation_z(if i > 0 { 0.14 } else { 0.0 }));
        solid.push(leaf.moved(turn.cos() * 0.09, 0.32, 0.3 + turn.sin() * 0.09, turn));
    }
    Prop { solid, foot: Some((0.42, 0.42)), ..Default::default() }
}

pub fn rack() -> Prop {
    let led = |x: f32, y: f32, hex: &str| mv(flat(0.05, 0.03, hex), x, y, 0.71);
    let mut lights = Vec::new();
    let mut rand = Seeded::new(9);
    for u in 0..9 {
        for i in 0..5 {
            if rand.next() < 0.7 {
                let hex = if rand.next() < 0.8 { "#6fd6a0" } else { "#8fb8f2" };
                lights.push(led(-0.36 + i as f32 * 0.09, 0.42 + u as f32 * 0.21, hex));
            }
        }
    }
    let mut solid = vec![mv(soft(1.0, 2.3, 0.7, 0.04, "#191b23"), 0.0, 1.15, 0.35)];
    solid.extend((0..9).map(|u| mv(bx(0.9, 0.17, 0.02, "#1d2029"), 0.0, 0.42 + u as f32 * 0.21, 0.7)));
    Prop { solid, lit: lights, foot: Some((1.0, 0.76)), ..Default::default() }
}

pub fn arcade(accent: &str) -> Prop {
    Prop {
        solid: vec![
            mv(soft(0.92, 1.62, 0.76, 0.05, &dim(accent, 0.62)), 0.0, 0.81, 0.4),
            mv(soft(0.94, 0.3, 0.8, 0.05, &dim(accent, 0.9)), 0.0, 1.78, 0.42),
            mv(soft(0.82, 0.08, 0.34, 0.03, "#20222c"), 0.0, 1.06, 0.86),
            mv(ball(0.05, 8, 6, "#c4605a"), -0.2, 1.14, 0.88),
            mv(ball(0.04, 8, 6, "#8fb8f2"), 0.12, 1.12, 0.88),
            mv(ball(0.04, 8, 6, "#8fd9a4"), 0.26, 1.12, 0.88),
        ],
        lit: vec![mv(flat(0.66, 0.5, "#75b5ef"), 0.0, 1.38, 0.79), mv(flat(0.8, 0.16, "#ffe7c0"), 0.0, 1.8, 0.83)],
        glows: vec![Glow { x: 0.0, y: 1.4, z: 0.9, size: 1.5, hex: "#6aa8e8", level: 0.32 }],
        foot: Some((0.96, 0.86)),
        ..Default::default()
    }
}

pub fn beanbags(accent: &str) -> Prop {
    let bag = |hex: &str, r: f32| scaled(ball(r, 14, 10, hex), 1.0, 0.72, 1.0);
    Prop {
        solid: vec![
            mv(bag(&dim(accent, 0.8), 0.52), -0.9, 0.36, 0.64),
            mv(bag(&shift(accent, "#e9dfcc", 0.45), 0.48), 0.92, 0.33, 0.72),
            mv(cyl(0.4, 0.4, 0.05, 18, o::DESK_TOP), 0.0, 0.42, 0.8),
            mv(cyl(0.05, 0.08, 0.4, 8, o::DESK_BODY), 0.0, 0.2, 0.8),
            mv(cyl(0.07, 0.06, 0.12, 10, o::PAPER), 0.1, 0.5, 0.86),
        ],
        foot: Some((2.5, 1.2)),
        ..Default::default()
    }
}

/// A small round table with stools: where two people talk it over.
pub fn meeting_table(accent: &str) -> Prop {
    let stool = |x: f32, z: f32| [mv(cyl(0.2, 0.2, 0.08, 14, accent), x, 0.46, z), mv(cyl(0.03, 0.05, 0.44, 8, o::DESK_BODY), x, 0.22, z)];
    let mut solid = vec![
        mv(cyl(0.62, 0.62, 0.06, 22, o::DESK_TOP), 0.0, 0.76, 0.86),
        mv(cyl(0.05, 0.08, 0.72, 10, o::DESK_BODY), 0.0, 0.37, 0.86),
        mv(cyl(0.3, 0.32, 0.03, 16, o::DESK_BODY), 0.0, 0.02, 0.86),
        bx(0.3, 0.015, 0.22, o::PAPER).moved(-0.12, 0.8, 0.8, 0.3),
        mv(cyl(0.055, 0.05, 0.1, 10, "#e9e4d8"), 0.24, 0.84, 0.96),
    ];
    for (x, z) in [(-0.95, 0.72), (0.96, 0.82), (0.1, 0.1)] {
        solid.extend(stool(x, z));
    }
    Prop { solid, foot: Some((2.2, 1.5)), ..Default::default() }
}

/// A low cabinet with things growing on it.
pub fn plant_cabinet(rand: &mut Seeded) -> Prop {
    let top = 0.74f32;
    let mut pots = Vec::new();
    for (i, x) in [-0.86f32, -0.3, 0.3, 0.86].into_iter().enumerate() {
        let odd = i % 2 == 1;
        pots.push(mv(cyl(0.15, 0.11, 0.22, 12, if odd { "#d9d2c4" } else { o::POT }).ramp("#7a5a48", if odd { "#e4dccd" } else { o::POT }), x, top + 0.11, 0.32));
        let r = (0.17 + rand.next() * 0.09) as f32;
        let leaf = ball(r, 9, 7, if odd { o::PLANT } else { o::PLANT_DEEP });
        pots.push(mv(leaf, x, (top as f64 + 0.34 + rand.next() * 0.08) as f32, 0.32));
    }
    let mut solid = vec![mv(soft(2.4, top, 0.6, 0.04, "#3a3038"), 0.0, top / 2.0, 0.32), mv(bx(2.44, 0.05, 0.64, o::DESK_TOP), 0.0, top + 0.02, 0.32)];
    solid.extend([-0.8f32, 0.0, 0.8].map(|x| mv(bx(0.02, top - 0.14, 0.01, "#2a2730"), x, top / 2.0, 0.625)));
    solid.extend(pots);
    Prop { solid, foot: Some((2.5, 0.72)), ..Default::default() }
}

pub fn floor_lamp(on: bool) -> Prop {
    let shade = cyl(0.2, 0.3, 0.32, 16, if on { "#ffdca8" } else { "#b9b2a4" });
    Prop {
        solid: vec![mv(cyl(0.17, 0.19, 0.04, 14, o::DESK_BODY), 0.0, 0.02, 0.32), mv(cyl(0.018, 0.018, 1.74, 8, o::DESK_BODY), 0.0, 0.9, 0.32)],
        lit: vec![mv(shade, 0.0, 1.86, 0.32)],
        glows: if on { vec![Glow { x: 0.0, y: 1.86, z: 0.32, size: 1.5, hex: "#ffd0a0", level: 0.24 }] } else { Vec::new() },
        lamps: if on { vec![Lamp { x: 0.0, y: 1.8, z: 0.32, colour: rgb(WARM_LAMP), power: 0.9, reach: 1.1 }] } else { Vec::new() },
        foot: Some((0.32, 0.32)),
    }
}

pub fn coat_rack() -> Prop {
    Prop {
        solid: vec![mv(cyl(0.03, 0.03, 1.9, 8, o::DESK_BODY), 0.0, 0.95, 0.28), mv(cyl(0.2, 0.22, 0.04, 12, o::DESK_BODY), 0.0, 0.02, 0.28), mv(soft(0.4, 0.7, 0.22, 0.1, "#57617f"), 0.14, 1.42, 0.28)],
        foot: Some((0.4, 0.4)),
        ..Default::default()
    }
}

