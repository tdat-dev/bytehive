//! The top of the tower (build.ts roofSteps): the deck, a glass rail, the
//! penthouse with its lit door and windows, string lights, planters, the sign
//! on its post, and the lift's shaft and mast. Everything that depends on the
//! height alone; the Founder's desk goes on it with the people.

use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use super::build_parts::{SLAB_D, SLAB_W, SLAB_X, lift_car, shell_geometry, string_lights};
use super::geom::Geom;
use super::glow::{flare, pool};
use super::layout::{GROUND, LIFT_W, ROOM_D, ROOM_W, SLAB, Tower};
use super::palette::office as o;
use super::props::{ball, bx, cyl, flat, mv, soft};
use super::seed::Seeded;
use super::slab::rounded_slab;

/// The roof sign: a thick hexagon, turned a little from the view.
pub const SIGN: Sign = Sign { x: 2.6, z: -2.0, r: 0.78, thick: 0.28, post: 0.36, turn: 0.42 };

pub struct Sign {
    pub x: f32,
    pub z: f32,
    pub r: f32,
    pub thick: f32,
    pub post: f32,
    pub turn: f32,
}

/// The roof, one geometry per material, and where its moving or textured parts sit.
pub struct Roof {
    /// The floor slab again, as the deck.
    pub deck: Geom,
    pub solid: Geom,
    pub lit: Geom,
    pub clear: Geom,
    pub thrown: Geom,
    /// Where the logo's picture stands on the sign, and its turn.
    pub logo_at: Vec3,
    pub logo_turn: f32,
    /// The lift car, where it starts, and the heights it stops at.
    pub car: Geom,
    pub car_at: Vec3,
    pub car_stops: Vec<f32>,
}

const HOUSE: (f32, f32, f32, f32, f32) = (0.7, -1.6, 6.0, 3.2, 2.7);

pub fn roof(t: &Tower) -> Roof {
    let (mut solid, mut lit, mut clear, mut thrown) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let y = t.roof_y as f32;
    let deck = mv(shell_geometry().0, 0.0, y, 0.0);

    // Boards underfoot, stopped short of the penthouse.
    let planks = 15;
    let deck_w = (ROOM_W + LIFT_W - 0.4) as f32;
    for i in 0..planks {
        let z = -ROOM_D / 2.0 + 0.1 + (i as f64 + 0.5) * (ROOM_D / planks as f64);
        let colour = if i % 3 != 0 { "#4a3a33" } else { "#544237" };
        solid.push(mv(bx(deck_w, 0.04, (ROOM_D / planks as f64 - 0.03) as f32, colour), SLAB_X, y + 0.02, z as f32));
    }

    // Glass rail on three sides, with a steel top.
    let (el, er) = ((-ROOM_W / 2.0 - 0.05) as f32, SLAB_X + SLAB_W / 2.0 - 0.3);
    let (ef, eb) = (SLAB_D / 2.0 - 0.3, -SLAB_D / 2.0 + 0.3);
    let rail_h = 0.72f32;
    let mut span_x = |x0: f32, x1: f32, z: f32, solid: &mut Vec<Geom>| {
        clear.push(mv(bx(x1 - x0, rail_h, 0.04, o::GLASS), (x0 + x1) / 2.0, y + rail_h / 2.0, z));
        solid.push(mv(bx(x1 - x0 + 0.05, 0.05, 0.06, o::STEEL), (x0 + x1) / 2.0, y + rail_h, z));
    };
    span_x(el, er, ef, &mut solid);
    let mut span_z = |z0: f32, z1: f32, x: f32, solid: &mut Vec<Geom>| {
        clear.push(mv(bx(0.04, rail_h, z1 - z0, o::GLASS), x, y + rail_h / 2.0, (z0 + z1) / 2.0));
        solid.push(mv(bx(0.06, 0.05, z1 - z0 + 0.05, o::STEEL), x, y + rail_h, (z0 + z1) / 2.0));
    };
    span_z(eb, ef, el, &mut solid);
    span_z((t.lift.z + LIFT_W / 2.0 + 0.1) as f32, ef, er, &mut solid);

    // Penthouse: a box, a flat overhanging roof, a lit door and one lit window.
    let (hx, hz, hw, hd, hh) = HOUSE;
    solid.push(mv(rounded_slab(hw as f64, hd as f64, hh as f64, 0.18, None).tint(o::SHELL_OUT, None), hx, y + hh, hz));
    solid.push(mv(rounded_slab((hw + 0.8) as f64, (hd + 0.8) as f64, 0.42, 0.28, None).tint(o::SHELL_OUT, Some(o::SLAB_TOP)), hx, y + hh + 0.38, hz));
    let front = hz + hd / 2.0;
    let (door_x, door_h, door_w) = (hx - 1.2, 2.1f32, 1.15f32);
    solid.push(mv(bx(door_w + 0.16, door_h + 0.1, 0.05, o::DESK_TOP), door_x, y + (door_h + 0.1) / 2.0, front + 0.01));
    lit.push(mv(flat(door_w * 0.52, door_h - 0.04, o::GLOW), door_x + door_w * 0.22, y + door_h / 2.0, front + 0.04));
    solid.push(mv(bx(door_w * 0.44, door_h - 0.04, 0.03, "#7a5534"), door_x - door_w * 0.26, y + door_h / 2.0, front + 0.05));
    let (fw, fh, fx, fy) = (1.9f32, 1.2f32, hx + 1.6, y + 1.5);
    solid.push(mv(bx(fw + 0.16, fh + 0.16, 0.05, o::DESK_TOP), fx, fy, front + 0.01));
    lit.push(mv(flat(fw, fh, o::GLOW), fx, fy, front + 0.045));
    solid.push(mv(bx(0.05, fh, 0.02, o::DESK_TOP), fx, fy, front + 0.055));
    let side = hx + hw / 2.0;
    let (pw, ph, pz, py) = (1.5f32, 1.05f32, hz + 0.1, y + 1.5);
    solid.push(mv(bx(0.05, ph + 0.16, pw + 0.16, o::DESK_TOP), side + 0.01, py, pz));
    lit.push(mv(flat(pw, ph, o::GLOW).transform(Mat4::from_rotation_y(PI / 2.0)), side + 0.045, py, pz));
    solid.push(mv(bx(0.02, ph, 0.05, o::DESK_TOP), side + 0.055, py, pz));
    thrown.push(pool(door_x + 0.3, y + 0.05, front + 1.0, 3.4, 2.6, "#ffc98a", 0.5));
    thrown.push(pool(fx, y + 0.05, front + 1.2, 4.0, 2.8, "#ffc98a", 0.4));

    // String lights over the deck, from the eaves to two posts at the rail.
    let post_l = Vec3::new(el + 0.2, y + 3.35, ef - 0.2);
    let post_r = Vec3::new(er - 0.5, y + 3.35, ef - 0.2);
    for p in [post_l, post_r] {
        solid.push(mv(cyl(0.035, 0.045, 3.4, 8, o::DESK_BODY), p.x, y + 1.7, p.z));
    }
    let eave_l = Vec3::new(hx - hw / 2.0 - 0.3, y + hh + 0.05, front + 0.34);
    let eave_r = Vec3::new(hx + hw / 2.0 + 0.3, y + hh + 0.05, front + 0.34);
    for (a, b, n) in [(eave_l, post_l, 8), (post_l, post_r, 16), (eave_r, post_r, 7)] {
        let (bulbs, glows, wire) = string_lights(a, b, n);
        solid.extend(wire);
        lit.extend(bulbs);
        thrown.extend(glows);
    }

    // Planters along the rail.
    let mut planter = |x: f32, z: f32, w: f32, d: f32| {
        solid.push(mv(soft(w, 0.5, d, 0.05, "#3a3a44"), x, y + 0.27, z));
        let mut rand = Seeded::new(((x as f64 * 13.0 + z as f64 * 7.0).round() as i64 + 99) as u32);
        let mut i = 0.0;
        while i < w as f64 * 3.0 {
            let r = 0.2 + rand.next() * 0.12;
            let colour = if rand.next() < 0.5 { o::PLANT } else { o::PLANT_DEEP };
            let (px, py, pz) = (x as f64 + (rand.next() - 0.5) * (w as f64 - 0.3), y as f64 + 0.62 + rand.next() * 0.12, z as f64 + (rand.next() - 0.5) * (d as f64 - 0.2));
            solid.push(mv(ball(r as f32, 8, 6, colour), px as f32, py as f32, pz as f32));
            i += 1.0;
        }
    };
    planter(4.7, ef - 0.42, 2.8, 0.56);
    planter(el + 0.44, -1.2, 0.56, 3.0);

    // The sign on the penthouse roof.
    let plate = cyl(SIGN.r, SIGN.r, SIGN.thick, 6, o::STEEL).transform(Mat4::from_rotation_x(PI / 2.0)).transform(Mat4::from_rotation_y(SIGN.turn));
    let roof_top = y + hh + 0.38;
    let sign_y = roof_top + SIGN.post + SIGN.r;
    solid.push(mv(plate, SIGN.x, sign_y, SIGN.z));
    solid.push(mv(cyl(0.07, 0.09, SIGN.post + 0.4, 10, o::STEEL), SIGN.x, roof_top + (SIGN.post + 0.4) / 2.0, SIGN.z));
    let out = SIGN.thick / 2.0 + 0.01;
    let logo_at = Vec3::new(SIGN.x + SIGN.turn.sin() * out, sign_y, SIGN.z + SIGN.turn.cos() * out);

    // The lift: posts from the ground to just over the roof, a glass box, a lamp
    // at each floor, and the mast from there up to the top of the tower.
    let (x, z, top) = (t.lift.x as f32, t.lift.z as f32, t.lift.y as f32);
    let half = (LIFT_W / 2.0 - 0.06) as f32;
    let bottom = (GROUND - SLAB) as f32;
    let shaft_top = (t.roof_y + 1.3) as f32;
    let height = shaft_top - bottom;
    for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        solid.push(mv(bx(0.1, height, 0.1, o::STEEL), x + dx * half, bottom + height / 2.0, z + dz * half));
    }
    let rings = t.floors.iter().map(|f| f.y as f32).chain([y, shaft_top]);
    for ry in rings {
        for s in [-1.0, 1.0] {
            solid.push(mv(bx(half * 2.0, 0.08, 0.08, o::STEEL), x, ry, z + s * half));
            solid.push(mv(bx(0.08, 0.08, half * 2.0, o::STEEL), x + s * half, ry, z));
        }
    }
    clear.push(mv(bx(half * 2.0, height, half * 2.0, o::GLASS), x, bottom + height / 2.0, z));
    for f in &t.floors {
        let fy = f.y as f32;
        lit.push(mv(bx(0.34, 0.1, 0.16, o::GLOW), x - half + 0.4, fy + 3.0, z + half - 0.02));
        thrown.push(flare(x - half + 0.4, fy + 3.0, z + half, 1.3, "#ffd8a0", 0.36));
    }
    let mast = top - shaft_top;
    solid.push(mv(cyl(0.07, 0.1, mast, 8, o::STEEL), x + 0.25, shaft_top + mast / 2.0, z - 0.25));
    solid.push(mv(cyl(0.045, 0.06, mast * 0.72, 8, o::STEEL), x - 0.22, shaft_top + mast * 0.36, z + 0.1));
    // One small warm light at the very top, for the aircraft.
    lit.push(mv(ball(0.09, 8, 6, "#ff8a70"), x + 0.25, top + 0.05, z - 0.25));
    thrown.push(flare(x + 0.25, top + 0.05, z - 0.25, 1.1, "#ff7a60", 0.5));

    let car_stops = std::iter::once(bottom + 0.05).chain(t.floors.iter().skip(1).map(|f| f.y as f32)).collect();
    Roof {
        deck,
        solid: Geom::merged(solid),
        lit: Geom::merged(lit),
        clear: Geom::merged(clear),
        thrown: Geom::merged(thrown),
        logo_at,
        logo_turn: SIGN.turn,
        car: lift_car(half),
        car_at: Vec3::new(x, bottom + 0.05, z),
        car_stops,
    }
}
