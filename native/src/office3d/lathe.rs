//! LatheGeometry and the CapsuleGeometry built on it, as three.js 0.170 makes
//! them. Worked in f64 as JavaScript does, stored as f32 as its buffers are.

use std::f64::consts::PI;

use glam::Vec3;

use super::geom::Geom;

/// LatheGeometry(points, segments): the profile (x out, y up) turned about Y.
pub fn lathe(points: &[(f64, f64)], segments: u32) -> Geom {
    let n = points.len();
    // Each profile point's normal in 2D: the edge after it turned, mixed with the one before.
    let mut init = Vec::with_capacity(n);
    let mut prev = (0.0, 0.0);
    for j in 0..n {
        if j == n - 1 {
            init.push(prev);
            continue;
        }
        let (dx, dy) = (points[j + 1].0 - points[j].0, points[j + 1].1 - points[j].1);
        let cur = (dy, -dx);
        let sum = if j == 0 { cur } else { (cur.0 + prev.0, cur.1 + prev.1) };
        let len = (sum.0 * sum.0 + sum.1 * sum.1).sqrt();
        init.push(if len > 0.0 { (sum.0 / len, sum.1 / len) } else { sum });
        prev = cur;
    }
    let mut pos = Vec::new();
    let mut normal = Vec::new();
    for i in 0..=segments {
        let phi = i as f64 * (1.0 / segments as f64) * PI * 2.0;
        let (sin, cos) = phi.sin_cos();
        for j in 0..n {
            pos.push(Vec3::new((points[j].0 * sin) as f32, points[j].1 as f32, (points[j].0 * cos) as f32));
            normal.push(Vec3::new((init[j].0 * sin) as f32, init[j].1 as f32, (init[j].0 * cos) as f32));
        }
    }
    let mut index = Vec::new();
    for i in 0..segments as usize {
        for j in 0..n - 1 {
            let base = (j + i * n) as u32;
            let (a, b, c, d) = (base, base + n as u32, base + n as u32 + 1, base + 1);
            index.extend([a, b, d, c, d, b]);
        }
    }
    Geom::from_indexed(&pos, &normal, &index)
}

/// A point on an arc, EllipseCurve.getPoint with equal radii and no turn.
pub(super) fn arc_point(cx: f64, cy: f64, r: f64, start: f64, end: f64, t: f64) -> (f64, f64) {
    let mut delta = end - start;
    let same = delta.abs() < f64::EPSILON;
    while delta < 0.0 {
        delta += PI * 2.0;
    }
    while delta > PI * 2.0 {
        delta -= PI * 2.0;
    }
    if delta < f64::EPSILON {
        delta = if same { 0.0 } else { PI * 2.0 };
    }
    let a = start + t * delta;
    (cx + r * a.cos(), cy + r * a.sin())
}

/// CapsuleGeometry(radius, length, capSegments, radialSegments).
pub fn capsule(radius: f64, length: f64, cap: u32, radial: u32) -> Geom {
    // The path: the lower arc, a line up the side, the upper arc; each arc in
    // cap * 2 steps, a repeated point dropped (CurvePath.getPoints).
    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut push = |p: (f64, f64)| {
        if points.last() != Some(&p) {
            points.push(p);
        }
    };
    let steps = cap * 2;
    for d in 0..=steps {
        push(arc_point(0.0, -length / 2.0, radius, PI * 1.5, 0.0, d as f64 / steps as f64));
    }
    push(arc_point(0.0, length / 2.0, radius, 0.0, PI * 0.5, 0.0));
    for d in 0..=steps {
        push(arc_point(0.0, length / 2.0, radius, 0.0, PI * 0.5, d as f64 / steps as f64));
    }
    lathe(&points, radial)
}
