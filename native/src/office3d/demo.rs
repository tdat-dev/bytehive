//! A company to look at the office with, apart from any real one: three
//! floors (one lit with a board and a Lead, one at work, one dark) and the
//! Founder on the roof. For the shot test and BYTEHIVE_OFFICE3D=demo.

use crate::company::types::Rank;

use super::people_pose::State;
use super::world_in::{FlightIn, FloorIn, OfficeIn, PersonIn};

/// (id, shirt, hair, prop, cli).
type Look<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str);

fn person((id, shirt, hair, prop, cli): Look, rank: Rank, state: State, seat: usize) -> PersonIn {
    PersonIn { id: id.into(), rank, shirt: shirt.into(), hair: hair.into(), prop: prop.into(), cli: cli.into(), state, seat, printing: state == State::Working }
}

pub fn demo_office(now: f64) -> OfficeIn {
    let staff = || Rank::Staff;
    let web = FloorIn {
        id: "web-app".into(),
        lit: true,
        board: Some((4, 3, 6)),
        lead: Some("lena".into()),
        papers: 2,
        people: vec![
            person(("lena", "#3f8f8a", "#2a2e3a", "clipboard", "claude"), Rank::Lead, State::Review, 0),
            person(("arun", "#c4605a", "#1f1a17", "laptop", "codex"), staff(), State::Working, 1),
            person(("mai", "#7a5fa8", "#6b3e26", "pen", "gemini"), staff(), State::Needs, 2),
            person(("tom", "#5d8a4e", "#d9b26a", "book", "opencode"), staff(), State::Idle, 3),
            person(("sara", "#4f7fc0", "#2a2e3a", "none", "claude"), staff(), State::Working, 4),
        ],
    };
    let api = FloorIn {
        id: "api".into(),
        lit: true,
        board: None,
        lead: None,
        papers: 0,
        people: vec![
            person(("kai", "#b0587f", "#2a2e3a", "magnifier", "cursor"), staff(), State::Working, 0),
            person(("ivy", "#a8664a", "#7a4a2a", "pencil", "qwen"), staff(), State::Working, 1),
            person(("noor", "#3b6f96", "#1f1a17", "megaphone", "claude"), staff(), State::Review, 2),
        ],
    };
    let docs = FloorIn { id: "docs".into(), lit: false, board: Some((2, 0, 9)), lead: None, papers: 0, people: vec![person(("eli", "#7d8b99", "#2a2e3a", "none", "aider"), staff(), State::Stopped, 0)] };
    OfficeIn {
        founder: Some(person(("founder", "#eef0f4", "#2a2e3a", "none", "claude"), Rank::Founder, State::Working, 0)),
        floors: vec![docs, api, web],
        needs: vec![],
        flights: vec![FlightIn { from: "arun".into(), to: "kai".into(), at: now - 900.0 }],
    }
}
