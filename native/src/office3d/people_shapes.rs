//! The people's shapes (people.ts): chunky toys with bead heads, big enough to
//! read from the street. Each is a torso with the head, legs, two arms (one may
//! hold a prop), a badge and, over someone who needs you, the honey mark. What
//! moves on its own is its own shape, set by the rig in people_pose.rs.

use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use super::colour::{hex_of, linear};
use super::geom::Geom;
use super::lathe::capsule;
use super::palette::{BEADS, office as o};
use super::people_pose::{ARM_SIT, HAND, HEAD_R, HEAD_UP};
use super::prims::{cone, plane, sphere_part, torus};
use super::props::{ball, bx, cyl, mv, scaled, soft};
use super::seed::hash_of;
use crate::company::types::Rank;

/// The head is modelled at this size and scaled up to HEAD_R.
const HEAD_MODEL: f32 = 0.41;
const INK: &str = "#14161c";
const CARD: &str = "#f6f3ec";
const TROUSERS: &str = "#2b2f3a";
pub const BADGE: (f32, f32) = (0.46, 0.285);

/// What a person looks like, from their role and the company file.
pub struct Looks<'a> {
    pub id: &'a str,
    pub rank: &'a Rank,
    pub shirt: &'a str,
    pub hair: &'a str,
    pub prop: &'a str,
}

/// A person's skin, the same every time for the same id.
fn bead_of(id: &str) -> &'static str {
    BEADS[hash_of(id) as usize % BEADS.len()]
}

fn rx(g: Geom, a: f32) -> Geom {
    g.transform(Mat4::from_rotation_x(a))
}

fn rz(g: Geom, a: f32) -> Geom {
    g.transform(Mat4::from_rotation_z(a))
}

fn caps(r: f64, len: f64, cap: u32, radial: u32, hex: &str) -> Geom {
    capsule(r, len, cap, radial).tint(hex, None)
}

fn dome(hex: &str, tip: f32) -> Geom {
    rx(sphere_part(HEAD_MODEL + 0.018, 22, 10, (0.0, PI * 2.0), (0.0, PI / 2.0)).tint(hex, None), tip)
}

/// Six kinds of hair. All of them keep clear of the face, which is on -z.
fn hair(kind: u32, hex: &str) -> Vec<Geom> {
    let lump = |r: f32, x: f32, y: f32, z: f32| mv(ball(r, 12, 9, hex), x, y, z);
    match kind {
        0 => vec![dome(hex, 0.32)],
        1 => vec![dome(hex, 0.32), mv(scaled(ball(0.19, 12, 8, hex), 1.5, 0.6, 1.0), -0.13, 0.31, -0.25)],
        2 => vec![dome(hex, 0.32), lump(0.17, 0.0, 0.42, 0.24)],
        3 => vec![dome(hex, 0.36), lump(0.2, -0.3, 0.05, 0.1), lump(0.2, 0.3, 0.05, 0.1), lump(0.22, 0.0, 0.26, 0.26)],
        4 => vec![dome(hex, 0.5), sphere_part(HEAD_MODEL + 0.03, 20, 9, (-0.35, PI + 0.7), (PI / 2.0 - 0.3, PI / 2.4)).tint(hex, None)],
        _ => vec![dome(hex, 0.68), rx(sphere_part(HEAD_MODEL + 0.014, 18, 5, (0.0, PI * 2.0), (PI * 0.8, PI * 0.2)).tint(hex, None), 0.62)],
    }
}

fn head(p: &Looks) -> Geom {
    let face = HEAD_MODEL + 0.002;
    let smile = rz(torus(0.075, 0.014, 6, 10, PI).tint(INK, None), PI);
    let cheek = || scaled(ball(0.06, 8, 6, "#e88f7a"), 1.0, 0.7, 0.4);
    let mut parts = vec![ball(HEAD_MODEL, 22, 16, bead_of(p.id))];
    parts.extend(hair(hash_of(p.id) % 6, p.hair));
    parts.extend([
        mv(ball(0.048, 8, 6, INK), -0.14, 0.06, -face + 0.04),
        mv(ball(0.048, 8, 6, INK), 0.14, 0.06, -face + 0.04),
        mv(smile, 0.0, -0.06, -face + 0.014),
        mv(cheek(), -0.23, -0.04, -face + 0.09),
        mv(cheek(), 0.23, -0.04, -face + 0.09),
    ]);
    let k = (HEAD_R / HEAD_MODEL as f64) as f32;
    scaled(Geom::merged(parts), k, k, k)
}

pub fn torso_geometry(p: &Looks) -> Geom {
    let staff = matches!(p.rank, Rank::Staff);
    let collar = if staff { hex_of(linear(p.shirt).lerp(linear("#ffffff"), 0.4)) } else { "#f4efe6".to_string() };
    let mut parts = vec![mv(caps(0.29, 0.24, 6, 14, p.shirt), 0.0, 0.34, 0.0), mv(soft(0.32, 0.09, 0.26, 0.04, &collar), 0.0, 0.6, -0.03)];
    if !staff {
        parts.push(mv(bx(0.08, 0.32, 0.02, "#3a4670"), 0.0, 0.4, -0.29));
    }
    parts.push(mv(head(p), 0.0, HEAD_UP as f32, 0.0));
    parts.push(mv(bx(0.22, 0.24, 0.015, CARD), 0.0, BADGE.0, -BADGE.1 + 0.012));
    Geom::merged(parts)
}

pub fn legs_geometry() -> Geom {
    let leg = || caps(0.125, 0.4, 4, 10, TROUSERS);
    let shoe = || soft(0.22, 0.13, 0.34, 0.06, "#e8e4da");
    Geom::merged([mv(leg(), -0.135, -0.3, 0.0), mv(leg(), 0.135, -0.3, 0.0), mv(shoe(), -0.135, -0.6, -0.06), mv(shoe(), 0.135, -0.6, -0.06)])
}

/// What a prop looks like held out at the desk, set into the arm that swings it.
fn in_hand(g: Geom) -> Geom {
    g.transform(Mat4::from_translation(Vec3::new(0.0, HAND as f32, 0.0)) * Mat4::from_rotation_x(-ARM_SIT as f32))
}

fn clipboard() -> Geom {
    in_hand(rx(Geom::merged([bx(0.2, 0.28, 0.02, o::DESK_TOP), bx(0.17, 0.23, 0.03, o::PAPER), mv(bx(0.08, 0.035, 0.04, o::STEEL), 0.0, 0.135, 0.0)]), 0.5))
}

fn megaphone() -> Geom {
    let horn = rx(cone(0.15, 0.34, 14, 1, false).tint(o::PAPER, None), PI / 2.0);
    let ring = torus(0.15, 0.025, 6, 16, PI * 2.0).tint(o::STEEL, None);
    in_hand(Geom::merged([mv(horn, 0.0, 0.04, -0.14), mv(ring, 0.0, 0.04, -0.31)]))
}

fn magnifier() -> Geom {
    in_hand(Geom::merged([mv(torus(0.11, 0.024, 8, 18, PI * 2.0).tint(o::STEEL, None), 0.0, 0.22, 0.0), mv(cyl(0.024, 0.03, 0.16, 8, o::DESK_TOP), 0.0, 0.05, 0.0)]))
}

fn writer(body: &str, tip: &str) -> Geom {
    let point = rx(cone(0.024, 0.07, 8, 1, false).tint(tip, None), PI);
    in_hand(rx(Geom::merged([mv(cyl(0.024, 0.024, 0.28, 8, body), 0.0, 0.03, 0.0), mv(point, 0.0, -0.145, 0.0)]), 0.35))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    L,
    R,
}

/// A prop held in a hand: which hand, and its shape.
fn held(prop: &str) -> Option<(Hand, Geom)> {
    Some(match prop {
        "clipboard" => (Hand::L, clipboard()),
        "megaphone" => (Hand::R, megaphone()),
        "magnifier" => (Hand::R, magnifier()),
        "pen" => (Hand::R, writer(o::STEEL, o::NOTE_DOING)),
        "pencil" => (Hand::R, writer("#c4605a", INK)),
        _ => return None,
    })
}

pub fn arm_geometry(p: &Looks, hand: Hand) -> Geom {
    let mut parts = vec![mv(caps(0.1, 0.34, 4, 10, p.shirt), 0.0, -0.26, 0.0), mv(ball(0.115, 10, 8, bead_of(p.id)), 0.0, (HAND + 0.02) as f32, 0.0)];
    if hand == Hand::L && matches!(p.rank, Rank::Lead) {
        parts.push(mv(cyl(0.1, 0.1, 0.07, 14, o::PAPER), 0.0, -0.17, 0.0));
    }
    if let Some((h, g)) = held(p.prop)
        && h == hand
    {
        parts.push(g);
    }
    Geom::merged(parts)
}

/// The honey mark over someone who needs you, its foot at the origin and a
/// dark edge all round, so it holds against a lit room and the sky alike.
pub fn bubble_geometry() -> Geom {
    let pill_at = 0.62;
    let pill = |r: f64, hex: &str| scaled(rz(caps(r, 0.34, 6, 20, hex), PI / 2.0), 1.0, 1.0, 0.4);
    let tail = |r: f32, hex: &str| rx(cone(r, r * 1.9, 4, 1, false).tint(hex, None), PI);
    Geom::merged([
        mv(tail(0.19, INK), 0.0, 0.2, 0.0),
        mv(pill(0.34, INK), 0.0, pill_at, 0.0),
        mv(tail(0.13, o::HONEY), 0.0, 0.24, 0.0),
        mv(pill(0.27, o::HONEY), 0.0, pill_at, 0.0),
        mv(bx(0.085, 0.24, 0.02, INK), 0.0, pill_at + 0.06, 0.12),
        mv(ball(0.056, 12, 8, INK), 0.0, pill_at - 0.15, 0.12),
    ])
}

/// The badge's plane, for the CLI's logo, on the torso's frame.
pub fn badge_geometry() -> Geom {
    plane(0.17, 0.17).transform(Mat4::from_rotation_y(PI)).transform(Mat4::from_translation(Vec3::new(0.0, BADGE.0, -BADGE.1))).tint("#ffffff", None)
}

/// A person's shapes, made once for each look.
pub struct Shapes {
    pub torso: Geom,
    pub legs: Geom,
    pub arm_l: Geom,
    pub arm_r: Geom,
}

pub fn shapes_of(p: &Looks) -> Shapes {
    Shapes { torso: torso_geometry(p), legs: legs_geometry(), arm_l: arm_geometry(p, Hand::L), arm_r: arm_geometry(p, Hand::R) }
}
