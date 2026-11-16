//! light.rs and glow.rs against the office's own light.ts and geometry.ts:
//! fixtures/light.json was made by bundling those files with three 0.170 and
//! running them in node (see the plan for how).

use std::f32::consts::PI;

use glam::{Mat4, Vec3};
use serde_json::Value;

use super::geom::Geom;
use super::glow::{flare, pool, wash};
use super::light::{Blocker, Lamp, RoomLight, bake, light_at, rgb, shade_at};
use super::prims::box_geom;

fn gold() -> Value {
    serde_json::from_str(include_str!("fixtures/light.json")).unwrap()
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

fn room() -> RoomLight {
    RoomLight {
        ambient: rgb("#3a3346"),
        lamps: vec![Lamp { x: 1.0, y: 2.6, z: 0.5, colour: rgb("#ffd9a0"), power: 1.4, reach: 2.2 }, Lamp { x: -3.0, y: 2.4, z: -1.0, colour: rgb("#9fb4e6"), power: 0.8, reach: 3.0 }],
        blockers: vec![Blocker { x: 0.2, z: 0.1, w: 1.5, d: 0.74, dark: 0.58 }, Blocker { x: 0.9, z: 0.6, w: 0.3, d: 0.3, dark: 0.38 }],
    }
}

fn same(name: &str, g: &Geom) {
    let all = gold();
    let (pos, normal, color) = (nums(&all[name]["pos"]), nums(&all[name]["normal"]), nums(&all[name]["color"]));
    assert_eq!(g.len() * 3, pos.len(), "{name}: corner count");
    for i in 0..g.len() {
        for k in 0..3 {
            let (p, n, c) = (g.pos[i].to_array()[k] as f64, g.normal[i].to_array()[k] as f64, g.color[i].to_array()[k] as f64);
            assert!((p - pos[i * 3 + k]).abs() < 1e-4, "{name}: corner {i} place {k}: {p} vs {}", pos[i * 3 + k]);
            assert!((n - normal[i * 3 + k]).abs() < 1e-4, "{name}: corner {i} normal {k}: {n} vs {}", normal[i * 3 + k]);
            assert!((c - color[i * 3 + k]).abs() < 1e-5, "{name}: corner {i} colour {k}: {c} vs {}", color[i * 3 + k]);
        }
    }
}

#[test]
fn light_matches_the_office() {
    let all = gold();
    let r = room();
    let want = &all["light_at"];
    for (i, (x, y, z)) in [(0.0, 0.2, 0.0), (2.0, 1.0, -1.0)].into_iter().enumerate() {
        let got = light_at(&r, x, y, z);
        for (c, w) in nums(&want[i]).into_iter().enumerate() {
            assert!((got[c] - w).abs() < 1e-6, "light_at {i}.{c}: {} vs {w}", got[c]);
        }
    }
    for (i, (x, z)) in [(0.3, 0.2), (1.2, 0.9), (3.0, 3.0)].into_iter().enumerate() {
        let w = all["shade_at"][i].as_f64().unwrap();
        assert!((shade_at(&r.blockers, x, z) - w).abs() < 1e-12, "shade_at {i}");
    }
    let baked = bake(box_geom(1.2, 0.8, 0.6).tint("#c8b49a", None).transform(Mat4::from_translation(Vec3::new(0.4, 0.3, 0.2))), &r);
    same("baked", &baked);
}

#[test]
fn glows_match_the_office() {
    same("flare", &flare(1.0, 2.0, 3.0, 0.8, "#ffd9a0", 0.6));
    same("pool", &pool(0.5, 0.01, -1.0, 2.2, 1.4, "#ffcf9a", 0.35));
    same("wash", &wash(-2.0, 1.5, -3.3, 3.0, 2.0, PI / 2.0, "#9fb4e6", 0.2));
}
