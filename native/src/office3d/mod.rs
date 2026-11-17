//! The Company's office in 3D, drawn by wgpu off GPUI's own renderer (see
//! gpu.rs for why). P0 is the spike; the plan is
//! docs/superpowers/plans/2026-11-11-native-3d-office.md.

// The port lands piece by piece, each tested against three.js, before the
// scene that uses it (P3): until then some pieces have no caller.
#![allow(dead_code)]

pub mod badges;
pub mod build_parts;
#[cfg(test)]
mod build_tests;
mod earcut;
pub mod clay;
pub mod colour;
pub mod demo;
pub mod desk;
pub mod draws;
pub mod floor;
#[cfg(test)]
mod floor_tests;
pub mod furnish;
#[cfg(test)]
mod furnish_tests;
pub mod geom;
pub mod glow;
pub mod gpu;
pub mod lathe;
pub mod layout;
pub mod lens;
#[cfg(test)]
mod lens_tests;
pub mod life;
pub mod light;
#[cfg(test)]
mod light_tests;
#[cfg(test)]
mod layout_tests;
pub mod nav;
pub mod palette;
pub mod people_desk;
pub mod people_pose;
pub mod people_shapes;
#[cfg(test)]
mod people_tests;
pub mod pick;
pub mod mesh;
pub mod prims;
pub mod props;
pub mod props_wall;
pub mod quality;
pub mod scene;
pub mod seed;
#[cfg(test)]
mod prims_tests;
mod readback;
pub mod shadow;
pub mod render;
pub mod render_pipes;
pub mod render_tex;
pub mod render_types;
pub mod roof;
#[cfg(test)]
mod roof_tests;
pub mod setting;
#[cfg(test)]
mod setting_tests;
pub mod sky;
pub mod slab;
pub mod stage;
pub mod surfaces;
pub mod town;
pub mod view;
pub mod world;
pub mod world_in;
pub mod spike;
