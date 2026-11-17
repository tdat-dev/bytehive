//! Where everything stands, in scene units (layout.ts): the tower's floors and
//! desks, and the camera's shots, as numbers the scene only draws. Worked in
//! f64 as JavaScript does.

use std::f64::consts::PI;

pub const STOREY: f64 = 4.0;
pub const SLAB: f64 = 0.56;
pub const ROOM_W: f64 = 10.4;
pub const ROOM_D: f64 = 6.8;
pub const LIFT_W: f64 = 1.7;
pub const GROUND: f64 = 0.5;
/// A long lens: the walls stay upright, every floor seen from the same height.
pub const FOV: f64 = 15.0;
/// How long a message is in the air, for the paper plane.
pub const FLIGHT_MS: f64 = 2400.0;
/// One line across the screen in two rows: near on the left, far on the right.
const NEAR: f64 = 1.75;
const FAR: f64 = -1.6;
const REACH: f64 = 3.95;
const WIDEST: f64 = 2.4;
const MOST: usize = 10;
/// Past this many floors the Building shot keeps the top and lets the bottom go.
const MAX_FRAMED: usize = 8;
/// The plot round the tower.
pub const PLOT: (f64, f64, f64, f64) = (21.0, 17.0, 0.85, 0.4);
/// About 50 degrees round from the front, and low.
pub const VIEW: Point = Point { x: 1.0, y: 0.2, z: 0.84 };
const CHAIR_BACK: f64 = 0.78;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

const fn p(x: f64, y: f64, z: f64) -> Point {
    Point { x, y, z }
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn view_len() -> f64 {
    (VIEW.x * VIEW.x + VIEW.y * VIEW.y + VIEW.z * VIEW.z).sqrt()
}

/// Toward the camera, and the screen's right and up, in the world.
fn aim() -> Point {
    let l = view_len();
    p(VIEW.x / l, VIEW.y / l, VIEW.z / l)
}

pub fn right() -> Point {
    let a = aim();
    let flat = (a.x * a.x + a.z * a.z).sqrt();
    p(a.z / flat, 0.0, -a.x / flat)
}

fn up() -> Point {
    let a = aim();
    let flat = (a.x * a.x + a.z * a.z).sqrt();
    p(-a.x * a.y / flat, flat, -a.z * a.y / flat)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    pub x: f64,
    pub z: f64,
    pub rot: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Desk {
    pub person_id: String,
    pub seat: Seat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FloorSpot {
    pub id: String,
    pub y: f64,
    pub desks: Vec<Desk>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tower {
    pub floors: Vec<FloorSpot>,
    pub roof_y: f64,
    /// The lift: x, z, and the top of its shaft.
    pub lift: Point,
    pub width: f64,
}

/// A camera: where it is, what it looks at, its lens, and `ox`, how far the
/// picture slides right (a share of the frame) to clear what stands on the left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub pos: Point,
    pub target: Point,
    pub fov: f64,
    pub ox: f64,
}

/// Where the camera goes (nav.ts Level).
#[derive(Clone, Debug, PartialEq)]
pub enum Level {
    Building,
    Floor(String),
    Desk { person: String, floor: Option<String> },
}

impl Default for Level {
    fn default() -> Self {
        Level::Building
    }
}

/// The Founder's desk, out on the roof in front of the penthouse.
pub const FOUNDER_DESK: Seat = Seat { x: -1.6, z: 1.4, rot: PI };

/// Where the chair stands: at rot 0 on the camera's side of the desk.
pub fn chair_spot(d: Seat) -> (f64, f64) {
    (d.x + d.rot.sin() * CHAIR_BACK, d.z + d.rot.cos() * CHAIR_BACK)
}

/// How many places a floor of this many is laid out for.
fn places_for(count: usize) -> usize {
    match count {
        0..=4 => 4,
        5..=6 => 6,
        7..=8 => 8,
        _ => MOST,
    }
}

/// From the middle of the line outward.
fn middle_out(places: usize, seat: usize) -> f64 {
    let half = (places / 2) as f64;
    if seat % 2 == 1 { half + (seat - 1) as f64 / 2.0 } else { half - 1.0 - seat as f64 / 2.0 }
}

/// Everyone faces the viewer from behind their desk; past ten, a third row.
pub fn seat_spot(seat: usize, count: usize) -> Seat {
    let places = places_for(count);
    if seat >= MOST {
        let k = seat - MOST;
        return Seat { x: ((k % 5) as f64 - 2.0) * 2.0, z: CHAIR_BACK + (k / 5) as f64 * 0.2, rot: PI };
    }
    let i = middle_out(places, seat);
    let n = places as f64;
    let gap = WIDEST.min(2.0 * REACH / (n - 1.0));
    let across = (i - (n - 1.0) / 2.0) * gap;
    let chair_z = if i < n / 2.0 { NEAR } else { FAR };
    let r = right();
    Seat { x: (across - r.z * chair_z) / r.x, z: chair_z + CHAIR_BACK, rot: PI }
}

/// The tower for these floors: (floor id, people's ids in seat order).
pub fn tower_layout(floors: &[(String, Vec<String>)]) -> Tower {
    let spots = floors
        .iter()
        .enumerate()
        .map(|(i, (id, people))| FloorSpot {
            id: id.clone(),
            y: GROUND + i as f64 * STOREY,
            desks: people.iter().enumerate().map(|(seat, pid)| Desk { person_id: pid.clone(), seat: seat_spot(seat, people.len()) }).collect(),
        })
        .collect();
    let roof_y = GROUND + floors.len() as f64 * STOREY;
    Tower { floors: spots, roof_y, lift: p(ROOM_W / 2.0 + LIFT_W / 2.0, roof_y + STOREY, -ROOM_D / 2.0 + LIFT_W / 2.0), width: ROOM_W + LIFT_W }
}

/// A shot from the fixed angle, far enough to hold a box of this size.
fn shot(tx: f64, ty: f64, tz: f64, w: f64, h: f64, aspect: f64) -> Shot {
    let half = (FOV * PI / 360.0).tan();
    let dist = (h / 2.0 / half).max(w / 2.0 / (half * aspect)) * 1.18;
    let k = dist / view_len();
    Shot { pos: p(tx + VIEW.x * k, ty + VIEW.y * k, tz + VIEW.z * k), target: p(tx, ty, tz), fov: FOV, ox: 0.0 }
}

/// The shot that holds every point, as the lens sees them. `room`: the share
/// of the frame the points may fill.
pub fn fit_shot(points: &[Point], aspect: f64, room: f64) -> Shot {
    let (r, u, a) = (right(), up(), aim());
    let us: Vec<f64> = points.iter().map(|q| dot(*q, r)).collect();
    let vs: Vec<f64> = points.iter().map(|q| dot(*q, u)).collect();
    let ws: Vec<f64> = points.iter().map(|q| dot(*q, a)).collect();
    let mid = |xs: &[f64]| (xs.iter().cloned().fold(f64::INFINITY, f64::min) + xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0;
    let (uc, vc, wc) = (mid(&us), mid(&vs), mid(&ws));
    let half = (FOV * PI / 360.0).tan() * room;
    let dist = (0..points.len()).map(|i| ((us[i] - uc).abs() / (half * aspect)).max((vs[i] - vc).abs() / half) + ws[i] - wc).fold(f64::NEG_INFINITY, f64::max);
    let t = p(r.x * uc + u.x * vc + a.x * wc, r.y * uc + u.y * vc + a.y * wc, r.z * uc + u.z * vc + a.z * wc);
    Shot { pos: p(t.x + a.x * dist, t.y + a.y * dist, t.z + a.z * dist), target: t, fov: FOV, ox: 0.0 }
}

/// What the Building shot must hold: the mast, the roof's corners, the plot.
pub fn tower_points(t: &Tower) -> Vec<Point> {
    let tall = t.floors.len() > MAX_FRAMED;
    let floor = if tall { t.roof_y - MAX_FRAMED as f64 * STOREY } else { -0.4 };
    let slab = (-ROOM_W / 2.0 - 0.3, ROOM_W / 2.0 + LIFT_W + 0.3, ROOM_D / 2.0 + 0.3, -ROOM_D / 2.0 - 0.3);
    let plot = (PLOT.2 - PLOT.0 / 2.0, PLOT.2 + PLOT.0 / 2.0, PLOT.3 + PLOT.1 / 2.0, PLOT.3 - PLOT.1 / 2.0);
    let corners = |b: (f64, f64, f64, f64)| [(b.0, b.2), (b.1, b.2), (b.0, b.3), (b.1, b.3)];
    let mut out: Vec<Point> = corners(slab).iter().map(|c| p(c.0, t.roof_y, c.1)).collect();
    out.extend(corners(if tall { slab } else { plot }).iter().map(|c| p(c.0, floor, c.1)));
    out.push(p(t.lift.x, t.lift.y, t.lift.z));
    out
}

/// A floor's own corners, for the Floor shot.
fn floor_points(f: &FloorSpot) -> Vec<Point> {
    let side = [(-ROOM_W / 2.0, ROOM_D / 2.0 + 0.2), (ROOM_W / 2.0 + LIFT_W, ROOM_D / 2.0 + 0.2), (ROOM_W / 2.0 + LIFT_W, -ROOM_D / 2.0), (-ROOM_W / 2.0, -ROOM_D / 2.0)];
    let mut out: Vec<Point> = side.iter().map(|c| p(c.0, f.y - 0.1, c.1)).collect();
    out.extend(side.iter().map(|c| p(c.0, f.y + STOREY - SLAB, c.1)));
    out
}

/// The shot for a level. `clear`: the share of the frame's width on the left
/// the Building shot leaves free for the job box.
pub fn shot_for(l: &Level, t: &Tower, aspect: f64, clear: f64) -> Shot {
    match l {
        Level::Desk { floor: None, .. } => shot(FOUNDER_DESK.x, t.roof_y + 1.2, FOUNDER_DESK.z, ROOM_W * 0.7, STOREY * 1.05, aspect),
        Level::Floor(id) | Level::Desk { floor: Some(id), .. } if t.floors.iter().any(|f| &f.id == id) => {
            let f = t.floors.iter().find(|f| &f.id == id).unwrap();
            match l {
                Level::Floor(_) => fit_shot(&floor_points(f), aspect, 1.0),
                Level::Desk { person, .. } => {
                    let d = f.desks.iter().find(|d| &d.person_id == person).map(|d| d.seat);
                    shot(d.map_or(0.0, |d| d.x), f.y + 1.1, d.map_or(0.0, |d| d.z), ROOM_W * 0.62, STOREY * 0.86, aspect)
                }
                Level::Building => unreachable!(),
            }
        }
        _ => Shot { ox: clear / 2.0, ..fit_shot(&tower_points(t), aspect * (1.0 - clear), 0.92) },
    }
}

pub fn mix_shots(a: Shot, b: Shot, k: f64) -> Shot {
    let at = |x: f64, y: f64| x + (y - x) * k;
    let pt = |x: Point, y: Point| p(at(x.x, y.x), at(x.y, y.y), at(x.z, y.z));
    Shot { pos: pt(a.pos, b.pos), target: pt(a.target, b.target), fov: at(a.fov, b.fov), ox: at(a.ox, b.ox) }
}

const LEG_MS: f64 = 11_000.0;
const GLIDE_MS: f64 = 4_200.0;

/// `(Math.imul(leg + 1, 2654435761) >>> 8) % n`, 32-bit as JavaScript does it.
fn stop_of(leg: i64, n: usize) -> usize {
    let m = (leg as i32).wrapping_add(1).wrapping_mul(2654435761u32 as i32);
    ((m as u32) >> 8) as usize % n
}

/// Where the lift's car is at `now`: resting at a floor or gliding to the next.
pub fn lift_at(now: f64, stops: &[f64]) -> f64 {
    if stops.len() < 2 {
        return stops.first().copied().unwrap_or(0.0);
    }
    let leg = (now / LEG_MS).floor();
    let from = stops[stop_of(leg as i64, stops.len())];
    let to = stops[stop_of(leg as i64 + 1, stops.len())];
    let k = ((now - leg * LEG_MS) / GLIDE_MS).min(1.0);
    if k >= 1.0 { to } else { from + (to - from) * (k * k * (3.0 - 2.0 * k)) }
}

const CRUISE: f64 = 1.6;

/// A paper plane at `k` (0 to 1): out to the lift, along it, in to the floor.
pub fn flight_point(t: &Tower, from: Option<&str>, to: Option<&str>, k: f64) -> Point {
    let y_of = |id: Option<&str>| t.floors.iter().find(|f| Some(f.id.as_str()) == id).map_or(t.roof_y, |f| f.y) + CRUISE;
    let (a, b) = (y_of(from), y_of(to));
    let mid = 0.15;
    if k < mid {
        return p(t.lift.x * k / mid, a, t.lift.z * k / mid);
    }
    if k > 1.0 - mid {
        return p(t.lift.x * (1.0 - k) / mid, b, t.lift.z * (1.0 - k) / mid);
    }
    p(t.lift.x, a + (b - a) * (k - mid) / (1.0 - 2.0 * mid), t.lift.z)
}
