//! The office as it sits on the card (scene.ts setModel): the setting once,
//! the roof for its height, a floor for each project and the Founder's desk,
//! each geometry uploaded with the look it is drawn with. A floor whose key
//! has not moved is left alone; one whose room has not moved keeps its room.

use std::collections::HashMap;

use glam::Vec3;

use super::clay::{Clay, INDOOR, MATTE, NIGHT};
use super::floor::{Room, floor_for, room_for, top_for};
use super::geom::Geom;
use super::layout::{FOUNDER_DESK, FloorSpot, Seat, Tower};
use super::life::{Busy, Rising, rising_all};
use super::palette::office as o;
use super::people_pose::State;
use super::people_shapes::{badge_geometry, bubble_geometry, shapes_of};
use super::people_desk::plane_geometry;
use super::render::Renderer;
use super::render_types::{Look, MeshId, POOL, TexId, WHITE};
use super::roof::roof;
use super::setting::{lamp_globes, lamp_glows, plot, spill};
use super::sky::{moon, moon_halo, paint_sky, sky_sheet, star_field};
use super::town::{field, town};
use super::world_in::{FloorIn, OfficeIn, PersonIn};

/// Which frame a part sits in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    World,
    /// The sky and the town: turned to the viewer and shifted by the pointer.
    Far,
    /// The field: turned to the viewer.
    Field,
    /// Lifted to a floor's height.
    Up(f32),
}

/// A geometry on the card and how it is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub mesh: MeshId,
    pub look: Look,
    pub clay: Option<Clay>,
    pub falls_back: bool,
    pub cast: bool,
    /// three's renderOrder.
    pub order: i32,
    pub texture: TexId,
    pub tint: [f32; 4],
    pub place: Place,
    /// Off when nothing gives it light (the spill with every room dark).
    pub on: bool,
}

impl Part {
    fn new(mesh: MeshId, look: Look, place: Place) -> Self {
        let texture = if matches!(look, Look::Light | Look::LightOver) { POOL } else { WHITE };
        Self { mesh, look, clay: None, falls_back: false, cast: false, order: 0, texture, tint: [1.0; 4], place, on: true }
    }

    fn clay(mesh: MeshId, clay: Clay, place: Place) -> Self {
        let tint = if clay.self_ == INDOOR.self_ { super::geom::hex("#fff1d8").extend(1.0).to_array() } else { [1.0; 4] };
        Self { clay: Some(clay), falls_back: true, cast: true, tint, ..Self::new(mesh, Look::Lit, place) }
    }

    fn ordered(self, order: i32) -> Self {
        Self { order, ..self }
    }
}

/// Someone in the scene: their shapes, where they sit, how they are.
pub struct Body {
    pub id: String,
    pub seat: Seat,
    pub y: f64,
    pub state: State,
    pub seat_no: f64,
    pub marked: bool,
    pub clay: Clay,
    pub torso: MeshId,
    pub legs: MeshId,
    pub arm_l: MeshId,
    pub arm_r: MeshId,
    pub badge: TexId,
    pub badge_ink: Vec3,
}

struct Built {
    key: String,
    room_key: String,
    room: Option<Room>,
    parts: Vec<Part>,
}

pub struct World {
    pub setting: Vec<Part>,
    /// The spill of the rooms' light on the ground: on when a room is lit.
    pub spill: usize,
    pub roof: Vec<Part>,
    roof_key: String,
    pub car: Option<(MeshId, Vec3, Vec<f32>)>,
    pub logo: Option<(MeshId, Vec3, f32)>,
    floors: HashMap<String, Built>,
    pub order: Vec<String>,
    pub top: Vec<Part>,
    pub bodies: Vec<Body>,
    shapes: HashMap<String, [MeshId; 4]>,
    badges: HashMap<String, (TexId, Vec3)>,
    pub bubble: MeshId,
    pub badge: MeshId,
    pub plane: MeshId,
    pub logo_tex: TexId,
    /// Steam and bytes over the desks where someone works.
    pub rising: Vec<Rising>,
}

fn up(r: &mut Renderer, g: &Geom) -> MeshId {
    r.geom(g)
}

/// The stars, as squares facing the camera (normal: corner x, corner y).
fn star_sprites() -> Vec<super::gpu::Vertex> {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    star_field().iter().flat_map(|s| corners.map(|(x, y)| super::gpu::Vertex { pos: s.at.to_array(), normal: [x, y, 0.0], color: s.colour.to_array() })).collect()
}

impl World {
    pub fn new(r: &mut Renderer, logo: TexId) -> Self {
        let sky = r.picture(paint_sky(1024, 512), 1024, 512);
        let mut setting = vec![
            Part { texture: sky, ..Part::new(up(r, &sky_sheet()), Look::Backdrop, Place::Far) }.ordered(-3),
            Part::new(r.mesh(&star_sprites()), Look::Star, Place::Far).ordered(-2),
            Part::new(up(r, &moon()), Look::Flat, Place::Far).ordered(-2),
            Part::new(up(r, &moon_halo()), Look::Light, Place::Far).ordered(-2),
            Part::new(up(r, &field()), Look::Flat, Place::Field),
            Part::clay(up(r, &plot()), MATTE, Place::World),
            Part::new(up(r, &lamp_glows()), Look::Light, Place::World).ordered(1),
            Part::new(up(r, &lamp_globes()), Look::Flat, Place::World),
            Part::new(up(r, &town()), Look::Flat, Place::Far),
        ];
        setting.push(Part::new(up(r, &spill()), Look::Light, Place::World).ordered(1));
        let spill = setting.len() - 1;
        Self {
            setting,
            spill,
            roof: vec![],
            roof_key: String::new(),
            car: None,
            logo: None,
            floors: HashMap::new(),
            order: vec![],
            top: vec![],
            bodies: vec![],
            shapes: HashMap::new(),
            badges: HashMap::new(),
            bubble: up(r, &bubble_geometry()),
            badge: up(r, &badge_geometry()),
            plane: up(r, &plane_geometry()),
            logo_tex: logo,
            rising: vec![],
        }
    }

    fn free(r: &mut Renderer, parts: &[Part]) {
        for p in parts {
            r.free(p.mesh);
        }
    }

    fn build_roof(&mut self, r: &mut Renderer, t: &Tower) {
        let key = format!("{:?}", (t.roof_y, t.floors.iter().map(|f| f.y).collect::<Vec<_>>()));
        if key == self.roof_key {
            return;
        }
        Self::free(r, &self.roof);
        let built = roof(t);
        self.roof = vec![
            Part::clay(up(r, &built.deck), MATTE, Place::World),
            Part::clay(up(r, &built.solid), MATTE, Place::World),
            Part::new(up(r, &built.lit), Look::Flat, Place::World),
            Part { falls_back: true, tint: super::geom::hex(o::GLASS).extend(0.13).to_array(), ..Part::new(up(r, &built.clear), Look::Glass, Place::World) }.ordered(2),
            Part::new(up(r, &built.thrown), Look::Light, Place::World).ordered(1),
        ];
        if let Some((m, ..)) = self.car.take() {
            r.free(m);
        }
        self.car = Some((up(r, &built.car), built.car_at, built.car_stops));
        let plate = 0.78 * (0.94 * 24.0 / 11.2);
        if self.logo.is_none() {
            self.logo = Some((up(r, &super::prims::plane(plate, plate).tint("#ffffff", None)), built.logo_at, built.logo_turn));
        } else if let Some(l) = self.logo.as_mut() {
            l.1 = built.logo_at;
            l.2 = built.logo_turn;
        }
        self.roof_key = key;
    }

    fn badge_of(&mut self, r: &mut Renderer, cli: &str) -> (TexId, Vec3) {
        if let Some(b) = self.badges.get(cli) {
            return *b;
        }
        let b = super::badges::badge_texture(r, cli);
        self.badges.insert(cli.to_string(), b);
        b
    }

    fn body(&mut self, r: &mut Renderer, p: &PersonIn, seat: Seat, y: f64, clay: Clay, marked: bool) -> Body {
        let key = format!("{}:{}:{}:{}:{}", p.id, p.shirt, p.hair, p.rank.as_str(), p.prop);
        let ids = match self.shapes.get(&key) {
            Some(ids) => *ids,
            None => {
                let s = shapes_of(&p.looks());
                let ids = [up(r, &s.torso), up(r, &s.legs), up(r, &s.arm_l), up(r, &s.arm_r)];
                self.shapes.insert(key, ids);
                ids
            }
        };
        let (badge, badge_ink) = self.badge_of(r, &p.cli);
        Body { id: p.id.clone(), seat, y, state: p.state, seat_no: p.seat as f64, marked, clay, torso: ids[0], legs: ids[1], arm_l: ids[2], arm_r: ids[3], badge, badge_ink }
    }

    fn build_floor(&mut self, r: &mut Renderer, f: &FloorIn, spot: &FloorSpot, key: String) {
        let plan = f.plan();
        let room_key = format!("{:?}", (&f.id, spot.y, f.lit, f.board.is_some(), spot.desks.iter().map(|d| (d.seat.x, d.seat.z, d.seat.rot)).collect::<Vec<_>>()));
        let old = self.floors.remove(&f.id);
        let room = match old {
            Some(mut b) if b.room_key == room_key => {
                let keep = b.parts.len().min(6);
                Self::free(r, &b.parts[keep..]);
                b.parts.truncate(keep);
                (b.room.take().unwrap(), b.parts)
            }
            other => {
                if let Some(b) = other {
                    Self::free(r, &b.parts);
                }
                let room = room_for(&plan, spot);
                let at = Place::Up(spot.y as f32);
                let in_room = if f.lit { INDOOR } else { NIGHT };
                let mut parts = vec![
                    Part::clay(up(r, &room.shell), MATTE, at),
                    Part::clay(up(r, &room.things), in_room, at),
                    Part { falls_back: true, ..Part::new(up(r, &room.wall), Look::Flat, at) },
                    Part::new(up(r, &room.lit), Look::Flat, at),
                    Part::new(up(r, &room.thrown), Look::Light, at).ordered(1),
                ];
                parts.push(match &room.haze {
                    Some(h) => Part::new(up(r, h), Look::LightOver, at).ordered(2),
                    None => Part { on: false, ..Part::new(r.mesh(&[]), Look::LightOver, at) },
                });
                (room, parts)
            }
        };
        let (room, mut parts) = room;
        let built = floor_for(&plan, spot, &room);
        let at = Place::Up(spot.y as f32);
        let in_room = if f.lit { INDOOR } else { NIGHT };
        parts.push(Part::clay(up(r, &built.solid), in_room, at));
        parts.push(Part::new(up(r, &built.lit), Look::Flat, at));
        parts.push(Part::new(up(r, &built.thrown), Look::Light, at).ordered(1));
        self.floors.insert(f.id.clone(), Built { key, room_key, room: Some(room), parts });
    }

    /// Brings the card up to the model.
    pub fn set(&mut self, r: &mut Renderer, m: &OfficeIn, t: &Tower) {
        let marked = m.marked();
        let gone: Vec<String> = self.floors.keys().filter(|id| !m.floors.iter().any(|f| &f.id == *id)).cloned().collect();
        for id in gone {
            if let Some(b) = self.floors.remove(&id) {
                Self::free(r, &b.parts);
            }
        }
        for (f, spot) in m.floors.iter().zip(&t.floors) {
            let looks: Vec<_> = f.people.iter().map(|p| (&p.id, p.state, p.seat, p.rank.as_str(), &p.shirt, &p.hair, &p.prop, &p.cli, marked.contains(&p.id))).collect();
            let key = format!("{:?}", (spot.y, f.lit, f.board, f.papers, &f.lead, looks, spot.desks.iter().map(|d| (&d.person_id, d.seat.x, d.seat.z, d.seat.rot)).collect::<Vec<_>>()));
            if self.floors.get(&f.id).is_none_or(|b| b.key != key) {
                self.build_floor(r, f, spot, key);
            }
        }
        self.order = m.floors.iter().map(|f| f.id.clone()).collect();
        self.build_roof(r, t);
        Self::free(r, &self.top);
        let founder = m.founder.as_ref().map(PersonIn::seated);
        let top = top_for(founder.as_ref(), t);
        self.top = vec![Part::clay(up(r, &top.solid), MATTE, Place::World), Part::new(up(r, &top.lit), Look::Flat, Place::World), Part::new(up(r, &top.thrown), Look::Light, Place::World).ordered(1)];
        self.bodies.clear();
        for (f, spot) in m.floors.iter().zip(&t.floors) {
            let clay = if f.lit { INDOOR } else { NIGHT };
            for d in &spot.desks {
                if let Some(p) = f.people.iter().find(|p| p.id == d.person_id) {
                    let b = self.body(r, p, d.seat, spot.y, clay, marked.contains(&p.id));
                    self.bodies.push(b);
                }
            }
        }
        if let (Some(p), Some(seat)) = (&m.founder, top.founder) {
            let b = self.body(r, p, seat, t.roof_y, INDOOR, marked.contains(&p.id));
            self.bodies.push(b);
        }
        self.setting[self.spill].on = m.floors.iter().any(|f| f.lit);
        let mut busy: Vec<Busy> = m
            .floors
            .iter()
            .zip(&t.floors)
            .flat_map(|(f, spot)| spot.desks.iter().filter_map(move |d| f.people.iter().find(|p| p.id == d.person_id && p.active()).map(|p| Busy { seat: d.seat, y: spot.y, printing: p.printing })))
            .collect();
        if let Some(p) = m.founder.as_ref().filter(|p| p.active()) {
            busy.push(Busy { seat: FOUNDER_DESK, y: t.roof_y, printing: p.printing });
        }
        self.rising = rising_all(&busy);
    }

    /// Every floor's parts, bottom up.
    pub fn floor_parts(&self) -> impl Iterator<Item = &Part> {
        self.order.iter().filter_map(|id| self.floors.get(id)).flat_map(|b| b.parts.iter())
    }
}
