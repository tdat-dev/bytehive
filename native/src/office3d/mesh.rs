//! Geometry with its colour on the vertices, so one draw takes a whole floor:
//! the office's way (geometry.ts). The spike needs only boxes.

use glam::{Mat4, Vec3};

use super::gpu::Vertex;

/// A box `size` big around `at`, every face `color`, two triangles a face.
pub fn push_box(out: &mut Vec<Vertex>, at: Vec3, size: Vec3, color: [f32; 3]) {
    let h = size * 0.5;
    // Each face: its normal, then its four corners counter-clockwise from outside.
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[1.0, -1.0, 1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0]]),
        ([-1.0, 0.0, 0.0], [[-1.0, -1.0, -1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0]]),
        ([0.0, 1.0, 0.0], [[-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0]]),
        ([0.0, -1.0, 0.0], [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0], [-1.0, -1.0, 1.0]]),
        ([0.0, 0.0, 1.0], [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]]),
        ([0.0, 0.0, -1.0], [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]]),
    ];
    for (normal, corners) in faces {
        let p = corners.map(|c| (at + Vec3::from(c) * h).to_array());
        for i in [0, 1, 2, 0, 2, 3] {
            out.push(Vertex { pos: p[i], normal, color });
        }
    }
}

/// The spike's stand-in office: a few floors of desks on slabs.
pub fn spike_scene() -> Vec<Vertex> {
    let mut out = Vec::new();
    let colors = [[0.85, 0.62, 0.38], [0.42, 0.62, 0.82], [0.55, 0.78, 0.55], [0.86, 0.48, 0.52], [0.75, 0.70, 0.86]];
    for floor in 0..4 {
        let y = floor as f32 * 3.0;
        push_box(&mut out, Vec3::new(0.0, y - 0.15, 0.0), Vec3::new(14.0, 0.3, 10.0), [0.80, 0.78, 0.74]);
        for i in 0..10 {
            for j in 0..6 {
                let at = Vec3::new(-5.4 + i as f32 * 1.2, y + 0.4, -3.6 + j as f32 * 1.4);
                push_box(&mut out, at, Vec3::new(0.9, 0.8, 0.6), colors[(i + j + floor) % colors.len()]);
            }
        }
    }
    out
}

/// Look at the middle of the tower from a camera turning around it: the
/// view-projection and where the camera stands.
pub fn spike_view(t: f32, aspect: f32) -> (Mat4, Vec3) {
    let eye = Vec3::new(t.cos() * 22.0, 14.0, t.sin() * 22.0);
    (Mat4::perspective_rh(40f32.to_radians(), aspect.max(0.1), 0.5, 200.0) * Mat4::look_at_rh(eye, Vec3::new(0.0, 4.0, 0.0), Vec3::Y), eye)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_is_twelve_triangles_facing_out() {
        let mut v = Vec::new();
        push_box(&mut v, Vec3::ZERO, Vec3::ONE, [1.0, 1.0, 1.0]);
        assert_eq!(v.len(), 36);
        for tri in v.chunks(3) {
            let (a, b, c) = (Vec3::from(tri[0].pos), Vec3::from(tri[1].pos), Vec3::from(tri[2].pos));
            let face = (b - a).cross(c - a).normalize();
            assert!(face.dot(Vec3::from(tri[0].normal)) > 0.99, "winding agrees with the normal");
        }
    }
}
