//! layout.test.ts, case for case.

use std::collections::HashSet;
use std::f64::consts::PI;

use super::layout::*;

/// `n` floors f0..; the first holds a and b, the rest x.
fn model(n: usize) -> Vec<(String, Vec<String>)> {
    (0..n).map(|i| (format!("f{i}"), if i == 0 { vec!["a".into(), "b".into()] } else { vec!["x".into()] })).collect()
}

fn across(s: Seat) -> f64 {
    let (x, z) = chair_spot(s);
    let r = right();
    x * r.x + z * r.z
}

#[test]
fn seat_spot_keeps_desks_in_the_room_none_on_another() {
    for count in [1, 2, 6, 9] {
        let spots: Vec<Seat> = (0..count).map(|i| seat_spot(i, count)).collect();
        for s in &spots {
            assert!(s.x.abs() < ROOM_W / 2.0 && s.z.abs() < ROOM_D / 2.0, "{count}: {s:?}");
        }
        let distinct: HashSet<String> = spots.iter().map(|s| format!("{},{}", s.x, s.z)).collect();
        assert_eq!(distinct.len(), count);
    }
}

#[test]
fn seat_spot_does_not_move_anyone_when_someone_joins_a_row_that_fits() {
    assert_eq!(seat_spot(0, 2), seat_spot(0, 3));
}

#[test]
fn seat_spot_keeps_heads_apart_across_the_screen() {
    for (count, gap) in [(2, 2.3), (4, 2.3), (6, 1.5), (8, 1.1)] {
        let mut xs: Vec<f64> = (0..count).map(|i| across(seat_spot(i, count))).collect();
        xs.sort_by(|a, b| a.total_cmp(b));
        for w in xs.windows(2) {
            assert!(w[1] - w[0] > gap, "{count}: {xs:?}");
        }
    }
}

#[test]
fn seat_spot_sits_everyone_facing_the_viewer() {
    for i in 0..8 {
        let d = seat_spot(i, 8);
        assert!(chair_spot(d).1 < d.z);
    }
}

#[test]
fn seat_spot_starts_a_small_team_at_the_middle_of_the_line() {
    let a = |s: Seat| across(s).abs();
    assert!(a(seat_spot(0, 4)) < a(seat_spot(2, 4)));
    assert!(a(seat_spot(1, 4)) < a(seat_spot(3, 4)));
}

#[test]
fn tower_layout_stacks_floors_from_the_ground_the_roof_above() {
    let t = tower_layout(&model(3));
    assert_eq!(t.floors.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["f0", "f1", "f2"]);
    assert_eq!(t.floors[1].y - t.floors[0].y, STOREY);
    assert!(t.roof_y > t.floors[2].y);
}

#[test]
fn tower_layout_still_has_a_roof_with_no_floors() {
    let t = tower_layout(&model(0));
    assert!(t.floors.is_empty());
    assert!(t.roof_y > 0.0);
}

#[test]
fn tower_layout_has_a_desk_per_person_and_the_lift_runs_the_full_height() {
    let t = tower_layout(&model(2));
    assert_eq!(t.floors[0].desks.iter().map(|d| d.person_id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert!(t.lift.y >= t.roof_y);
}

/// Where a point lands in the frame, -1 to 1 each way, by the camera's own axes.
fn in_frame(s: &Shot, q: Point, aspect: f64) -> (f64, f64) {
    let back = (s.pos.x - s.target.x, s.pos.y - s.target.y, s.pos.z - s.target.z);
    let len = (back.0 * back.0 + back.1 * back.1 + back.2 * back.2).sqrt();
    let w = (back.0 / len, back.1 / len, back.2 / len);
    let flat = (w.0 * w.0 + w.2 * w.2).sqrt();
    let r = (w.2 / flat, 0.0, -w.0 / flat);
    let u = (-w.0 * w.1 / flat, flat, -w.2 * w.1 / flat);
    let d = (q.x - s.pos.x, q.y - s.pos.y, q.z - s.pos.z);
    let dot = |a: (f64, f64, f64), b: (f64, f64, f64)| a.0 * b.0 + a.1 * b.1 + a.2 * b.2;
    let half = (FOV * PI / 360.0).tan();
    (dot(d, r) / (-dot(d, w) * half * aspect), dot(d, u) / (-dot(d, w) * half))
}

fn dist(s: &Shot) -> f64 {
    ((s.pos.x - s.target.x).powi(2) + (s.pos.y - s.target.y).powi(2) + (s.pos.z - s.target.z).powi(2)).sqrt()
}

#[test]
fn shot_for_frames_the_whole_tower_wider_on_a_narrow_screen() {
    let t = tower_layout(&model(4));
    let wide = shot_for(&Level::Building, &t, 2.0, 0.0);
    let tall = shot_for(&Level::Building, &t, 0.7, 0.0);
    assert!(dist(&tall) > dist(&wide));
    assert_eq!(wide.fov, FOV);
}

#[test]
fn shot_for_keeps_the_roof_the_mast_and_every_corner_in_frame() {
    for (n, aspect) in [(1, 1.3), (5, 1.34), (5, 2.0), (8, 1.34), (8, 0.8), (8, 2.2), (12, 1.34)] {
        let tower = tower_layout(&model(n));
        let s = shot_for(&Level::Building, &tower, aspect, 0.0);
        for q in tower_points(&tower) {
            let (x, y) = in_frame(&s, q, aspect);
            assert!(x.abs() <= 1.0 + 1e-9 && y.abs() <= 1.0 + 1e-9, "{n} floors at {aspect}: {q:?} -> {x}, {y}");
        }
    }
}

#[test]
fn shot_for_moves_the_building_over_for_what_stands_on_the_left() {
    let t = tower_layout(&model(4));
    let s = shot_for(&Level::Building, &t, 1.4, 0.3);
    assert!((s.ox - 0.15).abs() < 1e-9);
    for q in tower_points(&t) {
        assert!(in_frame(&s, q, 1.4 * 0.7).0.abs() <= 1.0 + 1e-9);
    }
    assert_eq!(shot_for(&Level::Floor("f1".into()), &t, 1.4, 0.3).ox, 0.0);
}

#[test]
fn shot_for_gives_up_the_bottom_of_a_very_tall_tower_never_the_top() {
    let (eight, twelve) = (tower_layout(&model(8)), tower_layout(&model(12)));
    assert!(dist(&shot_for(&Level::Building, &twelve, 1.6, 0.0)) <= dist(&shot_for(&Level::Building, &eight, 1.6, 0.0)));
    assert!(tower_points(&twelve).iter().any(|q| q.y >= twelve.lift.y));
    assert!(tower_points(&twelve).iter().map(|q| q.y).fold(f64::INFINITY, f64::min) > twelve.floors[0].y);
}

#[test]
fn shot_for_comes_closer_for_a_floor_closer_again_for_a_desk() {
    let t = tower_layout(&model(4));
    let b = shot_for(&Level::Building, &t, 1.6, 0.0);
    let f = shot_for(&Level::Floor("f1".into()), &t, 1.6, 0.0);
    let d = shot_for(&Level::Desk { person: "x".into(), floor: Some("f1".into()) }, &t, 1.6, 0.0);
    assert!(dist(&f) < dist(&b));
    assert!(dist(&d) < dist(&f));
    assert!(f.target.y > t.floors[1].y && f.target.y < t.floors[1].y + STOREY);
}

#[test]
fn shot_for_frames_the_roof_for_the_founder_and_looks_at_their_desk() {
    let t = tower_layout(&model(4));
    let s = shot_for(&Level::Desk { person: "founder".into(), floor: None }, &t, 1.6, 0.0);
    assert!(s.target.y >= t.roof_y);
    assert_eq!((s.target.x, s.target.z), (FOUNDER_DESK.x, FOUNDER_DESK.z));
}

#[test]
fn chair_spot_stands_on_the_cameras_side_in_the_back_row_and_swaps_for_the_other() {
    let c = chair_spot(Seat { x: 2.0, z: -1.0, rot: 0.0 });
    assert!((c.0 - 2.0).abs() < 1e-9 && c.1 > -1.0);
    assert!(chair_spot(Seat { x: 2.0, z: 1.0, rot: PI }).1 < 1.0);
}

#[test]
fn mix_shots_runs_from_one_shot_to_the_other() {
    let a = shot_for(&Level::Building, &tower_layout(&model(1)), 1.6, 0.0);
    let b = Shot { pos: Point { x: a.pos.x + 10.0, ..a.pos }, ..a };
    assert_eq!(mix_shots(a, b, 0.0), a);
    assert!((mix_shots(a, b, 1.0).pos.x - b.pos.x).abs() < 1e-9);
    assert!((mix_shots(a, b, 0.5).pos.x - (a.pos.x + 5.0)).abs() < 1e-9);
}

#[test]
fn lift_at_rests_most_of_the_time_moves_between_two_and_never_jumps() {
    let stops = [0.0, 4.0, 8.0, 12.0];
    let seen: Vec<f64> = (0..400).map(|i| lift_at(i as f64 * 500.0, &stops)).collect();
    let resting = seen.iter().filter(|y| stops.contains(y)).count();
    assert!(resting > seen.len() / 2 && seen.len() - resting > 0);
    assert!(seen.iter().all(|y| (0.0..=12.0).contains(y)));
    assert_eq!(lift_at(123456.0, &stops), lift_at(123456.0, &stops));
    let mut t = 0.0;
    while t < 200000.0 {
        assert!((lift_at(t + 125.0, &stops) - lift_at(t, &stops)).abs() < 2.0);
        t += 125.0;
    }
    assert_eq!(lift_at(5000.0, &[3.0]), 3.0);
}

#[test]
fn flight_point_leaves_one_floor_rises_along_the_lift_arrives_at_the_other() {
    let t = tower_layout(&model(3));
    assert!((flight_point(&t, Some("f0"), Some("f2"), 0.0).y - (t.floors[0].y + 1.6)).abs() < 1e-9);
    assert!((flight_point(&t, Some("f0"), Some("f2"), 1.0).y - (t.floors[2].y + 1.6)).abs() < 1e-9);
    assert!((flight_point(&t, Some("f0"), Some("f2"), 0.5).x - t.lift.x).abs() < 1e-9);
    assert!((flight_point(&t, None, Some("f0"), 0.0).y - (t.roof_y + 1.6)).abs() < 1e-9);
}
