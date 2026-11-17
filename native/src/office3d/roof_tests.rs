//! roof.rs against the office's own roofSteps: fixtures/roof.json holds, for
//! a tower of one floor and of three, each merged geometry as a digest (see
//! furnish_tests.rs) and where the logo and the lift car stand.

use serde_json::{Value, json};

use super::furnish_tests::same_list;
use super::geom::Geom;
use super::layout::{FloorSpot, GROUND, LIFT_W, Point, ROOM_D, ROOM_W, STOREY, Tower};
use super::roof::roof;

fn tower(n: usize) -> Tower {
    let roof_y = GROUND + n as f64 * STOREY;
    Tower {
        floors: (0..n).map(|i| FloorSpot { id: format!("f{i}"), y: GROUND + i as f64 * STOREY, desks: Vec::new() }).collect(),
        roof_y,
        lift: Point { x: ROOM_W / 2.0 + LIFT_W / 2.0, y: roof_y + STOREY, z: -ROOM_D / 2.0 + LIFT_W / 2.0 },
        width: ROOM_W + LIFT_W,
    }
}

fn close(what: &str, got: &[f32], want: &Value) {
    let want: Vec<f64> = want.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert_eq!(got.len(), want.len(), "{what}: how many");
    for (g, w) in got.iter().zip(&want) {
        assert!((*g as f64 - w).abs() < 1e-4, "{what}: {g} vs {w}");
    }
}

#[test]
fn the_roof_matches_the_office() {
    let all: Value = serde_json::from_str(include_str!("fixtures/roof.json")).unwrap();
    for case in all.as_array().unwrap() {
        let n = case["floors"].as_u64().unwrap() as usize;
        let name = format!("roof of {n}");
        let r = roof(&tower(n));
        let one = |what: &str, g: &Geom, want: &Value| same_list(&name, what, std::slice::from_ref(g), &json!([want]));
        let deck = case["deck"].as_array().unwrap();
        // three keeps the slab where it was made and lifts the mesh.
        let y = deck[0].as_f64().unwrap() as f32;
        assert!((y as f64 - tower(n).roof_y).abs() < 1e-6, "{name}: deck height");
        one("deck", &super::props::mv(r.deck.clone(), 0.0, -y, 0.0), &json!(deck[1..]));
        one("solid", &r.solid, &case["solid"]);
        one("lit", &r.lit, &case["lit"]);
        one("clear", &r.clear, &case["clear"]);
        one("thrown", &r.thrown, &case["thrown"]);
        one("car", &r.car, &case["car_geo"]);
        close(&format!("{name} logo"), &[r.logo_at.x, r.logo_at.y, r.logo_at.z, r.logo_turn], &case["logo"]);
        close(&format!("{name} car"), &r.car_at.to_array(), &case["car"]);
        close(&format!("{name} stops"), &r.car_stops, &case["stops"]);
    }
}
