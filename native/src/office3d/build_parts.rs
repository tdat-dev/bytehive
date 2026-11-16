//! The building's fixed parts (build.ts): the shell of a floor, a desk, a
//! chair and its back, the wall board, the things that make a desk somebody's,
//! the board's notes, a string of lights and the lift car. All plain geometry
//! with its colours on the vertices, as three builds them.

use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use super::geom::Geom;
use super::glow::flare;
use super::layout::{LIFT_W, ROOM_D, ROOM_W, SLAB, STOREY};
use super::palette::office as o;
use super::prims::plane_split;
use super::props::{ball, bx, cyl, flat, mv, shifted, soft};
use super::slab::rounded_slab;

pub const DESK_TOP: f32 = 0.78;
pub const WALL: f32 = 0.34;
pub const ROOM_H: f32 = (STOREY - SLAB) as f32;
pub const SLAB_W: f32 = (ROOM_W + LIFT_W + 0.5) as f32;
pub const SLAB_D: f32 = (ROOM_D + 0.5) as f32;
/// The slab reaches over the lift, so it is not centred on the room.
pub const SLAB_X: f32 = (LIFT_W / 2.0) as f32;
const MAX_STICKIES: usize = 12;

/// The floor's slab, and the shell: the slab with the back and left walls.
pub fn shell_geometry() -> (Geom, Geom) {
    // Sizes in f64, as JavaScript has them: the wall's 0.34 as an f32 leaves its
    // rounded ends a hair apart, and the outline grows two points three's has not.
    let (wall_m, room_h) = (0.34f64, STOREY - SLAB);
    let slab = mv(rounded_slab(ROOM_W + LIFT_W + 0.5, ROOM_D + 0.5, SLAB, 0.42, Some(0.17)).tint(o::SHELL_OUT, Some(o::SLAB_TOP)), SLAB_X, 0.0, 0.0);
    let wall = |len: f64| rounded_slab(len, wall_m, room_h, 0.17, None).tint(o::SHELL_OUT, None);
    let back = mv(wall(ROOM_W + wall_m), 0.0, ROOM_H, (-ROOM_D / 2.0) as f32);
    let left = wall(ROOM_D + wall_m).moved((-ROOM_W / 2.0) as f32, ROOM_H, 0.0, PI / 2.0);
    let shell = Geom::merged([slab.clone(), back, left]);
    (slab, shell)
}

pub fn desk_geometry() -> Geom {
    Geom::merged([
        mv(soft(1.5, 0.09, 0.8, 0.035, o::DESK_TOP), 0.0, DESK_TOP - 0.045, 0.0),
        mv(soft(0.42, DESK_TOP - 0.09, 0.72, 0.03, o::DESK_BODY), -0.5, (DESK_TOP - 0.09) / 2.0, 0.0),
        mv(soft(0.42, DESK_TOP - 0.09, 0.72, 0.03, o::DESK_BODY), 0.5, (DESK_TOP - 0.09) / 2.0, 0.0),
        mv(bx(0.56, 0.4, 0.04, o::DESK_BODY), 0.0, 0.46, -0.3),
        mv(soft(0.66, 0.44, 0.05, 0.02, "#22252f"), 0.0, DESK_TOP + 0.33, -0.2),
        mv(bx(0.07, 0.14, 0.05, o::DESK_BODY), 0.0, DESK_TOP + 0.07, -0.22),
        mv(soft(0.28, 0.02, 0.18, 0.008, o::DESK_BODY), 0.0, DESK_TOP + 0.01, -0.22),
        mv(soft(0.42, 0.018, 0.15, 0.008, "#d6d3cc"), -0.02, DESK_TOP + 0.01, 0.14),
        mv(cyl(0.052, 0.045, 0.1, 10, "#e9e4d8"), -0.52, DESK_TOP + 0.05, 0.14),
    ])
}

/// Where a chair's back turns when someone leans back.
pub const CHAIR_HINGE: (f32, f32) = (0.56, 0.27);

/// The back of a chair, apart from the rest, for tipping it.
pub fn chair_back_geometry() -> Geom {
    shifted(soft(0.52, 0.62, 0.11, 0.05, o::CHAIR), 0.0, 0.31, 0.0)
}

pub fn chair_geometry() -> Geom {
    let mut parts = vec![mv(soft(0.56, 0.11, 0.54, 0.05, o::CHAIR), 0.0, 0.5, 0.0), mv(cyl(0.04, 0.05, 0.36, 8, o::CHAIR), 0.0, 0.28, 0.0)];
    for i in 0..5 {
        let turn = (i as f64 / 5.0 * std::f64::consts::PI * 2.0) as f32;
        parts.push(bx(0.32, 0.045, 0.07, o::CHAIR).moved(turn.cos() * 0.16, 0.1, -turn.sin() * 0.16, turn));
        parts.push(mv(ball(0.05, 6, 4, o::CHAIR), turn.cos() * 0.3, 0.05, -turn.sin() * 0.3));
    }
    Geom::merged(parts)
}

pub fn board_geometry() -> Geom {
    soft(2.6, 1.3, 0.06, 0.02, "#3a2c24")
}

/// Something of their own by the screen, so no two desks match.
pub fn desk_thing(seed: u32) -> Geom {
    let (x, z, y) = (-0.5f32, -0.22f32, DESK_TOP);
    match seed % 4 {
        0 => Geom::merged([mv(cyl(0.07, 0.055, 0.1, 10, "#d9d2c4"), x, y + 0.05, z), mv(ball(0.1, 8, 6, o::PLANT), x, y + 0.16, z)]),
        1 => Geom::merged([bx(0.2, 0.26, 0.02, "#2a2730").moved(x, y + 0.13, z, 0.3), flat(0.16, 0.21, "#b7c6dc").moved(x + 0.004, y + 0.13, z + 0.012, 0.3)]),
        2 => Geom::merged((0..3).map(|i| bx(0.26, 0.05, 0.2, ["#c4605a", "#3f8f8a", "#e9dfcc"][i]).moved(x, y + 0.03 + i as f32 * 0.05, z, i as f32 * 0.2 - 0.2))),
        _ => Geom::merged([mv(cyl(0.06, 0.05, 0.13, 10, "#3a3d4a"), x, y + 0.065, z), mv(bx(0.012, 0.18, 0.012, "#c4605a"), x + 0.015, y + 0.15, z), mv(bx(0.012, 0.16, 0.012, "#8fb8f2"), x - 0.02, y + 0.14, z + 0.01)]),
    }
}

/// A project board's card counts.
#[derive(Clone, Copy, Debug, Default)]
pub struct BoardCounts {
    pub todo: u32,
    pub doing: u32,
    pub done: u32,
}

/// Up to twelve notes for the wall board: a list with cards never vanishes, the rest in proportion.
pub fn stickies(b: Option<BoardCounts>) -> Vec<&'static str> {
    let Some(b) = b else { return Vec::new() };
    let lists = [(b.todo as i64, o::NOTE_TODO), (b.doing as i64, o::NOTE_DOING), (b.done as i64, o::NOTE_DONE)];
    let total: i64 = lists.iter().map(|l| l.0).sum();
    if total == 0 {
        return Vec::new();
    }
    let most = MAX_STICKIES as i64;
    let mut share: Vec<i64> = lists.iter().map(|&(c, _)| if total <= most { c } else if c > 0 { (c * most / total).max(1) } else { 0 }).collect();
    let sum = |s: &[i64]| s.iter().sum::<i64>();
    // indexOf(max): the first of the longest.
    let longest = |s: &[i64]| s.iter().enumerate().fold(0, |best, (i, &v)| if v > s[best] { i } else { best });
    while sum(&share) > most {
        let i = longest(&share);
        share[i] -= 1;
    }
    while total > most && sum(&share) < most {
        let i = longest(&share);
        share[i] += 1;
    }
    lists.iter().zip(&share).flat_map(|(&(_, colour), &n)| std::iter::repeat_n(colour, n.max(0) as usize)).collect()
}

/// The board's notes, each tilted a little, in rows of four.
pub fn note_parts(board: Option<BoardCounts>) -> Vec<Geom> {
    stickies(board)
        .into_iter()
        .enumerate()
        .map(|(i, colour)| {
            let note = flat(0.34, 0.3, colour).transform(Mat4::from_rotation_z((((i * 37) % 7) as f32 - 3.0) * 0.035));
            mv(note, -0.84 + (i % 4) as f32 * 0.56, 0.36 - (i / 4) as f32 * 0.36, 0.042)
        })
        .collect()
}

/// three's Matrix4.lookAt(eye, target, up): a rotation whose z points from target to eye.
fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let mut z = eye - target;
    if z.length_squared() == 0.0 {
        z.z = 1.0;
    }
    z = z.normalize();
    let mut x = up.cross(z);
    if x.length_squared() == 0.0 {
        if up.z.abs() == 1.0 { z.x += 0.0001 } else { z.z += 0.0001 }
        z = z.normalize();
        x = up.cross(z);
    }
    x = x.normalize();
    let y = z.cross(x);
    Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), glam::Vec4::W)
}

/// Bulbs hung between two points, sagging, with their glows and the wire.
pub fn string_lights(a: Vec3, b: Vec3, bulbs: u32) -> (Vec<Geom>, Vec<Geom>, Vec<Geom>) {
    let (mut lit, mut thrown, mut wire) = (Vec::new(), Vec::new(), Vec::new());
    let point = |k: f32| a + (b - a) * k - Vec3::new(0.0, (k * PI).sin() * 0.5, 0.0);
    for i in 0..=bulbs {
        let p = point(i as f32 / bulbs as f32);
        lit.push(mv(ball(0.07, 8, 6, o::GLOW), p.x, p.y - 0.05, p.z));
        thrown.push(flare(p.x, p.y - 0.05, p.z, 0.62, "#ffd8a0", 0.3));
        if i < bulbs {
            let q = point((i + 1) as f32 / bulbs as f32);
            let seg = super::prims::box_geom(0.018, 0.018, (q - p).length()).transform(look_at(q, p, Vec3::Y));
            let mid = (p + q) / 2.0;
            wire.push(mv(seg.tint("#1d1f28", None), mid.x, mid.y, mid.z));
        }
    }
    (lit, thrown, wire)
}

/// The car, lit from inside: the one thing on the outside of the tower that moves.
pub fn lift_car(half: f32) -> Geom {
    let (w, h) = (half * 1.5, 2.5f32);
    let frame = "#23262f";
    let wall = |turn: f32| plane_split(w - 0.12, h - 0.26, 1, 4).tint("#000000", None).ramp("#332824", "#8a6448").transform(Mat4::from_rotation_y(turn));
    Geom::merged([
        mv(bx(w, 0.13, w, frame), 0.0, 0.065, 0.0),
        mv(bx(w, 0.16, w, frame), 0.0, h, 0.0),
        mv(wall(PI / 2.0), w / 2.0, h / 2.0, 0.0),
        mv(wall(0.0), 0.0, h / 2.0, w / 2.0),
        mv(bx(w * 0.8, 0.05, 0.02, o::GLOW), 0.0, h - 0.13, w / 2.0 + 0.012),
        mv(bx(0.02, 0.05, w * 0.8, o::GLOW), w / 2.0 + 0.012, h - 0.13, 0.0),
        mv(bx(0.03, h, 0.04, frame), w / 2.0 + 0.01, h / 2.0, 0.0),
        mv(bx(0.04, h, 0.03, frame), 0.0, h / 2.0, w / 2.0 + 0.01),
        mv(bx(0.05, h, 0.05, frame), w / 2.0, h / 2.0, w / 2.0),
    ])
}
