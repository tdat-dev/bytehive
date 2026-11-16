//! Everything in a floor but its desks (props.ts furnish): from the floor's
//! name and its height in the tower, so no two floors match and a floor keeps
//! its look. The random numbers are drawn in props.ts's order, every one, or
//! the rooms would differ from the Tauri office's. props.ts yields between
//! steps to spread the work over frames; the result is the same at once.

use std::f32::consts::PI;

use super::geom::Geom;
use super::glow::{flare, pool};
use super::layout::{ROOM_D, ROOM_W};
use super::light::{Blocker, Lamp, Rgb, rgb};
use super::palette::ACCENTS;
use super::props::{self as p, Prop, WARM_LAMP};
use super::props_wall::{self as w, PENDANT_Y, Pendant};
use super::seed::{Seeded, hash_of};

const WALL: f64 = 0.34;
const BACK_Z: f32 = (-ROOM_D / 2.0 + WALL / 2.0) as f32;
const LEFT_X: f32 = (-ROOM_W / 2.0 + WALL / 2.0) as f32;

#[derive(Default)]
pub struct Furnished {
    pub solid: Vec<Geom>,
    pub lit: Vec<Geom>,
    pub thrown: Vec<Geom>,
    pub lamps: Vec<Lamp>,
    pub blockers: Vec<Blocker>,
    pub accent: &'static str,
}

/// A floor to furnish: its id, its place in the tower, people in, a board on the right.
pub struct RoomPlan<'a> {
    pub id: &'a str,
    pub index: u32,
    pub lit: bool,
    pub board: bool,
}

/// The colour of a floor: from its name, stepped by its height so two floors in a row differ.
pub fn accent_of(name: &str, index: u32) -> &'static str {
    ACCENTS[((hash_of(name) as u64 + index as u64 * 3) % ACCENTS.len() as u64) as usize]
}

/// The colours of a room's walls and tiles, lit or left for the night.
pub fn room_ambient(lit: bool) -> Rgb {
    if lit { [0.68, 0.62, 0.55] } else { [0.56, 0.62, 0.86] }
}

fn shuffled<T: Copy>(list: &[T], rand: &mut Seeded) -> Vec<T> {
    let mut out = list.to_vec();
    for i in (1..out.len()).rev() {
        let j = (rand.next() * (i + 1) as f64).floor() as usize;
        out.swap(i, j);
    }
    out
}

fn spin(x: f32, z: f32, turn: f32) -> (f32, f32) {
    (x * turn.cos() + z * turn.sin(), -x * turn.sin() + z * turn.cos())
}

/// JavaScript's ToUint32 of an integer held in a double: what `seed >>> 0` keeps.
fn js_u32(n: f64) -> u32 {
    (n.rem_euclid(4_294_967_296.0)) as u32
}

#[derive(Clone, Copy)]
enum Thing {
    Sofa,
    Beanbags,
    Coffee,
    Meeting,
    PlantCabinet,
    Bookshelf(f64, f64, u32),
    Rack,
    Arcade,
    WallArt,
    Clock,
    WindowBay,
    Whiteboard,
    Plant(f64),
    CoatRack,
}

fn make(t: Thing, accent: &str, rand: &mut Seeded) -> Prop {
    match t {
        Thing::Sofa => p::sofa(accent),
        Thing::Beanbags => p::beanbags(accent),
        Thing::Coffee => p::coffee_counter(),
        Thing::Meeting => p::meeting_table(accent),
        Thing::PlantCabinet => p::plant_cabinet(rand),
        Thing::Bookshelf(wd, h, rows) => p::bookshelf(rand, wd, h, rows),
        Thing::Rack => p::rack(),
        Thing::Arcade => p::arcade(accent),
        Thing::WallArt => w::wall_art(accent, rand),
        Thing::Clock => w::clock(),
        Thing::WindowBay => w::window_bay(rand),
        Thing::Whiteboard => w::whiteboard(accent, rand),
        Thing::Plant(h) => p::plant(rand, h),
        Thing::CoatRack => p::coat_rack(),
    }
}

impl Furnished {
    fn put(&mut self, pr: Prop, x: f32, z: f32, turn: f32) {
        self.solid.extend(pr.solid.into_iter().map(|g| g.moved(x, 0.0, z, turn)));
        self.lit.extend(pr.lit.into_iter().map(|g| g.moved(x, 0.0, z, turn)));
        for g in pr.glows {
            let (ax, az) = spin(g.x, g.z, turn);
            self.thrown.push(flare(x + ax, g.y, z + az, g.size, g.hex, g.level));
        }
        for l in pr.lamps {
            let (ax, az) = spin(l.x as f32, l.z as f32, turn);
            self.lamps.push(Lamp { x: (x + ax) as f64, z: (z + az) as f64, ..l });
        }
        if let Some((fw, fd)) = pr.foot {
            let (cx, cz) = spin(0.0, (fd / 2.0) as f32, turn);
            let sideways = turn.sin().abs() > 0.5;
            let (bw, bd) = if sideways { (fd, fw) } else { (fw, fd) };
            self.blockers.push(Blocker { x: (x + cx) as f64, z: (z + cz) as f64, w: bw, d: bd, dark: 0.5 });
        }
    }
}

/// Everything in a floor but its desks.
pub fn furnished(room: &RoomPlan) -> Furnished {
    let hash = hash_of(room.id);
    let mut rand = Seeded::new(js_u32(hash as f64 * 31.0 + room.index as f64 * 7919.0));
    let accent = accent_of(room.id, room.index);
    let mut out = Furnished { accent, ..Default::default() };
    let r = &mut rand;

    // The back wall: left, middle, and right unless the board has it.
    let low = shuffled(&[Thing::Sofa, Thing::Beanbags, Thing::Coffee, Thing::Meeting, Thing::PlantCabinet], r);
    let tall = shuffled(&[Thing::Bookshelf(2.5, 2.7, 4), Thing::Rack, Thing::Arcade], r);
    let hung = shuffled(&[Thing::WallArt, Thing::Clock, Thing::WallArt], r);
    let on_wall = shuffled(&[Thing::WindowBay, Thing::Whiteboard, Thing::WindowBay], r);
    let slots = [(-3.0 + r.next() * 0.3) as f32, (0.2 + r.next() * 0.3) as f32, 3.1f32];

    if room.lit {
        // People are working here, and they are the picture: one quiet corner, one thing on the wall, one plant.
        let quiet = shuffled(&[Thing::Sofa, Thing::Coffee, Thing::PlantCabinet], r);
        let pr = make(quiet[0], accent, r);
        out.put(pr, slots[0], BACK_Z, 0.0);
        let pr = if room.index % 2 == 1 { w::window_bay(r) } else { make(hung[0], accent, r) };
        out.put(pr, LEFT_X, -1.5, PI / 2.0);
        let pr = if r.next() < 0.5 { p::plant(r, 1.7) } else { p::snake_plant() };
        out.put(pr, LEFT_X + 0.1, 2.5, PI / 2.0);
    } else {
        // Nobody is in, and the room can be a room.
        let pr = make(low[0], accent, r);
        out.put(pr, slots[0], BACK_Z, 0.0);
        let pr = make(hung[0], accent, r);
        out.put(pr, slots[0], BACK_Z, 0.0);
        let pr = make(low[1], accent, r);
        out.put(pr, slots[1], BACK_Z, 0.0);
        let pr = make(hung[1], accent, r);
        out.put(pr, slots[1], BACK_Z, 0.0);
        if !room.board {
            let pr = make(tall[0], accent, r);
            out.put(pr, slots[2], BACK_Z, 0.0);
        }
        let pr = make(on_wall[0], accent, r);
        out.put(pr, LEFT_X, -1.5, PI / 2.0);
        let standing = shuffled(&[Thing::Bookshelf(1.9, 2.3, 3), Thing::Plant(2.1), Thing::Arcade, Thing::CoatRack], r);
        let pr = make(standing[0], accent, r);
        out.put(pr, LEFT_X, 1.6, PI / 2.0);
        let pr = if r.next() < 0.5 { p::snake_plant() } else { p::plant(r, 1.7) };
        out.put(pr, LEFT_X + 0.1, -2.75, PI / 2.0);
        out.solid.extend(w::hex_rug(&super::colour::shift(accent, "#6a6a72", 0.42), 0.0, 0.06, 2.7));
        if r.next() < 0.45 {
            let pr = if r.next() < 0.5 { p::plant(r, 1.5) } else { p::snake_plant() };
            out.put(pr, -4.2, 2.75, 0.0);
        }
    }
    if r.next() < 0.5 {
        out.put(w::cat(["#3a3d4a", "#d8d2c8", "#8a8f9c"][(hash % 3) as usize]), 3.2, 2.7, 0.6);
    }

    // The lamps. Nobody in, and it is the sky that lights the room, and one lamp left on.
    let kind = [Pendant::Dome, Pendant::Globe, Pendant::Cone][(hash % 3) as usize];
    for x in [-3.1f32, 0.0, 3.1] {
        let (solid, lit, glows) = w::pendant(kind, accent, x, 0.1, room.lit);
        out.solid.extend(solid);
        out.lit.extend(lit);
        for g in glows {
            out.thrown.push(flare(g.x, g.y, g.z, g.size, g.hex, g.level));
        }
        if room.lit {
            out.thrown.push(pool(x, 0.03, 0.1, 5.4, 4.6, "#ffcf9e", 0.09));
            out.lamps.push(Lamp { x: x as f64, y: (PENDANT_Y - 0.1) as f64, z: 0.1, colour: rgb(WARM_LAMP), power: 1.05, reach: 2.2 });
        }
    }
    if !room.lit {
        out.put(p::floor_lamp(true), -1.25, BACK_Z, 0.0);
    }
    out
}
