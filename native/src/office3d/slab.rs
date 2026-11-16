//! roundedSlab (geometry.ts): a plate with rounded corners and a soft edge, its
//! top at y = 0, most of the office's toy look. Built as three.js 0.170 builds
//! it: a Shape of four arcs, ExtrudeGeometry with a bevel, turned to lie flat,
//! then flatTopped (toCreasedNormals at 0.9, level faces straight up or down).
//! Worked in f64 as JavaScript does; stored as f32 as its buffers are.

use std::collections::HashMap;
use std::f64::consts::PI;

use glam::{DVec3, Vec3};

use super::earcut::triangulate;
use super::geom::Geom;
use super::lathe::arc_point;

const BEVEL: f64 = 0.14;
const CURVE_SEGMENTS: u32 = 8;
const BEVEL_SEGMENTS: u32 = 3;

/// The outline's points, Shape.extractPoints(curveSegments): each arc in
/// curveSegments * 2 steps, a straight line between arcs, repeats dropped.
fn outline(hw: f64, hd: f64, q: f64) -> Vec<(f64, f64)> {
    let arcs = [(hw - q, hd - q, 0.0, PI / 2.0), (-hw + q, hd - q, PI / 2.0, PI), (-hw + q, -hd + q, PI, PI * 1.5), (hw - q, -hd + q, PI * 1.5, PI * 2.0)];
    let steps = CURVE_SEGMENTS * 2;
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for (cx, cy, a0, a1) in arcs {
        for d in 0..=steps {
            let p = arc_point(cx, cy, q, a0, a1, d as f64 / steps as f64);
            if pts.last() != Some(&p) {
                pts.push(p);
            }
        }
    }
    // Shape's points end where the last arc ends; a closing repeat would go.
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    pts
}

/// getBevelVec: which way, and how far, a contour point moves as the bevel grows.
fn bevel_vec(pt: (f64, f64), prev: (f64, f64), next: (f64, f64)) -> (f64, f64) {
    let (vpx, vpy) = (pt.0 - prev.0, pt.1 - prev.1);
    let (vnx, vny) = (next.0 - pt.0, next.1 - pt.1);
    let prev_lensq = vpx * vpx + vpy * vpy;
    let collinear = vpx * vny - vpy * vnx;
    let (tx, ty, shrink);
    if collinear.abs() > f64::EPSILON {
        let prev_len = prev_lensq.sqrt();
        let next_len = (vnx * vnx + vny * vny).sqrt();
        let (psx, psy) = (prev.0 - vpy / prev_len, prev.1 + vpx / prev_len);
        let (nsx, nsy) = (next.0 - vny / next_len, next.1 + vnx / next_len);
        let sf = ((nsx - psx) * vny - (nsy - psy) * vnx) / (vpx * vny - vpy * vnx);
        let (x, y) = (psx + vpx * sf - pt.0, psy + vpy * sf - pt.1);
        let lensq = x * x + y * y;
        if lensq <= 2.0 {
            return (x, y);
        }
        (tx, ty, shrink) = (x, y, (lensq / 2.0).sqrt());
    } else {
        let same = if vpx > f64::EPSILON {
            vnx > f64::EPSILON
        } else if vpx < -f64::EPSILON {
            vnx < -f64::EPSILON
        } else {
            vpy.signum() == vny.signum()
        };
        if same {
            (tx, ty, shrink) = (-vpy, vpx, prev_lensq.sqrt());
        } else {
            (tx, ty, shrink) = (vpx, vpy, (prev_lensq / 2.0).sqrt());
        }
    }
    (tx / shrink, ty / shrink)
}

/// roundedSlab(w, d, h, r, bevel = 0.14).
pub fn rounded_slab(w: f64, d: f64, h: f64, r: f64, bevel: Option<f64>) -> Geom {
    let bevel = bevel.unwrap_or(BEVEL);
    let (hw, hd) = (w / 2.0 - bevel, d / 2.0 - bevel);
    let q = (r - bevel).min(hw).min(hd).max(0.01);
    let depth = (h - bevel * 2.0).max(0.01);
    // ShapeUtils.isClockWise is false for these counter-clockwise arcs: reversed.
    let mut contour = outline(hw, hd, q);
    contour.reverse();
    let faces = triangulate(&contour);
    let n = contour.len();
    let moves: Vec<(f64, f64)> = (0..n).map(|i| bevel_vec(contour[i], contour[(i + n - 1) % n], contour[(i + 1) % n])).collect();
    // The rings, bottom bevel up, the two straight layers, the top bevel down.
    let mut rings: Vec<DVec3> = Vec::new();
    let ring = |rings: &mut Vec<DVec3>, bs: f64, z: f64| {
        for i in 0..n {
            rings.push(DVec3::new(contour[i].0 + moves[i].0 * bs, contour[i].1 + moves[i].1 * bs, z));
        }
    };
    let at = |b: u32| {
        let t = b as f64 / BEVEL_SEGMENTS as f64;
        (bevel * (t * PI / 2.0).cos(), bevel * (t * PI / 2.0).sin())
    };
    for b in 0..BEVEL_SEGMENTS {
        let (z, bs) = at(b);
        ring(&mut rings, bs, -z);
    }
    ring(&mut rings, bevel, 0.0);
    ring(&mut rings, bevel, depth);
    for b in (0..BEVEL_SEGMENTS).rev() {
        let (z, bs) = at(b);
        ring(&mut rings, bs, depth + z);
    }
    // Faces: the bottom lid (turned over), the top lid, then the walls.
    let top = (1 + BEVEL_SEGMENTS * 2) as usize * n;
    let mut corners: Vec<usize> = Vec::new();
    for f in &faces {
        corners.extend([f[2], f[1], f[0]]);
    }
    for f in &faces {
        corners.extend([f[0] + top, f[1] + top, f[2] + top]);
    }
    for i in (0..n).rev() {
        let (j, k) = (i, if i == 0 { n - 1 } else { i - 1 });
        for s in 0..(1 + BEVEL_SEGMENTS * 2) as usize {
            let (a, b, c, dd) = (j + n * s, k + n * s, k + n * (s + 1), j + n * (s + 1));
            corners.extend([a, b, dd, b, c, dd]);
        }
    }
    // rotateX(-PI/2), then down so the top sits at y = 0; kept as f32.
    let (c, s) = ((-PI / 2.0).cos(), (-PI / 2.0).sin());
    let pos: Vec<Vec3> = corners
        .iter()
        .map(|&i| {
            let p = rings[i];
            let p = DVec3::new(p.x as f32 as f64, p.y as f32 as f64, p.z as f32 as f64);
            let y = p.y * c - p.z * s;
            let z = p.y * s + p.z * c;
            let (x, y, z) = (p.x as f32 as f64, y as f32 as f64, z as f32 as f64);
            Vec3::new(x as f32, (y - (depth + bevel)) as f32, z as f32)
        })
        .collect();
    let normal = flat_topped(&pos);
    Geom { color: vec![Vec3::ONE; pos.len()], pos, normal }
}

/// toCreasedNormals(g, 0.9), then every level triangle straight up or down.
fn flat_topped(pos: &[Vec3]) -> Vec<Vec3> {
    let crease = 0.9f64.cos();
    let mult = (1.0 + 1e-10) * 1e2;
    let key = |v: Vec3| ((v.x as f64 * mult) as i64, (v.y as f64 * mult) as i64, (v.z as f64 * mult) as i64);
    let face = |i: usize| {
        let (a, b, c) = (pos[i].as_dvec3(), pos[i + 1].as_dvec3(), pos[i + 2].as_dvec3());
        (c - b).cross(a - b).normalize_or_zero()
    };
    let mut seen: HashMap<(i64, i64, i64), Vec<DVec3>> = HashMap::new();
    for t in (0..pos.len()).step_by(3) {
        let n = face(t);
        for k in 0..3 {
            seen.entry(key(pos[t + k])).or_default().push(n);
        }
    }
    let mut out = vec![Vec3::ZERO; pos.len()];
    for t in (0..pos.len()).step_by(3) {
        let n = face(t);
        for k in 0..3 {
            let sum = seen[&key(pos[t + k])].iter().filter(|o| n.dot(**o) > crease).fold(DVec3::ZERO, |s, o| s + *o);
            out[t + k] = sum.normalize_or_zero().as_vec3();
        }
        let y = pos[t].y;
        if (y - pos[t + 1].y).abs() < 1e-5 && (y - pos[t + 2].y).abs() < 1e-5 {
            let up = if y < -1e-5 { -1.0 } else { 1.0 };
            for k in 0..3 {
                out[t + k] = Vec3::new(0.0, up, 0.0);
            }
        }
    }
    out
}
