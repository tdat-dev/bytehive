//! One desk as its person leaves it (build.ts deskFor): the desk and chair,
//! the screen, a thing of their own, a prop set down, the review papers and,
//! for someone who needs you, the lamp; split into what is matte, what holds
//! its own light, and the light it throws.

use std::f32::consts::PI;

use glam::{Mat4, Vec3};

use super::build_parts::{CHAIR_HINGE, DESK_TOP, chair_back_geometry, chair_geometry, desk_geometry, desk_thing};
use super::colour::hex_of;
use super::geom::Geom;
use super::glow::flare;
use super::layout::{Seat, chair_spot};
use super::palette::office as o;
use super::people_desk::{DeskGlow, desk_glow, desk_prop, lamp_geometry, paper_stack, screen_light};
use super::people_pose::{Screen, State, pose};
use super::people_shapes::Looks;
use super::prims::plane;
use super::seed::hash_of;

/// Someone at a desk: how they look, how they are, and their seat number.
pub struct Seated<'a> {
    pub looks: Looks<'a>,
    pub state: State,
    pub seat: f64,
}

#[derive(Default)]
pub struct DeskParts {
    pub solid: Vec<Geom>,
    pub lit: Vec<Geom>,
    pub thrown: Vec<Geom>,
}

fn on_desk(g: Geom, d: Seat) -> Geom {
    g.moved(d.x as f32, 0.0, d.z as f32, d.rot as f32)
}

/// A lit screen, and at work a lit panel on its back: from a desk facing the
/// viewer that is all that shows.
fn screen(d: Seat, state: Screen) -> Geom {
    let colour = hex_of(screen_light(state).as_dvec3());
    let mut parts = vec![plane(0.58, 0.36).tint(&colour, None).transform(Mat4::from_translation(Vec3::new(0.0, DESK_TOP + 0.33, -0.2 + 0.03)))];
    if state == Screen::Work {
        parts.push(plane(0.46, 0.28).tint(&colour, None).transform(Mat4::from_rotation_y(PI)).transform(Mat4::from_translation(Vec3::new(0.0, DESK_TOP + 0.33, -0.2 - 0.03))));
    }
    on_desk(Geom::merged(parts), d)
}

fn chair_parts(d: Seat, lean: f64) -> [Geom; 2] {
    let (cx, cz) = chair_spot(d);
    let back = chair_back_geometry().transform(Mat4::from_translation(Vec3::new(0.0, CHAIR_HINGE.0, CHAIR_HINGE.1)) * Mat4::from_rotation_x(lean as f32));
    let at = |g: Geom| g.moved(cx as f32, 0.0, cz as f32, d.rot as f32);
    [at(chair_geometry()), at(back)]
}

/// The desk at `d`, for `p` or for nobody, with `papers` to review on it.
pub fn desk_for(d: Seat, p: Option<&Seated>, papers: usize) -> DeskParts {
    let look = match p {
        Some(p) => pose(p.state, 0.0, p.seat),
        None => pose(State::Stopped, 0.0, 0.0),
    };
    let mut out = DeskParts { solid: vec![on_desk(desk_geometry(), d)], lit: vec![screen(d, look.screen)], thrown: vec![] };
    out.solid.extend(chair_parts(d, look.lean));
    if look.screen == Screen::Work {
        out.thrown.push(on_desk(desk_glow(DeskGlow::Work), d));
        let front = -0.1;
        out.thrown.push(flare((d.x + d.rot.sin() * front) as f32, DESK_TOP + 0.36, (d.z + d.rot.cos() * front) as f32, 1.5, o::SCREEN_ON, 0.34));
    }
    let prop = p.and_then(|p| desk_prop(p.looks.prop));
    let aside = prop.is_some();
    if let Some(g) = prop {
        out.solid.push(on_desk(g, d));
    }
    if let Some(p) = p {
        out.solid.push(on_desk(desk_thing(hash_of(p.looks.id)), d));
    }
    if papers > 0 {
        out.solid.push(on_desk(paper_stack(papers, aside), d));
    }
    if look.lamp {
        let (solid, lit) = lamp_geometry();
        out.solid.push(on_desk(solid, d));
        out.lit.push(on_desk(lit, d));
        out.thrown.push(on_desk(desk_glow(DeskGlow::Lamp), d));
    }
    out
}
