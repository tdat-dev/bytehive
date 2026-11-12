//! Geometry as the office keeps it (geometry.ts): triangles one after another,
//! no index, each corner with its place, the way it faces and its colour, so
//! everything of one material merges into one draw. Colours are given as CSS
//! hex and kept linear, as three.js does with colour management on.

use glam::{Mat3, Mat4, Vec3};

use super::gpu::Vertex;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geom {
    pub pos: Vec<Vec3>,
    pub normal: Vec<Vec3>,
    pub color: Vec<Vec3>,
}

/// sRGB 0..1 to linear, three.js's SRGBToLinear.
fn linear(c: f32) -> f32 {
    if c < 0.04045 { c * 0.0773993808 } else { (c * 0.9478672986 + 0.0521327014).powf(2.4) }
}

/// `#rrggbb` (or `rrggbb`) to a linear colour, as `new Color(hex)` makes it.
pub fn hex(s: &str) -> Vec3 {
    let n = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    let ch = |shift: u32| linear(((n >> shift) & 0xff) as f32 / 255.0);
    Vec3::new(ch(16), ch(8), ch(0))
}

impl Geom {
    /// Unroll an indexed mesh, as `toNonIndexed` does; colour white.
    pub fn from_indexed(pos: &[Vec3], normal: &[Vec3], index: &[u32]) -> Self {
        let pick = |v: &[Vec3]| index.iter().map(|&i| v[i as usize]).collect::<Vec<_>>();
        Self { pos: pick(pos), normal: pick(normal), color: vec![Vec3::ONE; index.len()] }
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    /// One colour, and `top` on whatever faces up (tint in geometry.ts).
    pub fn tint(mut self, body: &str, top: Option<&str>) -> Self {
        let (body, up) = (hex(body), top.map(hex));
        self.color = self.normal.iter().map(|n| match up {
            Some(up) if n.y > 0.9 => up,
            _ => body,
        }).collect();
        self
    }

    /// Moved by a matrix; normals by its normal matrix, as applyMatrix4.
    pub fn transform(mut self, m: Mat4) -> Self {
        let n = Mat3::from_mat4(m).inverse().transpose();
        for p in &mut self.pos {
            *p = m.transform_point3(*p);
        }
        for v in &mut self.normal {
            *v = (n * *v).normalize_or_zero();
        }
        self
    }

    /// Turned about Y by `turn`, then set at (x, y, z): `moved` in geometry.ts.
    pub fn moved(self, x: f32, y: f32, z: f32, turn: f32) -> Self {
        self.transform(Mat4::from_translation(Vec3::new(x, y, z)) * Mat4::from_rotation_y(turn))
    }

    pub fn merged(parts: impl IntoIterator<Item = Geom>) -> Self {
        let mut out = Geom::default();
        for p in parts {
            out.pos.extend(p.pos);
            out.normal.extend(p.normal);
            out.color.extend(p.color);
        }
        out
    }

    /// Colour each corner from where it is and which way it faces.
    pub fn paint(mut self, at: impl Fn(Vec3, Vec3) -> Vec3) -> Self {
        for i in 0..self.pos.len() {
            self.color[i] = at(self.pos[i], self.normal[i]);
        }
        self
    }

    /// From `low` at the foot to `high` at the top.
    pub fn ramp(self, low: &str, high: &str) -> Self {
        let (a, b) = (hex(low), hex(high));
        let (min, max) = self.pos.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        let span = if max - min == 0.0 { 1.0 } else { max - min };
        self.paint(|p, _| a.lerp(b, (p.y - min) / span))
    }

    pub fn vertices(&self) -> Vec<Vertex> {
        (0..self.pos.len()).map(|i| Vertex { pos: self.pos[i].to_array(), normal: self.normal[i].to_array(), color: self.color[i].to_array() }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_linear_as_three_makes_it() {
        let c = hex("#ff8000");
        assert!((c.x - 1.0).abs() < 1e-6);
        // 0x80 = 128: three.js gives 0.21586 for it in linear.
        assert!((c.y - 0.21586).abs() < 1e-4, "{}", c.y);
        assert_eq!(c.z, 0.0);
    }

    #[test]
    fn tint_paints_the_top_apart() {
        let g = Geom { pos: vec![Vec3::ZERO; 2], normal: vec![Vec3::Y, Vec3::X], color: vec![Vec3::ZERO; 2] }.tint("#000000", Some("#ffffff"));
        assert_eq!(g.color, vec![Vec3::ONE, Vec3::ZERO]);
    }

    #[test]
    fn moved_turns_then_places() {
        let g = Geom { pos: vec![Vec3::X], normal: vec![Vec3::X], color: vec![Vec3::ONE] }.moved(0.0, 2.0, 0.0, std::f32::consts::FRAC_PI_2);
        assert!((g.pos[0] - Vec3::new(0.0, 2.0, -1.0)).length() < 1e-6, "{:?}", g.pos[0]);
        assert!((g.normal[0] - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-6);
    }
}
