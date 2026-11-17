//! What the renderer takes (scene.wgsl): the frame's uniforms, each draw's,
//! and the looks, one for each kind of three.js material scene.ts uses.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// The frame: camera, lights, shadow, focus band, viewport.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct FrameU {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub light: [[f32; 4]; 4],
    pub sun: [f32; 4],
    pub sun_color: [f32; 4],
    pub sky: [f32; 4],
    pub ground: [f32; 4],
    pub eye: [f32; 4],
    pub shadow: [f32; 4],
    pub band: [f32; 4],
    pub viewport: [f32; 4],
}

/// One draw's place and look.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DrawU {
    pub model: [[f32; 4]; 4],
    pub room: [f32; 4],
    pub rim: [f32; 4],
    pub tint: [f32; 4],
    pub flags: [f32; 4],
}

/// Draw uniforms sit this far apart, for dynamic offsets.
pub const DRAW_STRIDE: u64 = 256;

/// How a draw is drawn: a three.js material, as scene.ts and build.ts set them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Look {
    /// The clay: lit, opaque, takes and casts shadows.
    Lit,
    /// The glass: lit, see-through, writes no depth.
    Glass,
    /// Holds its own light (MeshBasicMaterial, no tone mapping).
    Flat,
    /// The same, seen from both sides (the paper plane).
    FlatBoth,
    /// A picture behind everything: the sky.
    Backdrop,
    /// A picture with see-through parts: the sign's logo, a badge.
    Picture,
    /// Light thrown: a soft pool added on.
    Light,
    /// The same over everything: a lit room's haze.
    LightOver,
    /// Drawn over everything: the mark over someone who needs you.
    Over,
    /// Stars: squares a few pixels across, added.
    Star,
    /// The rising marks: quads facing the camera, added.
    Mark,
}

pub const LOOKS: [Look; 11] = [Look::Lit, Look::Glass, Look::Flat, Look::FlatBoth, Look::Backdrop, Look::Picture, Look::Light, Look::LightOver, Look::Over, Look::Star, Look::Mark];

/// A geometry on the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshId(pub usize);

/// A picture on the card. 0 is plain white, and the pool of light is 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexId(pub usize);

pub const WHITE: TexId = TexId(0);
pub const POOL: TexId = TexId(1);

/// One thing to draw, in the order given.
#[derive(Clone, Copy, Debug)]
pub struct Draw {
    pub mesh: MeshId,
    pub look: Look,
    pub model: Mat4,
    /// Clay: the room's glow and the foot; the edge light and the roughness.
    pub room: [f32; 4],
    pub rim: [f32; 4],
    /// Multiplies the colour (rgb) and the opacity.
    pub tint: [f32; 4],
    /// The band dims it; tint replaces the vertex colour; a round mark.
    pub falls_back: bool,
    pub solid_tint: bool,
    pub round: bool,
    pub texture: TexId,
    /// It goes into the sun's shadow map.
    pub cast: bool,
}

impl Draw {
    /// A draw with nothing special: white tint, no clay, no band.
    pub fn new(mesh: MeshId, look: Look) -> Self {
        Self { mesh, look, model: Mat4::IDENTITY, room: [0.0; 4], rim: [0.0, 0.0, 0.0, 1.0], tint: [1.0; 4], falls_back: false, solid_tint: false, round: false, texture: WHITE, cast: false }
    }

    pub fn uniforms(&self) -> DrawU {
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        DrawU { model: self.model.to_cols_array_2d(), room: self.room, rim: self.rim, tint: self.tint, flags: [flag(self.falls_back), flag(self.solid_tint), flag(self.round), 0.0] }
    }
}

/// Everything the frame needs from the scene.
#[derive(Clone, Copy, Debug)]
pub struct FrameIn {
    pub view: Mat4,
    pub proj: Mat4,
    pub eye: Vec3,
    pub light: Mat4,
    pub shadows: bool,
    /// Draw the sun's map again: three only does when something moved.
    pub reshadow: bool,
    pub band: Vec3,
    /// A star's half size, in device pixels.
    pub star_half: f32,
    /// The background, linear.
    pub clear: Vec3,
}
