//! What the pointer is over, as scene.ts finds it: every pickable thing (a
//! person, a floor, the roof, a board) has an invisible box (its proxy), a ray
//! goes from the camera through the pointer, and the nearest box it enters
//! wins. Three's Raycaster against box meshes, without the meshes.

use glam::{Mat4, Vec2, Vec3};

/// A proxy: an axis-aligned box `size` big around `at`, and what it stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct Proxy<T> {
    pub at: Vec3,
    pub size: Vec3,
    pub what: T,
}

/// The ray from the camera at `eye` through `ndc` (x right, y up, -1 to 1),
/// as Raycaster.setFromCamera: it starts at the camera, so distances are the
/// camera's.
pub fn ray(view_proj: Mat4, eye: Vec3, ndc: Vec2) -> (Vec3, Vec3) {
    let far = view_proj.inverse().project_point3(Vec3::new(ndc.x, ndc.y, 1.0));
    (eye, (far - eye).normalize())
}

/// How far along the ray it enters the box, if it does (the slab method).
fn enters(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv = dir.recip();
    let (a, b) = ((min - origin) * inv, (max - origin) * inv);
    let (lo, hi) = (a.min(b), a.max(b));
    let near = lo.max_element();
    let far = hi.min_element();
    (far >= near.max(0.0)).then_some(near.max(0.0))
}

/// The nearest proxy under the pointer, and how far it is.
pub fn pick<T>(proxies: &[Proxy<T>], view_proj: Mat4, eye: Vec3, ndc: Vec2) -> Option<(&Proxy<T>, f32)> {
    let (origin, dir) = ray(view_proj, eye, ndc);
    proxies
        .iter()
        .filter_map(|p| enters(origin, dir, p.at - p.size * 0.5, p.at + p.size * 0.5).map(|d| (p, d)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EYE: Vec3 = Vec3::new(0.0, 2.0, 12.0);

    fn camera() -> Mat4 {
        Mat4::perspective_rh(40f32.to_radians(), 1.5, 0.5, 200.0) * Mat4::look_at_rh(EYE, Vec3::new(0.0, 2.0, 0.0), Vec3::Y)
    }

    #[test]
    fn the_nearest_box_under_the_pointer_wins() {
        let proxies = vec![
            Proxy { at: Vec3::new(0.0, 2.0, 0.0), size: Vec3::splat(1.0), what: "far" },
            Proxy { at: Vec3::new(0.0, 2.0, 4.0), size: Vec3::splat(1.0), what: "near" },
            Proxy { at: Vec3::new(5.0, 2.0, 0.0), size: Vec3::splat(1.0), what: "aside" },
        ];
        let (hit, d) = pick(&proxies, camera(), EYE, Vec2::ZERO).unwrap();
        assert_eq!(hit.what, "near");
        assert!((d - 7.5).abs() < 0.05, "{d}");
    }

    #[test]
    fn nothing_under_the_pointer_is_no_pick() {
        let proxies = vec![Proxy { at: Vec3::new(0.0, 2.0, 0.0), size: Vec3::splat(1.0), what: 1 }];
        assert!(pick(&proxies, camera(), EYE, Vec2::new(0.9, 0.9)).is_none());
    }

    #[test]
    fn a_box_behind_the_camera_is_not_seen() {
        let proxies = vec![Proxy { at: Vec3::new(0.0, 2.0, 20.0), size: Vec3::splat(1.0), what: 1 }];
        assert!(pick(&proxies, camera(), EYE, Vec2::ZERO).is_none());
    }
}
