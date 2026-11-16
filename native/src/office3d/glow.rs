//! What a light throws, as geometry.ts makes it: a plane whose corners carry
//! the light's colour times its level. `flare` stands facing the viewer (the
//! view never turns), `pool` lies on something flat, `wash` stands on a wall.

use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use super::geom::{Geom, hex};
use super::layout::VIEW;
use super::prims::plane;

fn light_plane(g: Geom, colour: &str, level: f32) -> Geom {
    let c = hex(colour) * level;
    let n = g.len();
    Geom { color: vec![c; n], ..g }
}

/// The glow round a light, turned to face the camera.
pub fn flare(x: f32, y: f32, z: f32, size: f32, colour: &str, level: f32) -> Geom {
    let len = (VIEW.x * VIEW.x + VIEW.y * VIEW.y + VIEW.z * VIEW.z).sqrt();
    let yaw = VIEW.x.atan2(VIEW.z) as f32;
    let pitch = (VIEW.y / len).asin() as f32;
    let g = plane(size, size).transform(Mat4::from_rotation_x(-pitch)).transform(Mat4::from_rotation_y(yaw)).transform(Mat4::from_translation(Vec3::new(x, y, z)));
    light_plane(g, colour, level)
}

/// A pool of light lying on something flat.
pub fn pool(x: f32, y: f32, z: f32, w: f32, d: f32, colour: &str, level: f32) -> Geom {
    let g = plane(w, d).transform(Mat4::from_rotation_x(-PI / 2.0)).transform(Mat4::from_translation(Vec3::new(x, y, z)));
    light_plane(g, colour, level)
}

/// The same on a wall facing +z or, turned, +x.
pub fn wash(x: f32, y: f32, z: f32, w: f32, h: f32, turn: f32, colour: &str, level: f32) -> Geom {
    let g = plane(w, h).transform(Mat4::from_rotation_y(turn)).transform(Mat4::from_translation(Vec3::new(x, y, z)));
    light_plane(g, colour, level)
}
