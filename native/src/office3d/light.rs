//! The office's light, worked out when a floor is built and left on its
//! vertices (light.ts): pools under the lamps, soft corners, and the dark under
//! anything that stands on the floor. It costs nothing to draw, and it is still
//! there with shadows off. Worked in f64 as JavaScript does.

use super::geom::{Geom, hex};
use super::layout::Seat;

pub type Rgb = [f64; 3];

/// A light in a room. `reach` is where it has fallen to half.
#[derive(Clone, Debug)]
pub struct Lamp {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub colour: Rgb,
    pub power: f64,
    pub reach: f64,
}

/// Something on the floor the light does not get under.
#[derive(Clone, Debug)]
pub struct Blocker {
    pub x: f64,
    pub z: f64,
    pub w: f64,
    pub d: f64,
    pub dark: f64,
}

#[derive(Clone, Debug)]
pub struct RoomLight {
    pub ambient: Rgb,
    pub lamps: Vec<Lamp>,
    pub blockers: Vec<Blocker>,
}

/// A CSS colour as three makes it, linear.
pub fn rgb(colour: &str) -> Rgb {
    let c = hex(colour);
    [c.x as f64, c.y as f64, c.z as f64]
}

const SHADE_REACH: f64 = 0.62;

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let k = ((x - a) / (b - a)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// How much of the room's light gets to this point, in each colour.
pub fn light_at(room: &RoomLight, x: f64, y: f64, z: f64) -> Rgb {
    let mut out = room.ambient;
    for l in &room.lamps {
        let d2 = (l.x - x).powi(2) + (l.y - y).powi(2) + (l.z - z).powi(2);
        let k = l.power / (1.0 + d2 / (l.reach * l.reach));
        for c in 0..3 {
            out[c] += l.colour[c] * k;
        }
    }
    out
}

/// 1 in the open, down to `1 - dark` under a blocker, with a soft edge round it.
pub fn shade_at(blockers: &[Blocker], x: f64, z: f64) -> f64 {
    let mut lit = 1.0;
    for b in blockers {
        let dx = ((x - b.x).abs() - b.w / 2.0).max(0.0);
        let dz = ((z - b.z).abs() - b.d / 2.0).max(0.0);
        if dx > SHADE_REACH || dz > SHADE_REACH {
            continue;
        }
        lit *= 1.0 - b.dark * (1.0 - smooth(0.0, SHADE_REACH, dx.hypot(dz)));
    }
    lit
}

/// Puts a room's light on what stands in it; what lies near the floor takes the blockers too.
pub fn bake(mut g: Geom, room: &RoomLight) -> Geom {
    for i in 0..g.len() {
        let p = g.pos[i];
        let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
        let [r, gg, b] = light_at(room, x, y, z);
        let low = if y < 0.5 { 1.0 + (shade_at(&room.blockers, x, z) - 1.0) * (1.0 - y / 0.5) } else { 1.0 };
        let c = g.color[i];
        g.color[i] = glam::Vec3::new((c.x as f64 * r * low) as f32, (c.y as f64 * gg * low) as f32, (c.z as f64 * b * low) as f32);
    }
    g
}

/// Where a desk and its chair keep the light off the floor.
pub fn desk_blockers(desk: &Seat, chair: (f64, f64)) -> Vec<Blocker> {
    vec![Blocker { x: desk.x, z: desk.z, w: 1.5, d: 0.74, dark: 0.58 }, Blocker { x: chair.0, z: chair.1, w: 0.3, d: 0.3, dark: 0.38 }]
}
