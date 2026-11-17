//! What stands on a desk for its person (people.ts), in the desk's own place:
//! a prop set down, the review papers, the lamp of someone who needs you, the
//! light on the desk top and the screen's colour; and the paper plane that
//! carries a message between floors.

use glam::{DVec3, Mat4, Vec3};

use super::build_parts::DESK_TOP;
use super::geom::{Geom, hex};
use super::glow::pool;
use super::layout::{FLIGHT_MS, Tower, flight_point};
use super::palette::office as o;
use super::people_pose::Screen;
use super::props::{bx, cyl, mv};
use super::prims::cone;

/// A prop that is set down rather than held.
pub fn desk_prop(prop: &str) -> Option<Geom> {
    match prop {
        "laptop" => {
            let lid = bx(0.44, 0.3, 0.02, o::STEEL).transform(Mat4::from_translation(Vec3::new(0.0, 0.15, 0.0))).transform(Mat4::from_rotation_x(-0.26));
            Some(Geom::merged([mv(bx(0.44, 0.022, 0.3, o::STEEL), 0.4, DESK_TOP + 0.011, 0.12), mv(lid, 0.4, DESK_TOP + 0.02, -0.03)]))
        }
        "book" => {
            let leaf = |s: f32| mv(bx(0.2, 0.04, 0.28, o::PAPER).transform(Mat4::from_rotation_z(s * 0.09)), 0.4 + s * 0.1, DESK_TOP + 0.03, 0.12);
            Some(Geom::merged([leaf(-1.0), leaf(1.0)]))
        }
        _ => None,
    }
}

/// In the corner the viewer can see past someone standing. `aside` moves it
/// off a prop that has that corner.
pub fn paper_stack(n: usize, aside: bool) -> Geom {
    let x = if aside { -0.46 } else { 0.54 };
    Geom::merged((0..n).map(|i| bx(0.3, 0.024, 0.32, o::PAPER).moved(x, DESK_TOP + 0.014 + i as f32 * 0.03, -0.02, (i as f32 - 1.0) * 0.09)))
}

/// A small desk lamp: its base and stem are matte, its shade alight.
pub fn lamp_geometry() -> (Geom, Geom) {
    let (x, z) = (0.62, -0.29);
    let solid = Geom::merged([mv(cyl(0.09, 0.1, 0.03, 12, o::DESK_BODY), x, DESK_TOP + 0.015, z), mv(cyl(0.02, 0.02, 0.44, 8, o::DESK_BODY), x, DESK_TOP + 0.24, z)]);
    (solid, mv(cone(0.18, 0.18, 14, 1, false).tint(o::LAMP_NEEDS, None), x, DESK_TOP + 0.5, z))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeskGlow {
    Work,
    Lamp,
}

/// Light lying on a desk top: from far off this is what shows someone at work.
pub fn desk_glow(kind: DeskGlow) -> Geom {
    let (w, x, colour, level) = match kind {
        DeskGlow::Work => (1.44, 0.0, o::SCREEN_ON, 2.2),
        DeskGlow::Lamp => (1.0, 0.24, o::LAMP_NEEDS, 1.0),
    };
    pool(x, DESK_TOP + 0.006, 0.0, w, 0.76, colour, level)
}

/// A screen's colour, linear: lit parts hold their own light, so its level is in the colour.
pub fn screen_light(s: Screen) -> Vec3 {
    match s {
        Screen::Off => hex(o::SCREEN_OFF),
        Screen::Work => hex(o::SCREEN_ON) * 1.1,
        Screen::Dim => hex(o::SCREEN_ON) * 0.2,
    }
}

/// Three triangles, nose on +z: two wings and the fold under them.
pub fn plane_geometry() -> Geom {
    let nose = Vec3::new(0.0, 0.0, 0.6);
    let tail_l = Vec3::new(-0.36, 0.06, -0.42);
    let tail_r = Vec3::new(0.36, 0.06, -0.42);
    let mid = Vec3::new(0.0, 0.06, -0.28);
    let keel = Vec3::new(0.0, -0.14, -0.36);
    let pos = vec![nose, tail_l, mid, nose, mid, tail_r, nose, mid, keel];
    // Unlit in the office; a face's own normal here, for whoever wants one.
    let normal = pos.chunks(3).flat_map(|t| [(t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero(); 3]).collect();
    let paper = hex(o::PAPER);
    let fold = paper * 0.62;
    Geom { pos, normal, color: vec![paper, paper, paper, paper, paper, paper, fold, fold, fold] }
}

const NOSE: f64 = 0.01;

/// Where a plane is, and which way its nose points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneSpot {
    pub at: DVec3,
    pub dir: DVec3,
}

/// Where a flight's plane is `now`. Reduced motion holds it at its end for the
/// time the flight takes. Nothing flies inside one floor, or to someone who
/// has gone: a floor is Some(Some(id)), the roof Some(None), nobody None.
pub fn plane_at(t: &Tower, from: Option<Option<&str>>, to: Option<Option<&str>>, left: f64, now: f64, reduced: bool) -> Option<PlaneSpot> {
    let (from, to) = (from?, to?);
    if from == to {
        return None;
    }
    let k = (now - left) / FLIGHT_MS;
    if !(0.0..1.0).contains(&k) {
        return None;
    }
    let at = if reduced { 1.0 } else { k };
    let point = |k: f64| {
        let p = flight_point(t, from, to, k);
        DVec3::new(p.x, p.y, p.z)
    };
    let (a, b) = (point((at - NOSE).max(0.0)), point(at.max(NOSE)));
    Some(PlaneSpot { at: point(at), dir: b - a })
}
