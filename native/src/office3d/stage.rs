//! What the scene knows besides the picture (scene.ts): the boxes a click
//! lands in, what a click means, where the words go over the picture, and the
//! band of heights that stays lit at a Floor. All from the tower and who is in
//! it, so nothing here needs the GPU.

use std::collections::{HashMap, HashSet};

use glam::{DVec3, Vec3};

use super::build_parts::{SLAB_D, SLAB_W, SLAB_X};
use super::floor::BOARD_EDGE;
use super::layout::{FOUNDER_DESK, Level, ROOM_D, SLAB, STOREY, Seat, Tower};
use super::lens::{Lens, MARK_PX, css_point};
use super::people_pose::{Spot, State, spot_of};
use super::pick::Proxy;

const PERSON_BOX: (f64, f64) = (0.9, 0.9);
/// A pick box reaches this far over the head.
const HEAD_ROOM: f64 = 0.5;
const BUBBLE_BOX: (f64, f64) = (1.1, 1.0);
const TAG_Y: f64 = 1.5;

/// Who is in the tower, as the scene reads it.
#[derive(Clone, Debug, Default)]
pub struct Cast {
    pub founder: Option<(String, State)>,
    pub states: HashMap<String, State>,
    /// Everyone a Needs you row points at, and everyone asking something.
    pub marked: HashSet<String>,
    /// Floors with a board.
    pub boards: HashSet<String>,
}

impl Cast {
    pub fn state_of(&self, id: &str) -> State {
        match &self.founder {
            Some((f, s)) if f == id => *s,
            _ => self.states.get(id).copied().unwrap_or(State::Stopped),
        }
    }
}

/// What a box stands for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct What {
    pub person: Option<String>,
    pub floor: Option<String>,
    /// A mark over someone, drawn over everything: it takes the click first.
    pub over: bool,
}

fn floor_box(y: f64, what: What) -> Proxy<What> {
    let (x, w, h, d) = (SLAB_X as f64, SLAB_W as f64, STOREY, SLAB_D as f64);
    Proxy { at: Vec3::new(x as f32, (y + h / 2.0) as f32, 0.0), size: Vec3::new(w as f32, h as f32, d as f32), what }
}

fn person_boxes(out: &mut Vec<Proxy<What>>, cast: &Cast, d: Seat, y: f64, what: What) {
    let id = what.person.clone().unwrap_or_default();
    let s = spot_of(d, cast.state_of(&id));
    let h = s.head + HEAD_ROOM;
    out.push(Proxy { at: Vec3::new(s.x as f32, (y + h / 2.0) as f32, s.z as f32), size: Vec3::new(PERSON_BOX.0 as f32, h as f32, PERSON_BOX.1 as f32), what: what.clone() });
    if cast.marked.contains(&id) {
        let at = Vec3::new(s.x as f32, (y + s.bubble + BUBBLE_BOX.1 / 2.0) as f32, s.z as f32);
        out.push(Proxy { at, size: Vec3::new(BUBBLE_BOX.0 as f32, BUBBLE_BOX.1 as f32, BUBBLE_BOX.0 as f32), what: What { over: true, ..what } });
    }
}

/// Every box a click can land in: each floor, everyone at a desk, and all of the top for the Founder.
pub fn proxies(t: &Tower, cast: &Cast) -> Vec<Proxy<What>> {
    let mut out = Vec::new();
    for spot in &t.floors {
        out.push(floor_box(spot.y, What { floor: Some(spot.id.clone()), ..Default::default() }));
        for d in &spot.desks {
            person_boxes(&mut out, cast, d.seat, spot.y, What { person: Some(d.person_id.clone()), floor: Some(spot.id.clone()), over: false });
        }
    }
    if let Some((id, _)) = &cast.founder {
        out.push(floor_box(t.roof_y, What { person: Some(id.clone()), ..Default::default() }));
    }
    out
}

/// What a ray means: every box it enters, with how far. Everyone stands inside
/// a floor's box, so the first thing it enters is the floor it can see; a mark
/// is drawn over everything, so it takes the click over everything too.
pub fn pick_from(hits: &[(What, f64)]) -> Option<What> {
    let mut near: Vec<&(What, f64)> = hits.iter().collect();
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((w, _)) = near.iter().find(|h| h.0.over) {
        return Some(What { over: false, ..w.clone() });
    }
    let first = &near.first()?.0;
    if first.person.is_some() && first.floor.is_none() {
        return Some(What { person: first.person.clone(), ..Default::default() });
    }
    match near.iter().find(|h| h.0.person.is_some() && h.0.floor == first.floor) {
        Some((w, _)) => Some(What { over: false, ..w.clone() }),
        None => Some(What { floor: first.floor.clone(), ..Default::default() }),
    }
}

/// What stays lit: all of it at the Building, the one floor or the roof below
/// that. (lowest height, highest height, how far the rest falls back).
pub fn band_for(l: &Level, t: &Tower) -> DVec3 {
    let floor = match l {
        Level::Building => return DVec3::new(-1000.0, 1000.0, 0.0),
        Level::Floor(id) => Some(id),
        Level::Desk { floor, .. } => floor.as_ref(),
    };
    match floor.and_then(|id| t.floors.iter().position(|f| &f.id == id)) {
        None => DVec3::new(t.roof_y - SLAB, 1000.0, 1.0),
        Some(i) => {
            let y = t.floors[i].y;
            DVec3::new(if i == 0 { -1000.0 } else { y - SLAB }, y + STOREY - SLAB, 1.0)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorKind {
    Floor,
    Person,
    Top,
    Board,
}

/// Where a word goes, in CSS pixels on the canvas. `clear`: a mark hangs this
/// far below the point, and no tag should cover it.
#[derive(Clone, Debug, PartialEq)]
pub struct Anchor {
    pub id: String,
    pub kind: AnchorKind,
    pub x: f64,
    pub y: f64,
    pub visible: bool,
    pub clear: Option<f64>,
}

/// Every floor's name, every board, everyone's name tag, and the top's.
pub fn anchors(t: &Tower, cast: &Cast, lens: &Lens, w: f64, h: f64) -> Vec<Anchor> {
    let edge = (SLAB_X as f64 + SLAB_W as f64 / 2.0, -ROOM_D / 2.0);
    let at = |id: &str, kind: AnchorKind, x: f64, y: f64, z: f64| {
        let (cx, cy, visible) = css_point(lens.project(DVec3::new(x, y, z)), w, h);
        Anchor { id: id.into(), kind, x: cx, y: cy, visible, clear: None }
    };
    // A name goes over the head, and over the mark when there is one: that is what says it needs you.
    let someone = |id: &str, s: Spot, y: f64| {
        let marked = cast.marked.contains(id);
        let a = at(id, AnchorKind::Person, s.x, y + if marked { s.bubble } else { s.head + HEAD_ROOM }, s.z);
        if marked { Anchor { y: a.y - MARK_PX, clear: Some(MARK_PX), ..a } } else { a }
    };
    let mut out = Vec::new();
    for spot in &t.floors {
        out.push(at(&spot.id, AnchorKind::Floor, edge.0, spot.y + TAG_Y, edge.1));
        if cast.boards.contains(&spot.id) {
            out.push(at(&spot.id, AnchorKind::Board, BOARD_EDGE.0 as f64, spot.y + BOARD_EDGE.1 as f64, BOARD_EDGE.2 as f64));
        }
        for d in &spot.desks {
            out.push(someone(&d.person_id, spot_of(d.seat, cast.state_of(&d.person_id)), spot.y));
        }
    }
    if let Some((id, state)) = &cast.founder {
        out.push(someone(id, spot_of(FOUNDER_DESK, *state), t.roof_y));
    }
    out.push(at("top", AnchorKind::Top, edge.0, t.roof_y + TAG_Y, edge.1));
    out
}
