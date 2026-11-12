//! Every shape against three.js 0.170 itself: fixtures/prims.json holds what
//! three made for the same arguments, unrolled with toNonIndexed (made by
//! node with the real three.module.js; see the plan for how).

use std::f32::consts::PI;

use serde_json::Value;

use super::geom::Geom;
use super::prims::*;

fn golden(name: &str) -> (Vec<f32>, Vec<f32>) {
    let all: Value = serde_json::from_str(include_str!("fixtures/prims.json")).unwrap();
    let list = |k: &str| all[name][k].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect::<Vec<_>>();
    (list("pos"), list("normal"))
}

fn same(name: &str, g: Geom) {
    let (pos, normal) = golden(name);
    assert_eq!(g.len() * 3, pos.len(), "{name}: corner count");
    for i in 0..g.len() {
        for k in 0..3 {
            let (p, n) = (g.pos[i].to_array()[k], g.normal[i].to_array()[k]);
            assert!((p - pos[i * 3 + k]).abs() < 1e-4, "{name}: corner {i} place {k}: {p} vs {}", pos[i * 3 + k]);
            assert!((n - normal[i * 3 + k]).abs() < 1e-4, "{name}: corner {i} normal {k}: {n} vs {}", normal[i * 3 + k]);
        }
    }
}

#[test]
fn shapes_match_three_corner_for_corner() {
    same("box", box_geom(1.2, 0.5, 0.8));
    same("plane", plane(2.0, 1.5));
    same("circle", circle(0.5, 6));
    same("cylinder", cylinder(0.3, 0.5, 1.2, 8, 2, false));
    same("cylinder_open", cylinder(0.2, 0.2, 1.0, 6, 1, true));
    same("cone", cone(0.4, 0.9, 7, 1, false));
    same("sphere", sphere(0.2, 9, 7));
    same("sphere_part", sphere_part(0.4, 20, 9, (-0.35, PI + 0.7), (PI / 2.0 - 0.3, PI / 2.4)));
    same("sphere_cap", sphere_part(0.34, 16, 8, (0.0, PI * 2.0), (0.0, PI / 2.0)));
    same("torus", torus(0.34, 0.03, 8, 24, PI * 2.0));
    same("torus_arc", torus(0.2, 0.038, 6, 12, PI * 1.1));
}
