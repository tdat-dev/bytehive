//! What a person in the office is doing, as people.ts poses them: a pose from
//! their state and the clock alone, so nothing is kept between frames, and the
//! rig, where each moving part of them stands for that pose. The shapes are in
//! people_shapes.rs.

use std::f64::consts::PI;

use glam::{DMat4, DVec3};

use super::layout::{Seat, chair_spot};

/// How someone in the office is, as the office model reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Needs,
    Working,
    Review,
    Idle,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Work,
    Off,
    Dim,
}

/// Everything a state looks like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// 0 standing, 1 seated.
    pub sit: f64,
    /// The chair back, radians.
    pub lean: f64,
    pub arm_l: f64,
    pub arm_r: f64,
    /// The top half, turned from the desk, radians.
    pub turn: f64,
    pub screen: Screen,
    pub lamp: bool,
    pub bubble: bool,
    /// False: an empty chair.
    pub present: bool,
}

const STILL: Pose = Pose { sit: 1.0, lean: 0.0, arm_l: 0.0, arm_r: 0.0, turn: 0.0, screen: Screen::Dim, lamp: false, bubble: false, present: true };
const GLANCE_EVERY: f64 = 7000.0;
const GLANCE_TAKES: f64 = 1500.0;
const GLANCE_FAR: f64 = 0.42;

/// A look away from the screen, every few seconds.
fn glance(t: f64, seat: f64) -> f64 {
    let at = (t + seat * 2300.0) % GLANCE_EVERY;
    if at >= GLANCE_TAKES {
        return 0.0;
    }
    let side = if ((t + seat * 2300.0) / GLANCE_EVERY).floor() % 2.0 != 0.0 { 1.0 } else { -1.0 };
    side * GLANCE_FAR * ((at / GLANCE_TAKES) * PI).sin().powi(2)
}

/// A person's whole picture from their state and the clock (ms).
pub fn pose(state: State, t: f64, seat: f64) -> Pose {
    match state {
        State::Needs => Pose { sit: 0.0, lamp: true, bubble: true, ..STILL },
        State::Working => {
            let phase = t / 125.0 + seat * 1.7;
            Pose { screen: Screen::Work, arm_l: phase.sin() * 0.18, arm_r: (phase + 2.0).sin() * 0.18, turn: glance(t, seat), ..STILL }
        }
        State::Review => Pose { arm_r: 0.5, ..STILL },
        State::Idle => Pose { lean: 0.32, ..STILL },
        State::Stopped => Pose { present: false, screen: Screen::Off, ..STILL },
    }
}

/* ---- sizes: everything from the hip, with -z as forward ---- */

const SIT_HIP: f64 = 0.6;
const STAND_HIP: f64 = 0.7;
pub const HEAD_UP: f64 = 1.06;
pub const HEAD_R: f64 = 0.49;
pub const SHOULDER: (f64, f64) = (0.34, 0.58);
pub const HAND: f64 = -0.54;
/// Arms reach for the desk sitting down, and hang on their feet.
pub const ARM_SIT: f64 = 1.02;
const ARM_STAND: f64 = 0.12;
/// Leaning back, the hands come off the desk, this much for each radian.
const ARM_REST: f64 = 2.2;
/// Out of the chair toward the viewer: round the back of a desk, or round
/// the end of one turned to face the viewer.
const STEP_BACK: (f64, f64) = (0.72, 0.85);
const STEP_FRONT: (f64, f64) = (1.1, 0.05);
const BUBBLE_GAP: f64 = 0.12;

fn step_of(rot: f64) -> (f64, f64) {
    if rot.cos() > 0.0 { STEP_BACK } else { STEP_FRONT }
}

/// The fixed view's bearing, for what turns to face it.
pub fn view_yaw() -> f64 {
    1f64.atan2(0.84)
}

/// Facing the viewer, with -z as forward.
fn face_viewer() -> f64 {
    view_yaw() + PI
}

fn hip_at(sit: f64) -> f64 {
    STAND_HIP + (SIT_HIP - STAND_HIP) * sit
}

fn bubble_at(sit: f64) -> f64 {
    hip_at(sit) + HEAD_UP + HEAD_R + BUBBLE_GAP
}

/// Where someone in this state is, for a click and a label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub x: f64,
    pub z: f64,
    pub head: f64,
    pub bubble: f64,
}

pub fn spot_of(d: Seat, state: State) -> Spot {
    let sit = pose(state, 0.0, 0.0).sit;
    let (cx, cz) = chair_spot(d);
    let step = step_of(d.rot);
    Spot { x: cx + step.0 * (1.0 - sit), z: cz + step.1 * (1.0 - sit), head: hip_at(sit) + HEAD_UP, bubble: bubble_at(sit) }
}

pub const HOP_MS: f64 = 460.0;
const HOP_UP: f64 = 0.46;

/// A small jump, from when it started: how high off the ground.
pub fn hop_at(ms: f64) -> f64 {
    if ms <= 0.0 || ms >= HOP_MS { 0.0 } else { HOP_UP * 4.0 * (ms / HOP_MS) * (1.0 - ms / HOP_MS) }
}

/// Where each part of a person stands, in the world: what applyPose sets on
/// the parts and three works out down the tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    pub visible: bool,
    /// The torso and head, and the badge on it.
    pub trunk: DMat4,
    pub legs: DMat4,
    pub arm_l: DMat4,
    pub arm_r: DMat4,
    /// The mark over someone who needs you, when it shows.
    pub bubble: Option<DMat4>,
}

/// A person at a desk on a floor at height `y`, in pose `p`, `raise` off the ground.
pub fn rig(d: Seat, y: f64, p: &Pose, raise: f64, marked: bool) -> Rig {
    let (cx, cz) = chair_spot(d);
    let root = DMat4::from_translation(DVec3::new(cx, y, cz));
    let up = 1.0 - p.sit;
    let reach = ARM_STAND + (ARM_SIT - ARM_STAND) * p.sit - p.lean * ARM_REST;
    let step = step_of(d.rot);
    let facing = if p.sit < 0.5 { face_viewer() } else { d.rot };
    let body = root * DMat4::from_translation(DVec3::new(step.0 * up, hip_at(p.sit) + raise, step.1 * up)) * DMat4::from_rotation_y(facing);
    // three's Euler XYZ: the X turn, then the Y turn inside it.
    let trunk = body * DMat4::from_rotation_x(p.lean) * DMat4::from_rotation_y(p.turn);
    let arm = |x: f64, swing: f64| trunk * DMat4::from_translation(DVec3::new(x, SHOULDER.1, 0.0)) * DMat4::from_rotation_x(reach + swing);
    let bubble = (p.bubble || marked).then(|| root * DMat4::from_translation(DVec3::new(step.0 * up, bubble_at(p.sit), step.1 * up)) * DMat4::from_rotation_y(view_yaw()));
    Rig { visible: p.present, trunk, legs: body * DMat4::from_rotation_x(PI / 2.0 * p.sit), arm_l: arm(-SHOULDER.0, p.arm_l), arm_r: arm(SHOULDER.0, p.arm_r), bubble }
}
