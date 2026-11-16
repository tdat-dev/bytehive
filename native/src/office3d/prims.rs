//! The shapes three.js 0.170 makes, with its own algorithms, corner for corner:
//! Box, Plane, Circle, Cylinder and Cone, Sphere, Torus. Each is built indexed
//! the way three builds it, then unrolled as `toNonIndexed` does, so a merged
//! floor has the same triangles in the same order as the Tauri office.

use std::f32::consts::PI;

use glam::Vec3;

use super::geom::Geom;

/// Positions, normals and the triangle list of an indexed mesh being built.
#[derive(Default)]
struct Mesh {
    pos: Vec<Vec3>,
    normal: Vec<Vec3>,
    index: Vec<u32>,
}

impl Mesh {
    fn done(self) -> Geom {
        Geom::from_indexed(&self.pos, &self.normal, &self.index)
    }
}

/// BoxGeometry(width, height, depth), one segment a side.
pub fn box_geom(width: f32, height: f32, depth: f32) -> Geom {
    box_split(width, height, depth, 1, 1, 1)
}

/// BoxGeometry(width, height, depth, widthSegments, heightSegments, depthSegments).
pub fn box_split(width: f32, height: f32, depth: f32, ws: u32, hs: u32, ds: u32) -> Geom {
    let mut m = Mesh::default();
    // buildPlane(u, v, w, udir, vdir, width, height, depth, gridX, gridY), in three's order.
    let planes: [(usize, usize, usize, f32, f32, f32, f32, f32, u32, u32); 6] = [
        (2, 1, 0, -1.0, -1.0, depth, height, width, ds, hs),
        (2, 1, 0, 1.0, -1.0, depth, height, -width, ds, hs),
        (0, 2, 1, 1.0, 1.0, width, depth, height, ws, ds),
        (0, 2, 1, 1.0, -1.0, width, depth, -height, ws, ds),
        (0, 1, 2, 1.0, -1.0, width, height, depth, ws, hs),
        (0, 1, 2, -1.0, -1.0, width, height, -depth, ws, hs),
    ];
    for (u, v, w, udir, vdir, pw, ph, pd, gx, gy) in planes {
        let start = m.pos.len() as u32;
        let (sw, sh) = (pw / gx as f32, ph / gy as f32);
        for iy in 0..=gy {
            let y = iy as f32 * sh - ph / 2.0;
            for ix in 0..=gx {
                let x = ix as f32 * sw - pw / 2.0;
                let mut p = [0.0f32; 3];
                p[u] = x * udir;
                p[v] = y * vdir;
                p[w] = pd / 2.0;
                let mut n = [0.0f32; 3];
                n[w] = if pd > 0.0 { 1.0 } else { -1.0 };
                m.pos.push(Vec3::from(p));
                m.normal.push(Vec3::from(n));
            }
        }
        let row = gx + 1;
        for iy in 0..gy {
            for ix in 0..gx {
                let a = start + ix + row * iy;
                let b = start + ix + row * (iy + 1);
                let c = start + (ix + 1) + row * (iy + 1);
                let d = start + (ix + 1) + row * iy;
                m.index.extend([a, b, d, b, c, d]);
            }
        }
    }
    m.done()
}

/// JavaScript's Math.sign: 0 stays 0 (Rust's signum gives 1).
fn sign(v: f64) -> f64 {
    if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }
}

/// RoundedBoxGeometry(width, height, depth, segments, radius), from three's
/// examples: a unit box cut in (segments * 2 + 1) a side, each corner pushed
/// out onto a rounded corner of the real size.
pub fn rounded_box(width: f32, height: f32, depth: f32, segments: u32, radius: f32) -> Geom {
    let n = segments * 2 + 1;
    let radius = (width / 2.0).min(height / 2.0).min(depth / 2.0).min(radius) as f64;
    let mut g = box_split(1.0, 1.0, 1.0, n, n, n);
    if n == 1 {
        return g;
    }
    let half = 0.5 / n as f64;
    let size = [width as f64 / 2.0 - radius, height as f64 / 2.0 - radius, depth as f64 / 2.0 - radius];
    for i in 0..g.len() {
        let p = g.pos[i].as_dvec3().to_array();
        let mut nrm = [p[0] - sign(p[0]) * half, p[1] - sign(p[1]) * half, p[2] - sign(p[2]) * half];
        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if len > 0.0 {
            nrm = nrm.map(|v| v / len);
        }
        g.pos[i] = Vec3::new((size[0] * sign(p[0]) + nrm[0] * radius) as f32, (size[1] * sign(p[1]) + nrm[1] * radius) as f32, (size[2] * sign(p[2]) + nrm[2] * radius) as f32);
        g.normal[i] = Vec3::new(nrm[0] as f32, nrm[1] as f32, nrm[2] as f32);
    }
    g
}

/// PlaneGeometry(width, height, 1, 1): in XY, facing +Z.
pub fn plane(width: f32, height: f32) -> Geom {
    plane_split(width, height, 1, 1)
}

/// PlaneGeometry(width, height, widthSegments, heightSegments).
pub fn plane_split(width: f32, height: f32, gx: u32, gy: u32) -> Geom {
    let mut m = Mesh::default();
    let (sw, sh) = (width / gx as f32, height / gy as f32);
    for iy in 0..=gy {
        let y = iy as f32 * sh - height / 2.0;
        for ix in 0..=gx {
            let x = ix as f32 * sw - width / 2.0;
            m.pos.push(Vec3::new(x, -y, 0.0));
            m.normal.push(Vec3::Z);
        }
    }
    let row = gx + 1;
    for iy in 0..gy {
        for ix in 0..gx {
            let (a, b, c, d) = (ix + row * iy, ix + row * (iy + 1), (ix + 1) + row * (iy + 1), (ix + 1) + row * iy);
            m.index.extend([a, b, d, b, c, d]);
        }
    }
    m.done()
}

/// CircleGeometry(radius, segments): in XY, facing +Z.
pub fn circle(radius: f32, segments: u32) -> Geom {
    let mut m = Mesh::default();
    m.pos.push(Vec3::ZERO);
    m.normal.push(Vec3::Z);
    for s in 0..=segments {
        let a = s as f32 / segments as f32 * PI * 2.0;
        m.pos.push(Vec3::new(radius * a.cos(), radius * a.sin(), 0.0));
        m.normal.push(Vec3::Z);
    }
    for i in 1..=segments {
        m.index.extend([i, i + 1, 0]);
    }
    m.done()
}

/// CylinderGeometry(radiusTop, radiusBottom, height, radialSegments, heightSegments, openEnded).
pub fn cylinder(top: f32, bottom: f32, height: f32, radial: u32, rings: u32, open: bool) -> Geom {
    let mut m = Mesh::default();
    let half = height / 2.0;
    let slope = (bottom - top) / height;
    let mut grid: Vec<Vec<u32>> = Vec::new();
    for y in 0..=rings {
        let v = y as f32 / rings as f32;
        let radius = v * (bottom - top) + top;
        let mut row = Vec::new();
        for x in 0..=radial {
            let theta = x as f32 / radial as f32 * PI * 2.0;
            let (s, c) = theta.sin_cos();
            m.pos.push(Vec3::new(radius * s, -v * height + half, radius * c));
            m.normal.push(Vec3::new(s, slope, c).normalize());
            row.push(m.pos.len() as u32 - 1);
        }
        grid.push(row);
    }
    for x in 0..radial as usize {
        for y in 0..rings as usize {
            let (a, b, c, d) = (grid[y][x], grid[y + 1][x], grid[y + 1][x + 1], grid[y][x + 1]);
            if top > 0.0 || y != 0 {
                m.index.extend([a, b, d]);
            }
            if bottom > 0.0 || y != rings as usize - 1 {
                m.index.extend([b, c, d]);
            }
        }
    }
    if !open {
        for (is_top, radius) in [(true, top), (false, bottom)] {
            if radius <= 0.0 {
                continue;
            }
            let sign = if is_top { 1.0 } else { -1.0 };
            let centers = m.pos.len() as u32;
            for _ in 1..=radial {
                m.pos.push(Vec3::new(0.0, half * sign, 0.0));
                m.normal.push(Vec3::new(0.0, sign, 0.0));
            }
            let ring = m.pos.len() as u32;
            for x in 0..=radial {
                let theta = x as f32 / radial as f32 * PI * 2.0;
                m.pos.push(Vec3::new(radius * theta.sin(), half * sign, radius * theta.cos()));
                m.normal.push(Vec3::new(0.0, sign, 0.0));
            }
            for x in 0..radial {
                let (c, i) = (centers + x, ring + x);
                if is_top { m.index.extend([i, i + 1, c]) } else { m.index.extend([i + 1, i, c]) }
            }
        }
    }
    m.done()
}

/// ConeGeometry(radius, height, radialSegments, heightSegments, openEnded).
pub fn cone(radius: f32, height: f32, radial: u32, rings: u32, open: bool) -> Geom {
    cylinder(0.0, radius, height, radial, rings, open)
}

/// SphereGeometry(radius, widthSegments, heightSegments, phiStart, phiLength, thetaStart, thetaLength).
pub fn sphere_part(radius: f32, width: u32, height: u32, phi: (f32, f32), theta: (f32, f32)) -> Geom {
    let mut m = Mesh::default();
    let theta_end = (theta.0 + theta.1).min(PI);
    let mut grid: Vec<Vec<u32>> = Vec::new();
    for iy in 0..=height {
        let v = iy as f32 / height as f32;
        let mut row = Vec::new();
        for ix in 0..=width {
            let u = ix as f32 / width as f32;
            let (ph, th) = (phi.0 + u * phi.1, theta.0 + v * theta.1);
            let p = Vec3::new(-radius * ph.cos() * th.sin(), radius * th.cos(), radius * ph.sin() * th.sin());
            m.pos.push(p);
            m.normal.push(p.normalize_or_zero());
            row.push(m.pos.len() as u32 - 1);
        }
        grid.push(row);
    }
    for iy in 0..height as usize {
        for ix in 0..width as usize {
            let (a, b, c, d) = (grid[iy][ix + 1], grid[iy][ix], grid[iy + 1][ix], grid[iy + 1][ix + 1]);
            if iy != 0 || theta.0 > 0.0 {
                m.index.extend([a, b, d]);
            }
            if iy != height as usize - 1 || theta_end < PI {
                m.index.extend([b, c, d]);
            }
        }
    }
    m.done()
}

pub fn sphere(radius: f32, width: u32, height: u32) -> Geom {
    sphere_part(radius, width, height, (0.0, PI * 2.0), (0.0, PI))
}

/// TorusGeometry(radius, tube, radialSegments, tubularSegments, arc).
pub fn torus(radius: f32, tube: f32, radial: u32, tubular: u32, arc: f32) -> Geom {
    let mut m = Mesh::default();
    for j in 0..=radial {
        for i in 0..=tubular {
            let u = i as f32 / tubular as f32 * arc;
            let v = j as f32 / radial as f32 * PI * 2.0;
            let p = Vec3::new((radius + tube * v.cos()) * u.cos(), (radius + tube * v.cos()) * u.sin(), tube * v.sin());
            let center = Vec3::new(radius * u.cos(), radius * u.sin(), 0.0);
            m.pos.push(p);
            m.normal.push((p - center).normalize());
        }
    }
    for j in 1..=radial {
        for i in 1..=tubular {
            let a = (tubular + 1) * j + i - 1;
            let b = (tubular + 1) * (j - 1) + i - 1;
            let c = (tubular + 1) * (j - 1) + i;
            let d = (tubular + 1) * j + i;
            m.index.extend([a, b, d, b, c, d]);
        }
    }
    m.done()
}
