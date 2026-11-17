//! The small things that rise while people work (life.ts): steam from a mug
//! and bytes off a screen. Each mark is a quad facing the camera; where it is
//! at a moment comes from the clock alone, as the office's vertex shader has
//! it, so nothing is kept between frames.

use std::f64::consts::PI;

use glam::{DVec3, Vec3};

use super::geom::hex;
use super::layout::Seat;
use super::palette::office as o;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Steam,
    Byte,
}

/// One mark: where it starts, what it is, and where in its cycle it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rising {
    pub at: DVec3,
    pub kind: Kind,
    pub seed: f64,
}

/// The desk top as JavaScript has it: build_parts' f32 is a hair under.
const DESK_TOP: f64 = 0.78;
const STEAM: (f64, f64, f64, usize) = (-0.52, DESK_TOP + 0.1, 0.14, 3);
const BYTES: (f64, f64, f64, usize) = (0.0, DESK_TOP + 0.6, -0.2, 4);

fn in_world(d: Seat, floor_y: f64, x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(d.x + x * d.rot.cos() + z * d.rot.sin(), floor_y + y, d.z - x * d.rot.sin() + z * d.rot.cos())
}

/// What rises from a desk: steam from the mug of someone working, and bytes only while their terminal prints.
pub fn rising_at(d: Seat, floor_y: f64, seed: f64, printing: bool) -> Vec<Rising> {
    let mut out: Vec<Rising> = (0..STEAM.3)
        .map(|i| Rising { at: in_world(d, floor_y, STEAM.0, STEAM.1, STEAM.2), kind: Kind::Steam, seed: (seed * 0.37 + i as f64 / STEAM.3 as f64) % 1.0 })
        .collect();
    if printing {
        out.extend((0..BYTES.3).map(|i| Rising {
            at: in_world(d, floor_y, (i as f64 - 1.5) * 0.14, BYTES.1, BYTES.2),
            kind: Kind::Byte,
            seed: (seed * 0.61 + (i * 2) as f64 / (BYTES.3 * 2 + 1) as f64) % 1.0,
        }));
    }
    out
}

/// A desk where someone works or prints: where, at what height, and whether bytes rise.
pub struct Busy {
    pub seat: Seat,
    pub y: f64,
    pub printing: bool,
}

/// Every mark in the building, each desk seeded by its place in the list.
pub fn rising_all(desks: &[Busy]) -> Vec<Rising> {
    desks.iter().enumerate().flat_map(|(i, b)| rising_at(b.seat, b.y, (i + 1) as f64 * 0.173, b.printing)).collect()
}

/// A mark at a moment: its middle, its half size across the screen, how
/// strong it shows (0 to 1), and its colour, linear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkNow {
    pub at: DVec3,
    pub half: f64,
    pub fade: f64,
    pub colour: Vec3,
}

/// Where a mark is `time` seconds in. Steam is round and sways; a byte is a square.
pub fn mark_now(m: &Rising, time: f64) -> MarkNow {
    let steam = if m.kind == Kind::Steam { 1.0 } else { 0.0 };
    let mix = |a: f64, b: f64, k: f64| a + (b - a) * k;
    let period = mix(2.6, 3.4, steam);
    let k = (time / period + m.seed).rem_euclid(1.0);
    let rise = k * mix(0.62, 0.74, steam);
    let sway = (k * 6.0 + m.seed * 40.0).sin() * 0.05 * steam;
    let colour = if m.kind == Kind::Steam { hex("#fff3e2") } else { hex(o::SCREEN_ON) };
    MarkNow { at: m.at + DVec3::new(sway, rise, 0.0), half: mix(0.03, 0.07 + k * 0.08, steam), fade: (k * PI).sin(), colour }
}

/// How much of a mark shows at `corner` (each of x, y from -1 to 1): steam is
/// a soft round puff, a byte a solid square. Laid on additively.
pub fn mark_alpha(kind: Kind, corner: (f64, f64), fade: f64) -> f64 {
    let t = ((corner.0.hypot(corner.1) - 0.1) / 0.9).clamp(0.0, 1.0);
    let round = 1.0 - t * t * (3.0 - 2.0 * t);
    match kind {
        Kind::Steam => round * 0.42 * fade,
        Kind::Byte => 0.85 * fade,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_rise_and_fade_over_a_cycle() {
        let d = Seat { x: 1.0, z: -0.5, rot: 0.0 };
        let marks = rising_at(d, 4.5, 0.173, true);
        assert_eq!(marks.len(), 7);
        let start = mark_now(&marks[3], 0.0);
        let later = mark_now(&marks[3], 0.6);
        assert!(later.at.y > start.at.y, "a byte rises");
        assert_eq!(rising_at(d, 4.5, 0.173, false).len(), 3, "no bytes without printing");
        let m = Rising { at: DVec3::ZERO, kind: Kind::Steam, seed: 0.0 };
        assert!(mark_now(&m, 0.0).fade.abs() < 1e-9 && (mark_now(&m, 1.7).fade - 1.0).abs() < 1e-9, "it fades in and out");
        assert!(mark_alpha(Kind::Steam, (1.0, 1.0), 1.0) == 0.0 && mark_alpha(Kind::Steam, (0.0, 0.0), 1.0) > 0.4);
    }

    /// Against the office's own risingAt (fixtures/life.json).
    #[test]
    fn marks_start_where_the_office_puts_them() {
        let all: serde_json::Value = serde_json::from_str(include_str!("fixtures/life.json")).unwrap();
        let num = |v: &serde_json::Value| v.as_f64().unwrap();
        for c in all.as_array().unwrap() {
            let d = Seat { x: num(&c["d"]["x"]), z: num(&c["d"]["z"]), rot: num(&c["d"]["rot"]) };
            let got = rising_at(d, num(&c["y"]), num(&c["seed"]), c["printing"] == true);
            let want = c["marks"].as_array().unwrap();
            assert_eq!(got.len(), want.len());
            for (g, w) in got.iter().zip(want) {
                for (a, b) in [(g.at.x, num(&w["x"])), (g.at.y, num(&w["y"])), (g.at.z, num(&w["z"])), (g.seed, num(&w["seed"]))] {
                    assert!((a - b).abs() < 1e-12, "{a} vs {b}");
                }
                assert_eq!(g.kind == Kind::Byte, num(&w["kind"]) == 1.0);
            }
        }
    }
}
