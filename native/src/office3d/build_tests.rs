//! build_parts.rs and surfaces.rs against the office's own build.ts:
//! fixtures/build.json holds each part as a digest (see furnish_tests.rs),
//! made by fixtures/gold with build.ts's private parts exported on the way in.

use glam::Vec3;
use serde_json::{Value, json};

use super::build_parts::*;
use super::furnish_tests::same_list;
use super::geom::Geom;
use super::light::{Blocker, Lamp, RoomLight, rgb};
use super::surfaces::{hex_floor, wall_grid};

fn one(name: &str, g: &Geom, want: &Value) {
    same_list("build", name, std::slice::from_ref(g), &json!([want]));
}

fn room() -> RoomLight {
    let lamp = |x: f64| Lamp { x, y: 2.6, z: 0.1, colour: rgb("#ffe0bd"), power: 1.05, reach: 2.2 };
    RoomLight {
        ambient: [0.68, 0.62, 0.55],
        lamps: vec![lamp(-3.1), lamp(3.1)],
        blockers: vec![Blocker { x: 0.2, z: 0.1, w: 1.5, d: 0.74, dark: 0.58 }, Blocker { x: -1.4, z: 1.2, w: 2.6, d: 1.0, dark: 0.5 }],
    }
}

#[test]
fn the_building_matches_the_office() {
    let all: Value = serde_json::from_str(include_str!("fixtures/build.json")).unwrap();
    let (slab, shell) = shell_geometry();
    one("slab", &slab, &all["slab"]);
    one("shell", &shell, &all["shell"]);
    one("desk", &desk_geometry(), &all["desk"]);
    one("chair", &chair_geometry(), &all["chair"]);
    one("chair_back", &chair_back_geometry(), &all["chair_back"]);
    one("board", &board_geometry(), &all["board"]);
    for k in 0..4 {
        one(&format!("thing {k}"), &desk_thing(k), &all["things"][k as usize]);
    }
    let counts = [None, Some(BoardCounts { todo: 3, doing: 2, done: 1 }), Some(BoardCounts { todo: 20, doing: 1, done: 7 }), Some(BoardCounts { todo: 0, doing: 0, done: 40 })];
    for (i, b) in counts.into_iter().enumerate() {
        let want: Vec<&str> = all["stickies"][i].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(stickies(b), want, "stickies {i}");
    }
    same_list("build", "notes", &note_parts(Some(BoardCounts { todo: 5, doing: 4, done: 2 })), &all["notes"]);
    let (lit, thrown, wire) = string_lights(Vec3::new(-2.0, 3.0, -1.0), Vec3::new(2.5, 3.2, 1.5), 6);
    same_list("build", "bulbs", &lit, &all["lights"]["lit"]);
    same_list("build", "bulb glows", &thrown, &all["lights"]["thrown"]);
    same_list("build", "wire", &wire, &all["lights"]["wire"]);
    one("lift", &lift_car(0.85), &all["lift"]);
    let r = room();
    one("wall lit", &wall_grid(&r, true), &all["wall_lit"]);
    one("wall dark", &wall_grid(&r, false), &all["wall_dark"]);
    one("floor lit", &hex_floor(&r, true, "#c4605a", 12345), &all["floor_lit"]);
    one("floor dark", &hex_floor(&r, false, "#3f8f8a", 987654), &all["floor_dark"]);
}
