//! The floors panel as the 3D office (shell.ts over scene.ts): the company's
//! model handed to the scene, a click on a floor flying to it and a click on
//! someone opening their desk, as the 2D list does, and the tags over the
//! picture (tags3d.rs). BYTEHIVE_OFFICE3D=off keeps the 2D list.

use std::rc::Rc;

use crate::office3d::layout::Level;
use crate::office3d::people_pose::State;
use crate::office3d::world_in::{FlightIn, FloorIn, OfficeIn, PersonIn};

use super::model::{OfficeModel, OfficePerson, OfficeState};
use super::rows::Target;
use super::*;

/// The office draws in 3D here: a graphics card took it, and nobody turned it off.
pub(super) fn office3d_on() -> bool {
    !std::env::var("BYTEHIVE_OFFICE3D").is_ok_and(|v| v == "off" || v.starts_with("spike")) && crate::office3d::view::broken().is_none()
}

fn state_of(s: OfficeState) -> State {
    match s {
        OfficeState::Needs => State::Needs,
        OfficeState::Working => State::Working,
        OfficeState::Review => State::Review,
        OfficeState::Idle => State::Idle,
        OfficeState::Stopped => State::Stopped,
    }
}

fn person_in(p: &OfficePerson) -> PersonIn {
    PersonIn {
        id: p.id.clone(),
        rank: p.rank.clone(),
        shirt: p.shirt.clone(),
        hair: p.hair.clone(),
        prop: p.prop.clone(),
        cli: p.cli.clone(),
        state: state_of(p.state),
        seat: p.seat,
        printing: p.printing,
    }
}

/// What the scene is told of the company.
pub(super) fn office_in(m: &OfficeModel) -> OfficeIn {
    OfficeIn {
        founder: m.founder.as_ref().map(person_in),
        floors: m.floors.iter().map(|f| FloorIn { id: f.id.clone(), lit: f.lit, board: f.board, people: f.people.iter().map(person_in).collect(), lead: f.lead.clone(), papers: f.papers }).collect(),
        needs: m.needs.iter().filter_map(|r| r.person_id.clone()).collect(),
        flights: m.flights.iter().map(|f| FlightIn { from: f.from.clone(), to: f.to.clone(), at: f.at as f64 }).collect(),
    }
}

/// People leave and projects close while the owner looks at them (nav.ts clampLevel).
fn clamp(l: &Level, m: &OfficeModel) -> Level {
    match l {
        Level::Floor(id) if m.floors.iter().any(|f| &f.id == id) => l.clone(),
        _ => Level::Building,
    }
}

impl Inbox {
    /// The office over the floors panel, with its tags.
    pub(super) fn co_office3d(&mut self, m: &OfficeModel, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let level = clamp(&self.company.level3d, m);
        self.company.level3d = level.clone();
        crate::office3d::view::set(&office_in(m), &level, crate::office3d::view::reduced_motion());
        let this = cx.entity().downgrade();
        let on_pick = Rc::new(move |w: crate::office3d::stage::What, window: &mut Window, cx: &mut App| {
            let _ = this.update(cx, |this, cx| match (w.person, w.floor) {
                (Some(id), _) => this.co_go(Target::Desk(id), window, cx),
                (None, Some(floor)) => this.co_level(Level::Floor(floor), cx),
                _ => {}
            });
        });
        let tags = self.co_tags3d(m, &level, window, cx);
        div().relative().size_full().child(crate::office3d::view::view(on_pick)).child(tags).into_any_element()
    }

    pub(super) fn co_level(&mut self, l: Level, cx: &mut Context<Self>) {
        if self.company.level3d != l {
            self.company.level3d = l;
            cx.notify();
        }
    }

    /// Esc on the office goes one level up; false at the Building.
    pub(in crate::inbox) fn co_level_up(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.company.open || self.company.level3d == Level::Building {
            return false;
        }
        self.co_level(Level::Building, cx);
        true
    }
}
