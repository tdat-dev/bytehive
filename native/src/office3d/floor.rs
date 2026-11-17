//! A floor of the tower and the top of it (build.ts roomSteps, buildFloor and
//! buildTop). The room is what does not change while people work: the shell,
//! the furniture, the tiles and walls with the room's light baked in. The
//! floor puts the desks and the board on it and says where everyone sits.
//! Everything is in the floor's own frame; the renderer lifts it by `spot.y`.

use super::build_parts::{BoardCounts, ROOM_H, WALL, board_geometry, note_parts, shell_geometry};
use super::desk::{DeskParts, Seated, desk_for};
use super::furnish::{RoomPlan, furnished, room_ambient};
use super::geom::Geom;
use super::glow::flare;
use super::layout::{FOUNDER_DESK, FloorSpot, GROUND, LIFT_W, ROOM_D, STOREY, Seat, Tower, chair_spot};
use super::light::{RoomLight, bake, desk_blockers};
use super::palette::office as o;
use super::people_pose::State;
use super::prims::plane;
use super::props::mv;
use super::seed::hash_of;
use super::surfaces::{hex_floor, wall_grid};

const BOARD_X: f32 = 2.9;
const BOARD_Y: f32 = 1.9;
/// The top of the board's frame, for its label.
pub const BOARD_EDGE: (f32, f32, f32) = (BOARD_X, BOARD_Y + 0.72, (-ROOM_D / 2.0) as f32 + WALL);

/// A floor as the office model has it.
pub struct FloorPlan<'a> {
    pub id: &'a str,
    pub lit: bool,
    pub board: Option<BoardCounts>,
    pub people: Vec<Seated<'a>>,
    pub lead: Option<&'a str>,
    pub papers: usize,
}

/// What a floor keeps while only someone's state moves.
pub struct Room {
    pub shell: Geom,
    /// The furniture and the tiles, the room's light on them.
    pub things: Geom,
    pub wall: Geom,
    pub lit: Geom,
    pub thrown: Geom,
    /// The warm air of a lit room, drawn over everything.
    pub haze: Option<Geom>,
    pub light: RoomLight,
}

pub fn room_for(f: &FloorPlan, spot: &FloorSpot) -> Room {
    let index = ((spot.y - GROUND) / STOREY).round() as u32;
    let room = furnished(&RoomPlan { id: f.id, index, lit: f.lit, board: f.board.is_some() });
    let mut blockers = room.blockers;
    for d in &spot.desks {
        blockers.extend(desk_blockers(&d.seat, chair_spot(d.seat)));
    }
    let light = RoomLight { ambient: room_ambient(f.lit), lamps: room.lamps, blockers };
    let tiles = hex_floor(&light, f.lit, room.accent, hash_of(f.id));
    let things = Geom::merged([bake(Geom::merged(room.solid), &light), tiles]);
    let haze = f.lit.then(|| flare((LIFT_W / 2.0) as f32, ROOM_H / 2.0, 0.8, 17.0, "#ffb070", 0.1));
    Room { shell: shell_geometry().1, things, wall: wall_grid(&light, f.lit), lit: Geom::merged(room.lit), thrown: Geom::merged(room.thrown), haze, light }
}

/// Where the review papers go: the Lead's desk, or the person's own on a floor with no Lead.
pub fn paper_desk<'a>(f: &FloorPlan<'a>) -> Option<&'a str> {
    if f.papers == 0 {
        return None;
    }
    f.lead.or_else(|| f.people.iter().find(|p| p.state == State::Review).map(|p| p.looks.id))
}

/// A floor's desks and board: the matte with the room's light baked in, what
/// holds its own light, the light thrown, and who sits where (by index into `people`).
pub struct Floor {
    pub solid: Geom,
    pub lit: Geom,
    pub thrown: Geom,
    pub seated: Vec<(usize, Seat)>,
}

pub fn floor_for(f: &FloorPlan, spot: &FloorSpot, light: &RoomLight) -> Floor {
    let mut all = DeskParts::default();
    let mut seated = Vec::new();
    let papers_on = paper_desk(f);
    for d in &spot.desks {
        let at = f.people.iter().position(|p| p.looks.id == d.person_id);
        let p = at.map(|i| &f.people[i]);
        let desk = desk_for(d.seat, p, if Some(d.person_id.as_str()) == papers_on { f.papers } else { 0 });
        all.solid.extend(desk.solid);
        all.lit.extend(desk.lit);
        all.thrown.extend(desk.thrown);
        if let Some(i) = at {
            seated.push((i, d.seat));
        }
    }
    if let Some(board) = f.board {
        // The notes are what the board is for, so on a lit floor they do not take light.
        let z = (-ROOM_D / 2.0) as f32 + WALL / 2.0 + 0.04;
        all.solid.push(mv(board_geometry(), BOARD_X, BOARD_Y, z));
        let mut face = vec![mv(plane(2.44, 1.14).tint(o::CORK, None), 0.0, 0.0, 0.034)];
        face.extend(note_parts(Some(board)));
        let face = mv(Geom::merged(face), BOARD_X, BOARD_Y, z);
        if f.lit { all.lit.push(face) } else { all.solid.push(face) }
    }
    Floor { solid: bake(Geom::merged(all.solid), light), lit: Geom::merged(all.lit), thrown: Geom::merged(all.thrown), seated }
}

/// The Founder's desk out on the roof, at the roof's height, and their seat.
pub struct Top {
    pub solid: Geom,
    pub lit: Geom,
    pub thrown: Geom,
    pub founder: Option<Seat>,
}

pub fn top_for(founder: Option<&Seated>, t: &Tower) -> Top {
    let y = t.roof_y as f32;
    let desk = desk_for(FOUNDER_DESK, founder, 0);
    let up = |parts: Vec<Geom>| Geom::merged(parts.into_iter().map(|g| mv(g, 0.0, y, 0.0)));
    Top { solid: up(desk.solid), lit: up(desk.lit), thrown: up(desk.thrown), founder: founder.map(|_| FOUNDER_DESK) }
}
