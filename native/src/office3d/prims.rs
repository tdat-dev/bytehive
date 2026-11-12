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
    let mut m = Mesh::default();
    // buildPlane(u, v, w, udir, vdir, width, height, depth), in three's order.
    let planes: [(usize, usize, usize, f32, f32, f32, f32, f32); 6] = [
        (2, 1, 0, -1.0, -1.0, depth, height, width),
        (2, 1, 0, 1.0, -1.0, depth, height, -width),
        (0, 2, 1, 1.0, 1.0, width, depth, height),
        (0, 2, 1, 1.0, -1.0, width, depth, -height),
        (0, 1, 2, 1.0, -1.0, width, height, depth),
        (0, 1, 2, -1.0, -1.0, width, height, -depth),
    ];
    for (u, v, w, udir, vdir, pw, ph, pd) in planes {
        let start = m.pos.len() as u32;
        for iy in 0..2 {
            let y = iy as f32 * ph - ph / 2.0;
            for ix in 0..2 {
                let x = ix as f32 * pw - pw / 2.0;
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
        let (a, b, c, d) = (start, start + 2, start + 3, start + 1);
        m.index.extend([a, b, d, b, c, d]);
    }
    m.done()
}

/// PlaneGeometry(width, height, 1, 1): in XY, facing +Z.
pub fn plane(width: f32, height: f32) -> Geom {
    let mut m = Mesh::default();
    for iy in 0..2 {
        let y = iy as f32 * height - height / 2.0;
        for ix in 0..2 {
            let x = ix as f32 * width - width / 2.0;
            m.pos.push(Vec3::new(x, -y, 0.0));
            m.normal.push(Vec3::Z);
        }
    }
    let (a, b, c, d) = (0, 2, 3, 1);
    m.index.extend([a, b, d, b, c, d]);
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
