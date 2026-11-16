//! A room's surfaces with its light on them (build.ts): the two walls as a
//! fine grid, bright under the ceiling, dark in the corner, a pool round every
//! lamp; and the floor of hexagon tiles, each its own shade. Colours worked in
//! f64 as three's Color does, stored as f32.

use std::f32::consts::PI;

use glam::{DVec3, Mat4, Vec3};

use super::build_parts::{ROOM_H, WALL};
use super::colour::linear;
use super::geom::Geom;
use super::layout::{LIFT_W, ROOM_D, ROOM_W};
use super::light::{Lamp, Rgb, RoomLight, light_at, rgb, shade_at};
use super::palette::office as o;
use super::prims::plane_split;
use super::seed::Seeded;

const BACK_Z: f64 = -ROOM_D / 2.0 + WALL as f64 / 2.0;
const LEFT_X: f64 = -ROOM_W / 2.0 + WALL as f64 / 2.0;
const HEX_R: f64 = 0.46;

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let k = ((x - a) / (b - a)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn times(c: DVec3, l: Rgb) -> DVec3 {
    DVec3::new(c.x * l[0], c.y * l[1], c.z * l[2])
}

/// The two walls, as a fine grid with the light on each point.
pub fn wall_grid(room: &RoomLight, lit: bool) -> Geom {
    // Lights set in the ceiling along each wall: they throw arcs down it, and light nothing else.
    let mut on_wall = room.clone();
    if lit {
        let wash = |x: f64, z: f64| Lamp { x, y: ROOM_H as f64 + 0.25, z, colour: rgb("#ffe6c8"), power: 0.92, reach: 0.95 };
        on_wall.lamps.extend([-4.0, -1.3, 1.4, 4.1].map(|x| wash(x, BACK_Z + 0.42)));
        on_wall.lamps.extend([-2.2, 0.6, 3.0].map(|z| wash(LEFT_X + 0.42, z)));
    }
    let top = linear(if lit { o::WALL_LIT } else { o::WALL_DARK });
    let low = linear(if lit { o::WALL_DIM } else { o::WALL_NIGHT_LOW });
    let inset = 0.005f32;
    let rh = ROOM_H;
    let back = plane_split((ROOM_W as f32) - WALL / 2.0, rh, 44, 14).transform(Mat4::from_translation(Vec3::new(WALL / 4.0, rh / 2.0, BACK_Z as f32 + inset)));
    let left = plane_split((ROOM_D as f32) - WALL / 2.0, rh, 30, 14).transform(Mat4::from_rotation_y(PI / 2.0)).transform(Mat4::from_translation(Vec3::new(LEFT_X as f32 + inset, rh / 2.0, WALL / 4.0)));
    let rhd = ROOM_H as f64;
    Geom::merged([back, left]).tint("#000000", None).paint(|p, _| {
        let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
        let corner = (x - LEFT_X).min(z - BACK_Z);
        let dark = (1.0 - 0.5 * (1.0 - smooth(0.0, 2.0, corner))) * (1.0 - 0.42 * (1.0 - smooth(0.0, 0.9, y))) * (1.0 - 0.3 * smooth(rhd - 0.5, rhd, y));
        let c = low.lerp(top, smooth(rhd * 0.12, rhd * 0.94, y));
        (times(c, light_at(&on_wall, x, y, z)) * dark).as_vec3()
    })
}

/// The floor: hexagon tiles, each its own shade, with the room's light on every corner.
pub fn hex_floor(room: &RoomLight, lit: bool, accent: &str, seed: u32) -> Geom {
    let mut rand = Seeded::new(seed);
    let (x0, x1) = (-ROOM_W / 2.0 + 0.05, ROOM_W / 2.0 + LIFT_W);
    let (z0, z1) = (-ROOM_D / 2.0 + 0.05, ROOM_D / 2.0 + 0.05);
    let tile = linear(if lit { o::FLOOR_LIT } else { o::FLOOR_DARK });
    let spice = linear(accent).lerp(tile, if lit { 0.62 } else { 0.72 });
    let mut tiles = Geom::default();
    let clamp_x = |x: f64| x.max(x0).min(x1);
    let clamp_z = |z: f64| z.max(z0).min(z1);
    let mut put = |x: f64, z: f64, c: DVec3| {
        let l = light_at(room, x, 0.0, z);
        let s = shade_at(&room.blockers, x, z);
        let edge = 1.0 - 0.34 * (1.0 - smooth(0.0, 1.2, (x - LEFT_X).min(z - BACK_Z)));
        tiles.pos.push(Vec3::new(x as f32, 0.006, z as f32));
        tiles.color.push(DVec3::new(c.x * l[0] * s * edge, c.y * l[1] * s * edge, c.z * l[2] * s * edge).as_vec3());
        tiles.normal.push(Vec3::Y);
    };
    let (dx, dz, inner) = (HEX_R * 3f64.sqrt(), HEX_R * 1.5, HEX_R * 0.95);
    let (mut row, mut z) = (0u32, z0);
    while z < z1 + HEX_R {
        let mut x = x0 + if row % 2 == 1 { dx / 2.0 } else { 0.0 };
        while x < x1 + dx {
            let base = if rand.next() < 0.05 { spice } else { tile };
            let c = base * (0.9 + rand.next() * 0.2);
            for k in 0..6 {
                let a = std::f64::consts::PI / 6.0 + k as f64 * std::f64::consts::PI / 3.0;
                let b = a + std::f64::consts::PI / 3.0;
                put(clamp_x(x), clamp_z(z), c);
                put(clamp_x(x + b.cos() * inner), clamp_z(z + b.sin() * inner), c);
                put(clamp_x(x + a.cos() * inner), clamp_z(z + a.sin() * inner), c);
            }
            x += dx;
        }
        row += 1;
        z += dz;
    }
    let grout = linear(if lit { o::GROUT } else { o::GROUT_NIGHT });
    let under = plane_split((x1 - x0) as f32, (z1 - z0) as f32, 14, 10)
        .transform(Mat4::from_rotation_x(-PI / 2.0))
        .transform(Mat4::from_translation(Vec3::new(((x0 + x1) / 2.0) as f32, 0.002, ((z0 + z1) / 2.0) as f32)))
        .tint("#000000", None)
        .paint(|p, _| {
            let (x, z) = (p.x as f64, p.z as f64);
            (times(grout, light_at(room, x, 0.0, z)) * shade_at(&room.blockers, x, z)).as_vec3()
        });
    Geom::merged([under, tiles])
}
