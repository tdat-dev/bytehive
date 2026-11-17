//! The 3D office (scene.ts createScene): it draws the model and holds nothing
//! else that matters; drop it and set the model again and the picture is the
//! same. The camera eases between levels, people move with the clock, and a
//! frame is drawn only when the caller asks (see `next_frame`).

use std::collections::HashMap;

use glam::{DVec3, Vec2};

use super::badges::logo_texture;
use super::build_parts::SLAB_X;
use super::draws::{Moment, draws, rising_quads};
use super::geom::hex;
use super::layout::{FLIGHT_MS, Level, Shot, Tower, mix_shots, shot_for};
use super::lens::{Lens, MOVE_MS, ease_level, haze_opacity, lens, mark_scale, same_shot};
use super::nav::frame_delay;
use super::palette::office as o;
use super::people_pose::{HOP_MS, State};
use super::pick::{Proxy, pick};
use super::quality::{Look as Seen, Quality, look_of, start_tier, stepped_down};
use super::render::{Renderer, light_for};
use super::render_types::{FrameIn, MeshId};
use super::stage::{Anchor, Cast, What, anchors, band_for, pick_from, proxies};
use super::world::World;
use super::world_in::OfficeIn;

/// How far sky and town shift at the edge of the canvas, in scene units.
const PARALLAX: (f64, f64) = (1.6, 0.7);

struct Tween {
    from: Shot,
    to: Shot,
    at: Option<f64>,
    band: (DVec3, DVec3),
}

pub struct Scene {
    r: Renderer,
    world: World,
    model: OfficeIn,
    tower: Tower,
    cast: Cast,
    level: Level,
    shot: Option<Shot>,
    tween: Option<Tween>,
    band: DVec3,
    /// The canvas in CSS pixels, and device pixels per CSS pixel.
    css: (f64, f64),
    dpr: f64,
    clear_px: f64,
    hovered: Option<String>,
    hops: HashMap<String, f64>,
    shown: HashMap<String, State>,
    rising: (MeshId, MeshId),
    boxes: Vec<Proxy<What>>,
    pub reduced: bool,
    quality: Quality,
    tier: usize,
    gaps: Vec<f64>,
    last_frame: Option<f64>,
    reshadow: bool,
    parallax: (f32, f32),
    /// The camera came to rest this frame: the words over the picture can be placed.
    pub rested: bool,
    pub frames: u64,
}

impl Scene {
    pub fn new(quality: Quality, battery: bool) -> Result<Self, String> {
        let mut r = Renderer::new()?;
        let logo = logo_texture(&mut r);
        let world = World::new(&mut r, logo);
        let rising = (r.mesh(&[]), r.mesh(&[]));
        let tier = start_tier(quality, battery);
        let model = OfficeIn::default();
        let tower = model.tower();
        Ok(Self {
            r,
            world,
            model,
            tower,
            cast: Cast::default(),
            level: Level::Building,
            shot: None,
            tween: None,
            band: DVec3::new(-1000.0, 1000.0, 0.0),
            css: (1.0, 1.0),
            dpr: 1.0,
            clear_px: 0.0,
            hovered: None,
            hops: HashMap::new(),
            shown: HashMap::new(),
            rising,
            boxes: vec![],
            reduced: false,
            quality,
            tier,
            gaps: vec![],
            last_frame: None,
            reshadow: true,
            parallax: (0.0, 0.0),
            rested: false,
            frames: 0,
        })
    }

    pub fn adapter(&self) -> &str {
        &self.r.adapter
    }

    fn seen(&self) -> Seen {
        look_of(self.tier)
    }

    fn target(&self) -> Shot {
        shot_for(&self.level, &self.tower, self.css.0 / self.css.1, (self.clear_px / self.css.0).min(0.6))
    }

    /// Point the camera where the level says, at once or eased.
    fn aim(&mut self, instant: bool) {
        let to = self.target();
        let band = band_for(&self.level, &self.tower);
        match self.shot {
            Some(now) if !instant && !self.reduced && !same_shot(&to, &now) => self.tween = Some(Tween { from: now, to, at: None, band: (self.band, band) }),
            _ => {
                self.tween = None;
                self.shot = Some(to);
                self.band = band;
                self.rested = true;
            }
        }
    }

    /// The office to show. Someone who was working and has stopped with nothing to ask has finished: a small jump.
    pub fn set_model(&mut self, m: OfficeIn, now: f64) -> bool {
        if m == self.model && self.shot.is_some() {
            return false;
        }
        let tower = m.tower();
        self.world.set(&mut self.r, &m, &tower);
        for p in m.everyone() {
            if self.shown.get(&p.id) == Some(&State::Working) && matches!(p.state, State::Review | State::Idle) && !self.reduced {
                self.hops.insert(p.id.clone(), now);
            }
        }
        self.shown = m.everyone().map(|p| (p.id.clone(), p.state)).collect();
        self.cast = m.cast();
        self.boxes = proxies(&tower, &self.cast);
        self.tower = tower;
        self.model = m;
        self.reshadow = true;
        let to = self.target();
        let first = self.shot.is_none();
        if first || !same_shot(&to, &self.tween.as_ref().map_or(self.shot.unwrap(), |t| t.to)) {
            self.aim(first);
        }
        true
    }

    pub fn set_level(&mut self, l: Level, instant: bool) {
        self.level = l;
        self.aim(instant);
    }

    /// The camera is between levels: the words over the picture wait.
    pub fn moving(&self) -> bool {
        self.tween.is_some()
    }

    pub fn hovered(&self) -> Option<&str> {
        self.hovered.as_deref()
    }

    pub fn level(&self) -> &Level {
        &self.level
    }

    /// The canvas, in CSS pixels, and its device pixel ratio.
    pub fn resize(&mut self, w: f64, h: f64, dpr: f64) {
        if w <= 0.0 || h <= 0.0 || ((w, h) == self.css && dpr == self.dpr) {
            return;
        }
        self.css = (w, h);
        self.dpr = dpr;
        match (&mut self.tween, self.shot) {
            (Some(t), Some(now)) => {
                t.from = now;
                t.at = None;
                t.to = shot_for(&self.level, &self.tower, w / h, (self.clear_px / w).min(0.6));
            }
            // No model yet, no tower to aim at: set_model aims.
            (_, None) => {}
            _ => {
                self.shot = Some(self.target());
                self.rested = true;
            }
        }
    }

    /// Sets aside this many CSS pixels on the left at the Building, for what stands over the picture.
    pub fn set_clear(&mut self, px: f64) {
        if (px - self.clear_px).abs() < 1.0 {
            return;
        }
        self.clear_px = px;
        if self.shot.is_some() {
            self.aim(true);
        }
    }

    pub fn set_quality(&mut self, q: Quality, battery: bool) {
        self.quality = q;
        self.tier = start_tier(q, battery);
        self.reshadow = true;
    }

    /// The person under the pointer rises a little.
    pub fn hover(&mut self, id: Option<&str>) -> bool {
        let changed = self.hovered.as_deref() != id;
        self.hovered = id.map(Into::into);
        changed
    }

    /// Where the pointer is on the canvas, -1 to 1: sky and town shift behind the tower.
    pub fn set_parallax(&mut self, x: f64, y: f64) -> bool {
        if self.reduced {
            return false;
        }
        let on = if self.level == Level::Building { 1.0 } else { 0.0 };
        let next = ((-x * PARALLAX.0 * on) as f32, (y * PARALLAX.1 * on) as f32);
        let moved = (next.0 - self.parallax.0).abs() >= 0.01 || (next.1 - self.parallax.1).abs() >= 0.01;
        if moved {
            self.parallax = next;
        }
        moved
    }

    fn lens(&self) -> Option<Lens> {
        self.shot.map(|s| lens(&s, self.css.0, self.css.1))
    }

    /// What is under a point on the canvas (CSS pixels).
    pub fn pick(&self, x: f64, y: f64) -> Option<What> {
        let l = self.lens()?;
        let ndc = Vec2::new((x / self.css.0 * 2.0 - 1.0) as f32, (1.0 - y / self.css.1 * 2.0) as f32);
        let vp = (l.projection_gl() * l.view).as_mat4();
        let hits: Vec<(What, f64)> = self.boxes.iter().filter_map(|b| pick(std::slice::from_ref(b), vp, l.eye.as_vec3(), ndc).map(|(p, d)| (p.what.clone(), d as f64))).collect();
        pick_from(&hits)
    }

    pub fn anchors(&self) -> Vec<Anchor> {
        self.lens().map(|l| anchors(&self.tower, &self.cast, &l, self.css.0, self.css.1)).unwrap_or_default()
    }

    fn ambient(&self) -> bool {
        self.model.everyone().any(|p| p.active()) && self.seen().ambient && !matches!(self.level, Level::Desk { .. })
    }

    fn flying(&self, now: f64) -> bool {
        self.model.flights.iter().any(|f| now - f.at >= 0.0 && now - f.at < FLIGHT_MS)
    }

    /// How long until the next frame is wanted (ms), or None to rest.
    pub fn next_frame(&self, now: f64) -> Option<f64> {
        if self.tween.is_some() {
            return Some(0.0);
        }
        let hopping = self.hops.values().any(|&s| now - s < HOP_MS);
        let moving = hopping || (!self.reduced && self.flying(now));
        frame_delay(true, moving, self.ambient(), self.reduced).map(|ms| ms as f64)
    }

    /// Draw a frame at `now` (the model's clock, ms) and `t` (a steady clock for
    /// the camera, ms). Gives back the newest finished picture, BGRA.
    pub fn frame(&mut self, now: f64, t: f64) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
        self.rested = false;
        self.hops.retain(|_, s| now - *s < HOP_MS);
        if let Some(tw) = &mut self.tween {
            if let Some(last) = self.last_frame {
                self.gaps.push(t - last);
            }
            self.last_frame = Some(t);
            let start = *tw.at.get_or_insert(t);
            let k = ((t - start) / MOVE_MS).min(1.0);
            let e = ease_level(k);
            self.shot = Some(mix_shots(tw.from, tw.to, e));
            self.band = tw.band.0.lerp(tw.band.1, e);
            if k >= 1.0 {
                self.tween = None;
                self.rested = true;
                self.last_frame = None;
                let next = stepped_down(self.tier, self.quality, &self.gaps);
                self.gaps.clear();
                if next != self.tier {
                    self.tier = next;
                    self.reshadow = true;
                }
            }
        }
        let Some(shot) = self.shot else { return Ok(None) };
        let ratio = self.dpr.min(self.seen().max_dpr as f64);
        let (w, h) = ((self.css.0 * ratio).round() as u32, (self.css.1 * ratio).round() as u32);
        let l = lens(&shot, self.css.0, self.css.1);
        let still = self.reduced || !self.ambient();
        if !still {
            let (steam, bytes) = rising_quads(&self.world.rising, (now % 3_600_000.0) / 1000.0);
            self.r.remesh(self.rising.0, &steam);
            self.r.remesh(self.rising.1, &bytes);
        }
        let moment = Moment {
            now,
            still,
            hovered: self.hovered.as_deref(),
            hops: &self.hops,
            lens: &l,
            mark: mark_scale(&shot, self.css.1) as f32,
            parallax: self.parallax,
            haze: haze_opacity(&shot) as f32,
            glow: self.seen().glow,
            reduced: self.reduced,
            rising: self.rising,
        };
        let list = draws(&self.world, &self.model, &self.tower, &moment);
        let seen = self.seen();
        let frame = FrameIn {
            view: l.view.as_mat4(),
            proj: l.projection_gpu().as_mat4(),
            eye: l.eye.as_vec3(),
            light: light_for(SLAB_X, self.tower.roof_y as f32),
            shadows: seen.shadows,
            reshadow: self.reshadow && seen.shadows,
            band: self.band.as_vec3(),
            star_half: (0.8 * ratio) as f32,
            clear: hex(o::SKY_TOP),
        };
        let before = self.r.drawn;
        let out = self.r.draw(w, h, &frame, &list)?;
        if self.r.drawn > before {
            self.reshadow = false;
            self.frames += 1;
        }
        Ok(out)
    }

    /// A finished picture, if one came back, and whether another is on its way.
    pub fn collect(&mut self) -> (Option<(u32, u32, Vec<u8>)>, bool) {
        self.r.collect()
    }

    /// Draws until a picture comes back, waiting on the card: for shots, never on the UI thread.
    pub fn shoot(&mut self, now: f64) -> Result<(u32, u32, Vec<u8>), String> {
        for _ in 0..6 {
            if let Some(px) = self.frame(now, now)? {
                return Ok(px);
            }
            self.r.wait();
        }
        Err("no picture came back".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws the demo office to BYTEHIVE_OFFICE_SHOT (a .png), to look at by eye.
    /// BYTEHIVE_OFFICE_LEVEL picks a floor id, or "founder" for the roof desk.
    #[test]
    #[ignore]
    fn shoot_the_office() {
        let Ok(path) = std::env::var("BYTEHIVE_OFFICE_SHOT") else { return };
        let mut s = Scene::new(Quality::High, false).expect("a graphics card");
        let now = 1_000_000.0;
        s.resize(1280.0, 800.0, 1.0);
        s.set_model(super::super::demo::demo_office(now), now);
        match std::env::var("BYTEHIVE_OFFICE_LEVEL").ok().as_deref() {
            Some("founder") => s.set_level(Level::Desk { person: "founder".into(), floor: None }, true),
            Some(id) => s.set_level(Level::Floor(id.into()), true),
            None => {}
        }
        let (w, h, bgra) = s.shoot(now).unwrap();
        let rgb: Vec<u8> = bgra.chunks(4).flat_map(|p| [p[2], p[1], p[0]]).collect();
        image::save_buffer(path, &rgb, w, h, image::ColorType::Rgb8).unwrap();
        eprintln!("{} on {}", s.frames, s.adapter());
    }
}
