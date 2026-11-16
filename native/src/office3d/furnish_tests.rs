//! Whole rooms against the office's own props.ts: fixtures/furnish.json holds,
//! for six floors, every geometry of `furnished(room)` as a digest (corner
//! count, sums of place, |place|, normal and colour, first and last corner),
//! with the room's accent, lamps and blockers. Made by fixtures/gold.

use serde_json::Value;

use super::furnish::{RoomPlan, furnished};
use super::geom::Geom;

pub(super) fn digest(g: &Geom) -> Vec<f64> {
    let sum = |v: &[glam::Vec3], abs: bool| {
        let mut s = [0.0f64; 3];
        for p in v {
            for (k, c) in p.to_array().into_iter().enumerate() {
                s[k] += if abs { (c as f64).abs() } else { c as f64 };
            }
        }
        s
    };
    let mut d = vec![g.len() as f64];
    d.extend(sum(&g.pos, false));
    d.extend(sum(&g.pos, true));
    d.extend(sum(&g.normal, false));
    d.extend(sum(&g.color, false));
    d.extend(g.pos[0].to_array().map(|c| c as f64));
    d.extend(g.pos[g.len() - 1].to_array().map(|c| c as f64));
    d
}

pub(super) fn same_list(room: &str, what: &str, got: &[Geom], want: &Value) {
    let want = want.as_array().unwrap();
    assert_eq!(got.len(), want.len(), "{room} {what}: how many geometries");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        let w: Vec<f64> = w.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let d = digest(g);
        assert_eq!(d[0], w[0], "{room} {what} {i}: corner count");
        // A sum over n corners carries n f32 roundings.
        let tol = 1e-4 * d[0].max(10.0);
        for k in 1..d.len() {
            let t = if k >= 13 { 1e-4 } else { tol };
            assert!((d[k] - w[k]).abs() < t, "{room} {what} {i}: digest {k}: {} vs {}", d[k], w[k]);
        }
    }
}

#[test]
fn rooms_match_the_office() {
    let all: Value = serde_json::from_str(include_str!("fixtures/furnish.json")).unwrap();
    for case in all.as_array().unwrap() {
        let r = &case["room"];
        let id = r["id"].as_str().unwrap();
        let plan = RoomPlan { id, index: r["index"].as_u64().unwrap() as u32, lit: r["lit"].as_bool().unwrap(), board: r["board"].as_bool().unwrap() };
        let f = furnished(&plan);
        assert_eq!(f.accent, case["accent"].as_str().unwrap(), "{id}: accent");
        same_list(id, "solid", &f.solid, &case["solid"]);
        same_list(id, "lit", &f.lit, &case["lit"]);
        same_list(id, "thrown", &f.thrown, &case["thrown"]);
        let lamps = case["lamps"].as_array().unwrap();
        assert_eq!(f.lamps.len(), lamps.len(), "{id}: lamps");
        for (l, w) in f.lamps.iter().zip(lamps) {
            for (k, v) in [("x", l.x), ("y", l.y), ("z", l.z), ("power", l.power), ("reach", l.reach)] {
                assert!((v - w[k].as_f64().unwrap()).abs() < 1e-5, "{id}: lamp {k}");
            }
        }
        let blockers = case["blockers"].as_array().unwrap();
        assert_eq!(f.blockers.len(), blockers.len(), "{id}: blockers");
        for (b, w) in f.blockers.iter().zip(blockers) {
            for (k, v) in [("x", b.x), ("z", b.z), ("w", b.w), ("d", b.d), ("dark", b.dark)] {
                assert!((v - w[k].as_f64().unwrap()).abs() < 1e-5, "{id}: blocker {k}: {v} vs {}", w[k]);
            }
        }
    }
}
