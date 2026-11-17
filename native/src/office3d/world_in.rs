//! What the 3D office is told (model.ts OfficeModel, as far as the scene
//! reads it): the floors and who is on them, the Founder, who is marked, the
//! messages in the air. The tower and the cast follow from it here.

use std::collections::HashSet;

use crate::company::types::Rank;

use super::build_parts::BoardCounts;
use super::desk::Seated;
use super::floor::FloorPlan;
use super::layout::{Desk, FloorSpot, GROUND, LIFT_W, Point, ROOM_D, ROOM_W, STOREY, Tower, seat_spot};
use super::people_pose::State;
use super::people_shapes::Looks;
use super::stage::Cast;

#[derive(Clone, Debug, PartialEq)]
pub struct PersonIn {
    pub id: String,
    pub rank: Rank,
    pub shirt: String,
    pub hair: String,
    pub prop: String,
    pub cli: String,
    pub state: State,
    pub seat: usize,
    /// Their terminal is putting something out right now.
    pub printing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FloorIn {
    pub id: String,
    pub lit: bool,
    pub board: Option<(u32, u32, u32)>,
    pub people: Vec<PersonIn>,
    pub lead: Option<String>,
    pub papers: usize,
}

/// A message on its way: from whom, to whom, and when it left (ms).
#[derive(Clone, Debug, PartialEq)]
pub struct FlightIn {
    pub from: String,
    pub to: String,
    pub at: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OfficeIn {
    pub founder: Option<PersonIn>,
    pub floors: Vec<FloorIn>,
    /// Whom the Needs you rows point at.
    pub needs: Vec<String>,
    pub flights: Vec<FlightIn>,
}

impl PersonIn {
    pub fn looks(&self) -> Looks<'_> {
        Looks { id: &self.id, rank: &self.rank, shirt: &self.shirt, hair: &self.hair, prop: &self.prop }
    }

    pub fn seated(&self) -> Seated<'_> {
        Seated { looks: self.looks(), state: self.state, seat: self.seat as f64 }
    }

    /// Steam over their desk: they work, or their terminal prints.
    pub fn active(&self) -> bool {
        self.state == State::Working || self.printing
    }
}

impl FloorIn {
    pub fn plan(&self) -> FloorPlan<'_> {
        let board = self.board.map(|(todo, doing, done)| BoardCounts { todo, doing, done });
        FloorPlan { id: &self.id, lit: self.lit, board, people: self.people.iter().map(PersonIn::seated).collect(), lead: self.lead.as_deref(), papers: self.papers }
    }
}

impl OfficeIn {
    pub fn everyone(&self) -> impl Iterator<Item = &PersonIn> {
        self.founder.iter().chain(self.floors.iter().flat_map(|f| f.people.iter()))
    }

    /// towerLayout: a floor a storey, everyone at the seat they were given.
    pub fn tower(&self) -> Tower {
        let floors = self
            .floors
            .iter()
            .enumerate()
            .map(|(i, f)| FloorSpot { id: f.id.clone(), y: GROUND + i as f64 * STOREY, desks: f.people.iter().map(|p| Desk { person_id: p.id.clone(), seat: seat_spot(p.seat, f.people.len()) }).collect() })
            .collect();
        let roof_y = GROUND + self.floors.len() as f64 * STOREY;
        Tower { floors, roof_y, lift: Point { x: ROOM_W / 2.0 + LIFT_W / 2.0, y: roof_y + STOREY, z: -ROOM_D / 2.0 + LIFT_W / 2.0 }, width: ROOM_W + LIFT_W }
    }

    /// Everyone a Needs you row points at, and everyone asking something.
    pub fn marked(&self) -> HashSet<String> {
        self.needs.iter().cloned().chain(self.everyone().filter(|p| p.state == State::Needs).map(|p| p.id.clone())).collect()
    }

    pub fn cast(&self) -> Cast {
        Cast {
            founder: self.founder.as_ref().map(|f| (f.id.clone(), f.state)),
            states: self.floors.iter().flat_map(|f| f.people.iter()).map(|p| (p.id.clone(), p.state)).collect(),
            marked: self.marked(),
            boards: self.floors.iter().filter(|f| f.board.is_some()).map(|f| f.id.clone()).collect(),
        }
    }

    /// Where someone is: a floor, the roof (Some(None)), or nowhere.
    pub fn floor_of(&self, id: &str) -> Option<Option<&str>> {
        if self.founder.as_ref().is_some_and(|f| f.id == id) {
            return Some(None);
        }
        self.floors.iter().find(|f| f.people.iter().any(|p| p.id == id)).map(|f| Some(f.id.as_str()))
    }
}
