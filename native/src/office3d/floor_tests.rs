//! desk.rs and floor.rs against the office's own buildFloor and buildTop:
//! fixtures/floor.json holds three floors (lit with a board and a Lead, dark
//! with papers and no Lead, lit with five at work) and the roof's Founder desk
//! in each state, every mesh as a digest (see furnish_tests.rs) by material.

use serde_json::{Value, json};

use super::build_parts::BoardCounts;
use super::desk::Seated;
use super::floor::{FloorPlan, floor_for, room_for, top_for};
use super::furnish_tests::same_list;
use super::geom::Geom;
use super::layout::{Desk, FloorSpot, GROUND, LIFT_W, Point, ROOM_D, ROOM_W, STOREY, Seat, Tower};
use super::people_pose::State;
use super::people_shapes::Looks;
use crate::company::types::Rank;

fn one(name: &str, g: &Geom, want: &Value) {
    same_list("floor", name, std::slice::from_ref(g), &json!([want]));
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn state(s: &str) -> State {
    match s {
        "needs" => State::Needs,
        "working" => State::Working,
        "review" => State::Review,
        "idle" => State::Idle,
        _ => State::Stopped,
    }
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn seated<'a>(p: &'a Value, rank: &'a Rank) -> Seated<'a> {
    Seated { looks: Looks { id: str_of(&p["id"]), rank, shirt: str_of(&p["shirt"]), hair: str_of(&p["hair"]), prop: str_of(&p["prop"]) }, state: state(str_of(&p["state"])), seat: num(&p["seat"]) }
}

#[test]
fn floors_match_the_office() {
    let all: Value = serde_json::from_str(include_str!("fixtures/floor.json")).unwrap();
    for f in all["floors"].as_array().unwrap() {
        let id = str_of(&f["id"]);
        let ranks: Vec<Rank> = f["people"].as_array().unwrap().iter().map(|p| Rank::from(str_of(&p["rank"]).to_string())).collect();
        let people = f["people"].as_array().unwrap().iter().zip(&ranks).map(|(p, r)| seated(p, r)).collect();
        let board = f["board"].as_object().map(|b| BoardCounts { todo: b["todo"].as_u64().unwrap() as u32, doing: b["doing"].as_u64().unwrap() as u32, done: b["done"].as_u64().unwrap() as u32 });
        let plan = FloorPlan { id, lit: f["lit"] == true, board, people, lead: f["lead"].as_str(), papers: f["papers"].as_u64().unwrap() as usize };
        let desks = f["desks"].as_array().unwrap().iter().map(|d| Desk { person_id: str_of(&d["personId"]).into(), seat: Seat { x: num(&d["x"]), z: num(&d["z"]), rot: num(&d["rot"]) } }).collect();
        let spot = FloorSpot { id: id.into(), y: num(&f["y"]), desks };

        let room = room_for(&plan, &spot);
        let r = &f["room"];
        one(&format!("{id} shell"), &room.shell, &r["matte"]);
        one(&format!("{id} things"), &room.things, &r[if plan.lit { "indoor" } else { "night" }]);
        one(&format!("{id} wall"), &room.wall, &r["wall"]);
        one(&format!("{id} room lit"), &room.lit, &r["glow"]);
        one(&format!("{id} room thrown"), &room.thrown, &r["deskLight"]);
        assert_eq!(room.haze.is_some(), r.get("hazeLight").is_some(), "{id} haze");

        let floor = floor_for(&plan, &spot, &room.light);
        let w = &f["floor"];
        one(&format!("{id} desks"), &floor.solid, &w[if plan.lit { "indoor" } else { "night" }]);
        one(&format!("{id} desks lit"), &floor.lit, &w["glow"]);
        match w.get("deskLight") {
            Some(want) => one(&format!("{id} desks thrown"), &floor.thrown, want),
            None => assert!(floor.thrown.is_empty(), "{id} throws nothing"),
        }
        let sat = f["sat"].as_array().unwrap();
        assert_eq!(floor.seated.len(), sat.len(), "{id} seated");
        for (i, seat) in &floor.seated {
            let p = sat.iter().find(|s| s["id"] == f["people"][*i]["id"]).unwrap();
            let (cx, cz) = super::layout::chair_spot(*seat);
            assert!((cx - num(&p["x"])).abs() < 1e-9 && (cz - num(&p["z"])).abs() < 1e-9 && (seat.rot - num(&p["rot"])).abs() < 1e-12, "{id} seat of {}", p["id"]);
        }
    }
}

#[test]
fn the_founder_desk_matches_the_office() {
    let all: Value = serde_json::from_str(include_str!("fixtures/floor.json")).unwrap();
    let founder_rank = Rank::Founder;
    for c in all["tops"].as_array().unwrap() {
        let n = c["n"].as_u64().unwrap() as usize;
        let roof_y = GROUND + n as f64 * STOREY;
        let spots = (0..n).map(|i| FloorSpot { id: format!("f{i}"), y: GROUND + i as f64 * STOREY, desks: vec![] }).collect();
        let t = Tower { floors: spots, roof_y, lift: Point { x: ROOM_W / 2.0 + LIFT_W / 2.0, y: roof_y + STOREY, z: -ROOM_D / 2.0 + LIFT_W / 2.0 }, width: ROOM_W + LIFT_W };
        let founder = c["state"].as_str().map(|s| Seated { looks: Looks { id: "founder", rank: &founder_rank, shirt: "#eef0f4", hair: "#2a2e3a", prop: "pen" }, state: state(s), seat: 0.0 });
        let top = top_for(founder.as_ref(), &t);
        let what = format!("top {n} {}", c["state"]);
        one(&format!("{what} solid"), &top.solid, &c["parts"]["matte"]);
        one(&format!("{what} lit"), &top.lit, &c["parts"]["glow"]);
        match c["parts"].get("deskLight") {
            Some(want) => one(&format!("{what} thrown"), &top.thrown, want),
            None => assert!(top.thrown.is_empty(), "{what} throws nothing"),
        }
        match (top.founder, c["founder"].as_array()) {
            (Some(seat), Some(at)) => {
                let (cx, cz) = super::layout::chair_spot(seat);
                assert!((cx - num(&at[0])).abs() < 1e-9 && (roof_y - num(&at[1])).abs() < 1e-9 && (cz - num(&at[2])).abs() < 1e-9, "{what} founder at");
            }
            (None, None) => {}
            (got, _) => panic!("{what} founder: {got:?}"),
        }
    }
}
