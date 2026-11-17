//! One frame's draws (scene.ts advance and render): everything on the card,
//! placed for the clock, in the order three draws it: what is solid first by
//! renderOrder, then what lets light through by renderOrder, far to near.

use std::collections::HashMap;

use glam::{Mat4, Vec3};

use super::clay::Clay;
use super::gpu::Vertex;
use super::layout::{VIEW, Tower, lift_at};
use super::lens::Lens;
use super::life::{Kind, Rising, mark_now};
use super::people_desk::plane_at;
use super::people_pose::{hop_at, pose, rig};
use super::render_types::{Draw, Look, MeshId};
use super::setting::facing;
use super::world::{Part, Place, World};
use super::world_in::OfficeIn;

/// How far the hovered person rises.
const HOVER_RISE: f64 = 0.14;

/// What the frame depends on besides the world.
pub struct Moment<'a> {
    pub now: f64,
    /// Reduced motion, or nothing to animate: everyone holds still.
    pub still: bool,
    pub hovered: Option<&'a str>,
    /// When each finisher's hop began.
    pub hops: &'a HashMap<String, f64>,
    pub lens: &'a Lens,
    /// The mark's scale over someone who needs you (lens::mark_scale).
    pub mark: f32,
    /// Sky and town shifted behind the tower by the pointer.
    pub parallax: (f32, f32),
    /// The lit rooms' haze, by how far the camera is.
    pub haze: f32,
    /// Thrown light is drawn (quality).
    pub glow: bool,
    pub reduced: bool,
    /// The rising marks this frame: steam and bytes.
    pub rising: (MeshId, MeshId),
}

fn clay_draw(mut d: Draw, clay: Clay, roughness: f32) -> Draw {
    let room = super::geom::hex(clay.self_) * clay.self_level;
    let rim = super::geom::hex(clay.rim) * clay.rim_level;
    d.room = [room.x, room.y, room.z, clay.foot];
    d.rim = [rim.x, rim.y, rim.z, roughness];
    d
}

fn place(p: Place, parallax: (f32, f32)) -> Mat4 {
    match p {
        Place::World => Mat4::IDENTITY,
        Place::Far => Mat4::from_translation(Vec3::new(parallax.0, parallax.1, 0.0)) * Mat4::from_rotation_y(facing()),
        Place::Field => Mat4::from_rotation_y(facing()),
        Place::Up(y) => Mat4::from_translation(Vec3::new(0.0, y, 0.0)),
    }
}

fn of_part(p: &Part, parallax: (f32, f32)) -> Draw {
    let mut d = Draw::new(p.mesh, p.look);
    d.model = place(p.place, parallax);
    d.tint = p.tint;
    d.texture = p.texture;
    d.falls_back = p.falls_back;
    d.cast = p.cast;
    if p.look == Look::Glass {
        d.solid_tint = true;
        return clay_draw(d, Clay { self_: "#000000", self_level: 0.0, rim: "#000000", rim_level: 0.0, foot: 0.0 }, 0.15);
    }
    match p.clay {
        Some(c) => clay_draw(d, c, 0.82),
        None => d,
    }
}

/// Object3D.lookAt for a thing (not a camera): its +z toward `dir`, rolled so `up` stays up.
fn facing_along(at: Vec3, dir: Vec3, up: Vec3) -> Mat4 {
    let z = dir.normalize_or_zero();
    let mut x = up.cross(z);
    if x.length_squared() < 1e-12 {
        x = Vec3::X;
    }
    let x = x.normalize();
    let y = z.cross(x);
    Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), at.extend(1.0))
}

/// The marks' quads for this moment: (steam, bytes).
pub fn rising_quads(marks: &[Rising], time: f64) -> (Vec<Vertex>, Vec<Vertex>) {
    let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let (mut steam, mut bytes) = (Vec::new(), Vec::new());
    for m in marks {
        let at = mark_now(m, time);
        let strength = if m.kind == Kind::Steam { 0.42 } else { 0.85 };
        let colour = (at.colour * (strength * at.fade) as f32).to_array();
        let out = if m.kind == Kind::Steam { &mut steam } else { &mut bytes };
        out.extend(corners.map(|(x, y)| Vertex { pos: at.at.as_vec3().to_array(), normal: [x, y, at.half as f32], color: colour }));
    }
    (steam, bytes)
}

/// Every draw of the frame, in order.
pub fn draws(w: &World, m: &OfficeIn, t: &Tower, at: &Moment) -> Vec<Draw> {
    let mut list: Vec<(i32, f32, Draw)> = Vec::new();
    let mut push = |order: i32, d: Draw| list.push((order, 0.0, d));
    let parts = w.setting.iter().chain(w.roof.iter()).chain(w.floor_parts()).chain(w.top.iter());
    for p in parts.filter(|p| p.on) {
        if matches!(p.look, Look::Light | Look::LightOver) && !at.glow {
            continue;
        }
        let mut d = of_part(p, at.parallax);
        if p.look == Look::LightOver {
            d.tint[3] = at.haze;
        }
        push(p.order, d);
    }
    let clock = if at.still { 0.0 } else { at.now };
    if let Some((mesh, start, stops)) = &w.car {
        let stops: Vec<f64> = stops.iter().map(|&s| s as f64).collect();
        let y = if at.still { start.y } else { lift_at(at.now, &stops) as f32 };
        let mut d = Draw::new(*mesh, Look::Flat);
        d.model = Mat4::from_translation(Vec3::new(start.x, y, start.z));
        push(0, d);
    }
    if let Some((mesh, spot, turn)) = w.logo {
        let mut d = Draw::new(mesh, Look::Picture);
        d.model = Mat4::from_translation(spot) * Mat4::from_rotation_y(turn);
        d.texture = w.logo_tex;
        d.falls_back = true;
        push(0, d);
    }
    for b in &w.bodies {
        let hop = at.hops.get(&b.id).map_or(0.0, |s| hop_at(at.now - s));
        let raise = hop + if at.hovered == Some(b.id.as_str()) { HOVER_RISE } else { 0.0 };
        let r = rig(b.seat, b.y, &pose(b.state, clock, b.seat_no), raise, b.marked);
        if !r.visible {
            continue;
        }
        let part = |mesh: MeshId, model: glam::DMat4, cast: bool| {
            let mut d = clay_draw(Draw::new(mesh, Look::Lit), b.clay, 0.82);
            d.model = model.as_mat4();
            d.falls_back = true;
            d.cast = cast;
            if b.clay.self_ == super::clay::INDOOR.self_ {
                d.tint = super::geom::hex("#fff1d8").extend(1.0).to_array();
            }
            d
        };
        push(0, part(b.torso, r.trunk, true));
        push(0, part(b.legs, r.legs, true));
        push(0, part(b.arm_l, r.arm_l, false));
        push(0, part(b.arm_r, r.arm_r, false));
        let mut badge = Draw::new(w.badge, Look::Picture);
        badge.model = r.trunk.as_mat4();
        badge.texture = b.badge;
        badge.tint = b.badge_ink.extend(1.0).to_array();
        badge.falls_back = true;
        push(0, badge);
        if let Some(bubble) = r.bubble {
            let mut d = Draw::new(w.bubble, Look::Over);
            d.model = bubble.as_mat4() * Mat4::from_scale(Vec3::splat(at.mark));
            push(3, d);
        }
    }
    let viewer = Vec3::new(VIEW.x as f32, VIEW.y as f32, VIEW.z as f32).normalize();
    for f in &m.flights {
        let spot = plane_at(t, m.floor_of(&f.from), m.floor_of(&f.to), f.at, at.now, at.reduced);
        if let Some(s) = spot {
            let mut d = Draw::new(w.plane, Look::FlatBoth);
            d.model = facing_along(s.at.as_vec3(), s.dir.as_vec3(), viewer);
            push(0, d);
        }
    }
    if !at.still {
        let mut steam = Draw::new(at.rising.0, Look::Mark);
        steam.round = true;
        push(2, steam);
        push(2, Draw::new(at.rising.1, Look::Mark));
    }
    // Far to near among what lets light through.
    let view = at.lens.view.as_mat4();
    for (_, depth, d) in &mut list {
        *depth = view.transform_point3(d.model.w_axis.truncate()).z;
    }
    let see_through = |l: Look| matches!(l, Look::Glass | Look::Picture | Look::Light | Look::LightOver | Look::Over | Look::Star | Look::Mark);
    let (mut solid, mut clear): (Vec<_>, Vec<_>) = list.into_iter().partition(|(_, _, d)| !see_through(d.look));
    solid.sort_by_key(|(o, _, _)| *o);
    clear.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    solid.into_iter().chain(clear).map(|(_, _, d)| d).collect()
}
