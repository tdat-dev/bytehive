//! The camera as scene.ts holds it: a PerspectiveCamera (near 1, far 900) at a
//! shot, slid sideways by the shot's `ox` with setViewOffset, the move between
//! levels eased on a CSS curve, and points taken to CSS pixels for the words
//! drawn over the picture. three's projection is OpenGL's (z from -1 to 1);
//! wgpu's depth runs 0 to 1, so the GPU gets its own from the same frustum.

use glam::{DMat4, DVec3, DVec4};

use super::layout::Shot;

const NEAR: f64 = 1.0;
const FAR: f64 = 900.0;

/// A CSS cubic-bezier at `k`, by halving as scene.ts does it.
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, k: f64) -> f64 {
    if k <= 0.0 || k >= 1.0 {
        return if k <= 0.0 { 0.0 } else { 1.0 };
    }
    let at = |a: f64, b: f64, s: f64| 3.0 * (1.0 - s).powi(2) * s * a + 3.0 * (1.0 - s) * s * s * b + s.powi(3);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let s = (lo + hi) / 2.0;
        if at(x1, x2, s) < k { lo = s } else { hi = s }
    }
    at(y1, y2, (lo + hi) / 2.0)
}

/// A level change: it starts gently, or the first frame jumps.
pub fn ease_level(k: f64) -> f64 {
    cubic_bezier(0.2, 0.0, 0.0, 1.0, k)
}

/// How long a level change takes, ms.
pub const MOVE_MS: f64 = 560.0;

/// The camera for a shot on a `w` by `h` canvas: where it is, its view, and
/// the frustum's sides at the near plane (left, right, top, bottom).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    pub eye: DVec3,
    pub view: DMat4,
    pub sides: (f64, f64, f64, f64),
}

pub fn lens(s: &Shot, w: f64, h: f64) -> Lens {
    let eye = DVec3::new(s.pos.x, s.pos.y, s.pos.z);
    let view = DMat4::look_at_rh(eye, DVec3::new(s.target.x, s.target.y, s.target.z), DVec3::Y);
    let top = NEAR * (s.fov.to_radians() / 2.0).tan();
    let height = 2.0 * top;
    let width = (w / h) * height;
    // setViewOffset(w, h, -ox * w, 0, w, h): the whole frame, slid.
    let left = -0.5 * width + (-s.ox * w) * width / w;
    Lens { eye, view, sides: (left, left + width, top, top - height) }
}

impl Lens {
    /// three's projectionMatrix (makePerspective, OpenGL depth).
    pub fn projection_gl(&self) -> DMat4 {
        let (l, r, t, b) = self.sides;
        let x = 2.0 * NEAR / (r - l);
        let y = 2.0 * NEAR / (t - b);
        let (a, bb) = ((r + l) / (r - l), (t + b) / (t - b));
        let c = -(FAR + NEAR) / (FAR - NEAR);
        let d = -2.0 * FAR * NEAR / (FAR - NEAR);
        DMat4::from_cols(DVec4::new(x, 0.0, 0.0, 0.0), DVec4::new(0.0, y, 0.0, 0.0), DVec4::new(a, bb, c, -1.0), DVec4::new(0.0, 0.0, d, 0.0))
    }

    /// The same frustum with depth from 0 to 1, for wgpu.
    pub fn projection_gpu(&self) -> DMat4 {
        let gl = self.projection_gl();
        let (c, d) = (FAR / (NEAR - FAR), NEAR * FAR / (NEAR - FAR));
        DMat4::from_cols(gl.x_axis, gl.y_axis, DVec4::new(gl.z_axis.x, gl.z_axis.y, c, -1.0), DVec4::new(0.0, 0.0, d, 0.0))
    }

    /// Where a point lands, -1 to 1 each way (three's Vector3.project).
    pub fn project(&self, p: DVec3) -> DVec3 {
        self.projection_gl().project_point3(self.view.transform_point3(p))
    }

    pub fn view_proj_gpu(&self) -> DMat4 {
        self.projection_gpu() * self.view
    }
}

/// From normalised device coordinates to CSS pixels. Past z = 1 is behind the camera.
pub fn css_point(p: DVec3, w: f64, h: f64) -> (f64, f64, bool) {
    ((p.x + 1.0) / 2.0 * w, (1.0 - p.y) / 2.0 * h, p.x.abs() <= 1.0 && p.y.abs() <= 1.0 && p.z.abs() <= 1.0)
}

/// Two shots the same, within a millionth.
pub fn same_shot(a: &Shot, b: &Shot) -> bool {
    let n = |s: &Shot| [s.pos.x, s.pos.y, s.pos.z, s.target.x, s.target.y, s.target.z, s.fov, s.ox];
    n(a).iter().zip(n(b)).all(|(x, y)| (x - y).abs() < 1e-6)
}

/// How far the camera is from what it looks at.
pub fn distance(s: &Shot) -> f64 {
    (s.pos.x - s.target.x).hypot(s.pos.y - s.target.y).hypot(s.pos.z - s.target.z)
}

/// How tall a mark over someone who needs you stands on screen, px.
pub const MARK_PX: f64 = 36.0;

/// The mark's scale: the same size on screen at every level.
pub fn mark_scale(s: &Shot, height: f64) -> f64 {
    let per_pixel = 2.0 * (s.fov.to_radians() / 2.0).tan() * distance(s) / height;
    MARK_PX * per_pixel
}

/// The glow of a lit room is for the far view: close to, it would only grey the picture.
pub fn haze_opacity(s: &Shot) -> f64 {
    ((distance(s) - 75.0) / 60.0).clamp(0.0, 1.0)
}
