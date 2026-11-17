//! setting.rs, town.rs and sky.rs against the office's own setting.ts:
//! fixtures/setting.json holds each built part as a digest (see
//! furnish_tests.rs), the stars as sums, and the turn to the viewer.

use serde_json::{Value, json};

use super::furnish_tests::same_list;
use super::geom::Geom;
use super::setting::{bench, facing, lamp_globes, lamp_glows, plot, spill, street_lamp};
use super::sky::{moon, star_field};
use super::town::{field, town};

fn one(name: &str, g: &Geom, want: &Value) {
    same_list("setting", name, std::slice::from_ref(g), &json!([want]));
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

#[test]
fn the_setting_matches_the_office() {
    let all: Value = serde_json::from_str(include_str!("fixtures/setting.json")).unwrap();
    assert!((facing() as f64 - all["facing"].as_f64().unwrap()).abs() < 1e-6, "facing");
    one("field", &field(), &all["field"]);
    one("plot", &plot(), &all["plot"]);
    one("lamp glows", &lamp_glows(), &all["glows"]);
    one("globes", &lamp_globes(), &all["globes"]);
    one("spill", &spill(), &all["spill"]);
    one("town", &town(), &all["town"]);
    // The golden is the moon as made, before it is set in the sky.
    one("moon", &super::props::mv(moon(), 50.0, -5.0, -(super::sky::SKY_Z + 5.0)), &all["moon"]);
    one("bench", &bench(), &all["bench"]);
    one("street lamp", &street_lamp(), &all["lamp"]);

    let stars = star_field();
    let ends = |s: &super::sky::Star| [s.at.x, s.at.y, s.at.z, s.colour.x, s.colour.y, s.colour.z].map(|v| v as f64);
    for (name, got, want) in [("first", ends(&stars[0]), nums(&all["stars"]["first"])), ("last", ends(&stars[stars.len() - 1]), nums(&all["stars"]["last"]))] {
        for (g, w) in got.iter().zip(&want) {
            assert!((g - w).abs() < 1e-4, "star {name}: {g} vs {w}");
        }
    }
    let mut sum = [0.0f64; 6];
    for s in &stars {
        for (k, v) in [s.at.x, s.at.y, s.at.z, s.colour.x, s.colour.y, s.colour.z].into_iter().enumerate() {
            sum[k] += v as f64;
        }
    }
    for (k, w) in nums(&all["stars"]["sum"]).iter().enumerate() {
        assert!((sum[k] - w).abs() < 1e-2, "stars sum {k}: {} vs {w}", sum[k]);
    }
}
