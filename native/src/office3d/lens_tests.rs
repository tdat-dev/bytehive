//! lens.rs and stage.rs against scene.ts and three's own PerspectiveCamera
//! (fixtures/lens.json): the CSS curves, the camera's matrices with and
//! without a view offset, projected points, and what a click means.

use glam::{DMat4, DVec3, Vec2};
use serde_json::Value;

use super::layout::{Desk, FloorSpot, Level, Point, Seat, Shot, Tower};
use super::lens::{cubic_bezier, ease_level, lens};
use super::people_pose::State;
use super::pick::pick;
use super::stage::{AnchorKind, Cast, What, anchors, band_for, pick_from, proxies};

fn all() -> Value {
    serde_json::from_str(include_str!("fixtures/lens.json")).unwrap()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(num).collect()
}

#[test]
fn curves_ease_as_css_does() {
    let all = all();
    let ks = [-0.1, 0.0, 0.05, 0.2, 0.37, 0.5, 0.81, 0.99, 1.0, 1.2];
    for c in all["curves"].as_array().unwrap() {
        let p = nums(&c["c"]);
        for (k, want) in ks.iter().zip(nums(&c["at"])) {
            assert!((cubic_bezier(p[0], p[1], p[2], p[3], *k) - want).abs() < 1e-12, "{p:?} at {k}");
        }
    }
    for (k, want) in ks.iter().zip(nums(&all["ease"])) {
        assert!((ease_level(*k) - want).abs() < 1e-12);
    }
}

#[test]
fn the_camera_matches_three() {
    for c in all()["lenses"].as_array().unwrap() {
        let s = &c["s"];
        let shot = Shot { pos: Point { x: num(&s["px"]), y: num(&s["py"]), z: num(&s["pz"]) }, target: Point { x: num(&s["tx"]), y: num(&s["ty"]), z: num(&s["tz"]) }, fov: num(&s["fov"]), ox: num(&s["ox"]) };
        let l = lens(&shot, num(&c["w"]), num(&c["h"]));
        let same = |name: &str, got: DMat4, want: Vec<f64>| {
            for (k, (g, w)) in got.to_cols_array().iter().zip(want).enumerate() {
                assert!((g - w).abs() < 1e-9, "{name}[{k}] {g} vs {w} ({s})");
            }
        };
        same("projection", l.projection_gl(), nums(&c["proj"]));
        same("view", l.view, nums(&c["view"]));
        let points = [[0.0, 0.0, 0.0], [3.0, 4.5, -2.0], [-5.0, 12.0, 3.0], [6.05, 16.0, -2.55], [100.0, 0.0, 0.0]];
        for (p, want) in points.iter().zip(c["points"].as_array().unwrap()) {
            let got = l.project(DVec3::from_array(*p));
            for (g, w) in got.to_array().iter().zip(nums(want)) {
                assert!((g - w).abs() < 1e-9, "project {p:?}: {g} vs {w}");
            }
            // The GPU's depth runs 0 to 1, the rest the same.
            let gpu = l.view_proj_gpu().project_point3(DVec3::from_array(*p));
            assert!((gpu.x - got.x).abs() < 1e-9 && (gpu.y - got.y).abs() < 1e-9 && ((gpu.z * 2.0 - 1.0) - got.z).abs() < 1e-6, "gpu depth {p:?}");
        }
    }
}

fn what(v: &Value) -> What {
    What { person: v["personId"].as_str().map(Into::into), floor: v["floorId"].as_str().map(Into::into), over: v["over"] == true }
}

#[test]
fn a_click_means_what_scene_ts_says() {
    for c in all()["picks"].as_array().unwrap() {
        let hits: Vec<(What, f64)> = c["hits"].as_array().unwrap().iter().map(|h| (what(h), num(&h["distance"]))).collect();
        let got = pick_from(&hits);
        let want = c["got"].as_object().map(|_| what(&c["got"]));
        assert_eq!(got, want, "{}", c["hits"]);
    }
}

fn tower() -> Tower {
    let desk = |id: &str, x: f64| Desk { person_id: id.into(), seat: Seat { x, z: 0.5, rot: 0.0 } };
    let floors = vec![FloorSpot { id: "a".into(), y: 0.5, desks: vec![desk("ann", -2.0), desk("bo", 1.0)] }, FloorSpot { id: "b".into(), y: 4.5, desks: vec![desk("cy", 0.0)] }];
    Tower { floors, roof_y: 8.5, lift: Point { x: 6.05, y: 12.5, z: -2.55 }, width: 12.1 }
}

#[test]
fn boxes_bands_and_anchors_follow_the_tower() {
    let t = tower();
    let mut cast = Cast { founder: Some(("founder".into(), State::Working)), ..Default::default() };
    cast.states.insert("ann".into(), State::Needs);
    cast.states.insert("bo".into(), State::Working);
    cast.marked.insert("ann".into());
    cast.boards.insert("b".into());
    let boxes = proxies(&t, &cast);
    // Two floors, three people, ann's mark, and the top.
    assert_eq!(boxes.len(), 2 + 3 + 1 + 1);
    assert!(boxes.iter().any(|b| b.what.over && b.what.person.as_deref() == Some("ann")));

    let shot = super::layout::shot_for(&Level::Building, &t, 1.6, 0.0);
    let l = lens(&shot, 1280.0, 800.0);
    let vp = (l.projection_gl() * l.view).as_mat4();
    // A click on the middle of the tower lands on a floor or someone in it.
    let hits: Vec<(What, f64)> = boxes.iter().filter_map(|b| {
        let one = [b.clone()];
        pick(&one, vp, l.eye.as_vec3(), Vec2::ZERO).map(|(p, d)| (p.what.clone(), d as f64))
    }).collect();
    assert!(pick_from(&hits).is_some());

    assert_eq!(band_for(&Level::Building, &t).z, 0.0);
    assert_eq!(band_for(&Level::Floor("a".into()), &t).x, -1000.0);
    assert!((band_for(&Level::Floor("b".into()), &t).x - (4.5 - 0.56)).abs() < 1e-12);
    assert!((band_for(&Level::Desk { person: "founder".into(), floor: None }, &t).x - (8.5 - 0.56)).abs() < 1e-12);

    let marks = anchors(&t, &cast, &l, 1280.0, 800.0);
    let kinds = |k: AnchorKind| marks.iter().filter(|a| a.kind == k).count();
    assert_eq!((kinds(AnchorKind::Floor), kinds(AnchorKind::Board), kinds(AnchorKind::Person), kinds(AnchorKind::Top)), (2, 1, 4, 1));
    let ann = marks.iter().find(|a| a.id == "ann").unwrap();
    assert_eq!(ann.clear, Some(super::lens::MARK_PX));
    assert!(marks.iter().all(|a| a.visible), "the Building shot holds every word");
}

#[test]
#[ignore]
fn count_the_vertices() {
    use super::floor::{floor_for, room_for};
    let m = super::demo::demo_office(0.0);
    let t = m.tower();
    let mut total = 0usize;
    let mut say = |name: &str, n: usize| {
        total += n;
        eprintln!("{name}: {n}");
    };
    say("town", super::town::town().len());
    say("plot", super::setting::plot().len());
    say("field", super::town::field().len());
    let r = super::roof::roof(&t);
    say("roof", r.deck.len() + r.solid.len() + r.lit.len() + r.clear.len() + r.thrown.len() + r.car.len());
    for (f, spot) in m.floors.iter().zip(&t.floors) {
        let plan = f.plan();
        let room = room_for(&plan, spot);
        say(&format!("{} shell", f.id), room.shell.len());
        say(&format!("{} things", f.id), room.things.len());
        say(&format!("{} wall", f.id), room.wall.len());
        say(&format!("{} lit+thrown", f.id), room.lit.len() + room.thrown.len());
        let fl = floor_for(&plan, spot, &room.light);
        say(&format!("{} desks", f.id), fl.solid.len() + fl.lit.len());
    }
    let p = &m.floors[2].people[0];
    let s = super::people_shapes::shapes_of(&p.looks());
    say("one person", s.torso.len() + s.legs.len() + s.arm_l.len() + s.arm_r.len());
    eprintln!("total {total} vertices, {} MB at 36 bytes", total * 36 / 1_048_576);
}
