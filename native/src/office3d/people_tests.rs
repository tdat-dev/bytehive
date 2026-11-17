//! people_pose.rs, people_shapes.rs and people_desk.rs against the office's
//! own people.ts: fixtures/people.json holds each shape as a digest (see
//! furnish_tests.rs), poses and rigs as numbers, and the planes' flights.

use serde_json::{Value, json};

use super::furnish_tests::same_list;
use super::geom::Geom;
use super::layout::{FloorSpot, Point, Seat, Tower};
use super::people_desk::{DeskGlow, desk_glow, desk_prop, lamp_geometry, paper_stack, plane_at, plane_geometry, screen_light};
use super::people_pose::{Screen, State, hop_at, pose, rig, spot_of};
use super::people_shapes::{Hand, Looks, arm_geometry, bubble_geometry, legs_geometry, torso_geometry};
use crate::company::types::Rank;

fn all() -> Value {
    serde_json::from_str(include_str!("fixtures/people.json")).unwrap()
}

fn one(name: &str, g: &Geom, want: &Value) {
    same_list("people", name, std::slice::from_ref(g), &json!([want]));
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn close(what: &str, got: f64, want: f64) {
    assert!((got - want).abs() < 1e-6, "{what}: {got} vs {want}");
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

fn seat(d: &Value) -> Seat {
    Seat { x: num(&d["x"]), z: num(&d["z"]), rot: num(&d["rot"]) }
}

#[test]
fn the_people_look_as_in_the_office() {
    let all = all();
    for p in all["people"].as_array().unwrap() {
        let rank = Rank::from(p["rank"].as_str().unwrap().to_string());
        let looks = Looks { id: p["id"].as_str().unwrap(), rank: &rank, shirt: p["shirt"].as_str().unwrap(), hair: p["hair"].as_str().unwrap(), prop: p["prop"].as_str().unwrap() };
        let id = looks.id;
        one(&format!("{id} torso"), &torso_geometry(&looks), &p["torso"]);
        one(&format!("{id} legs"), &legs_geometry(), &p["legs"]);
        one(&format!("{id} arm L"), &arm_geometry(&looks, Hand::L), &p["armL"]);
        one(&format!("{id} arm R"), &arm_geometry(&looks, Hand::R), &p["armR"]);
    }
    one("bubble", &bubble_geometry(), &all["bubble"]);
}

#[test]
fn poses_and_rigs_match_the_office() {
    let all = all();
    for c in all["poses"].as_array().unwrap() {
        let p = pose(state(c["s"].as_str().unwrap()), num(&c["t"]), num(&c["seat"]));
        let w = &c["pose"];
        let what = format!("{} t{} seat{}", c["s"], c["t"], c["seat"]);
        for (name, got) in [("sit", p.sit), ("lean", p.lean), ("armL", p.arm_l), ("armR", p.arm_r), ("turn", p.turn)] {
            close(&format!("{what} {name}"), got, num(&w[name]));
        }
        let screen = match p.screen {
            Screen::Work => "work",
            Screen::Off => "off",
            Screen::Dim => "dim",
        };
        assert_eq!((screen, p.lamp, p.bubble, p.present), (w["screen"].as_str().unwrap(), w["lamp"] == true, w["bubble"] == true, w["present"] == true), "{what}");
    }
    for (index, c) in all["rigs"].as_array().unwrap().iter().enumerate() {
        let s = state(c["s"].as_str().unwrap());
        let d = seat(&c["d"]);
        // Per state, three desks each unmarked then marked; person i sits at
        // desk i with seat number i, posed at 1.3 s and raised 0.2.
        let at = (index / 2) % 3;
        let r = rig(d, 4.5, &pose(s, 1300.0, at as f64), 0.2, c["marked"] == true);
        let what = format!("{} desk {at} marked {}", c["s"], c["marked"]);
        assert_eq!(r.visible, c["visible"] == true, "{what} visible");
        let same = |name: &str, m: Option<glam::DMat4>| match (m, c[name].as_array()) {
            (Some(m), Some(want)) => {
                for (k, (g, w)) in m.to_cols_array().iter().zip(want).enumerate() {
                    assert!((g - num(w)).abs() < 1e-9, "{what} {name}[{k}]: {g} vs {w}");
                }
            }
            (None, None) => {}
            (got, _) => panic!("{what} {name}: {got:?} vs {}", c[name]),
        };
        same("trunk", Some(r.trunk));
        same("legs", Some(r.legs));
        same("armL", Some(r.arm_l));
        same("armR", Some(r.arm_r));
        same("bubble", r.bubble);
    }
    for c in all["spots"].as_array().unwrap() {
        let s = spot_of(seat(&c["d"]), state(c["s"].as_str().unwrap()));
        for (name, got) in [("x", s.x), ("z", s.z), ("head", s.head), ("bubble", s.bubble)] {
            close(&format!("spot {} {name}", c["s"]), got, num(&c["spot"][name]));
        }
    }
    for h in all["hops"].as_array().unwrap() {
        close("hop", hop_at(num(&h[0])), num(&h[1]));
    }
}

#[test]
fn desk_things_and_planes_match_the_office() {
    let all = all();
    one("laptop", &desk_prop("laptop").unwrap(), &all["laptop"]);
    one("book", &desk_prop("book").unwrap(), &all["book"]);
    assert!(desk_prop("pen").is_none());
    one("stack of 3", &paper_stack(3, false), &all["stack3"]);
    one("stack of 2 aside", &paper_stack(2, true), &all["stack2"]);
    let (solid, lit) = lamp_geometry();
    one("lamp", &solid, &all["lampSolid"]);
    one("lamp shade", &lit, &all["lampLit"]);
    one("work glow", &desk_glow(DeskGlow::Work), &all["glowWork"]);
    one("lamp glow", &desk_glow(DeskGlow::Lamp), &all["glowLamp"]);
    for (s, want) in [Screen::Work, Screen::Dim, Screen::Off].into_iter().zip(all["screens"].as_array().unwrap()) {
        let got = screen_light(s);
        for (g, w) in got.to_array().iter().zip(want.as_array().unwrap()) {
            assert!((*g as f64 - num(w)).abs() < 1e-5, "screen {g} vs {w}");
        }
    }
    one("plane", &plane_geometry(), &all["plane"]);

    let spot = |id: &str, y: f64| FloorSpot { id: id.into(), y, desks: vec![] };
    let tower = Tower { floors: vec![spot("a", 0.5), spot("b", 4.5)], roof_y: 8.5, lift: Point { x: 6.05, y: 12.5, z: -2.55 }, width: 12.1 };
    let floor_of = |id: &str| match id {
        "ann" => Some(Some("a")),
        "bo" => Some(Some("b")),
        "fo" => Some(None),
        _ => None,
    };
    for f in all["flights"].as_array().unwrap() {
        let (from, to) = (f["from"].as_str().unwrap(), f["to"].as_str().unwrap());
        let got = plane_at(&tower, floor_of(from), floor_of(to), 0.0, num(&f["now"]), f["reduced"] == true);
        match (got, f["at"].as_object()) {
            (Some(g), Some(w)) => {
                for (name, v) in [("x", g.at.x), ("y", g.at.y), ("z", g.at.z), ("dx", g.dir.x), ("dy", g.dir.y), ("dz", g.dir.z)] {
                    close(&format!("plane {from}>{to} {name}"), v, num(&w[name]));
                }
            }
            (None, None) => {}
            (g, _) => panic!("plane {from}>{to}: {g:?} vs {}", f["at"]),
        }
    }
}
