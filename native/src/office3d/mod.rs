//! The Company's office in 3D, drawn by wgpu off GPUI's own renderer (see
//! gpu.rs for why). P0 is the spike; the plan is
//! docs/superpowers/plans/2026-11-11-native-3d-office.md.

// The port lands piece by piece, each tested against three.js, before the
// scene that uses it (P3): until then some pieces have no caller.
#![allow(dead_code)]

mod earcut;
pub mod clay;
pub mod geom;
pub mod glow;
pub mod gpu;
pub mod lathe;
pub mod layout;
pub mod light;
#[cfg(test)]
mod light_tests;
#[cfg(test)]
mod layout_tests;
pub mod nav;
pub mod palette;
pub mod pick;
pub mod mesh;
pub mod prims;
pub mod quality;
pub mod seed;
#[cfg(test)]
mod prims_tests;
mod readback;
pub mod shadow;
pub mod slab;
pub mod spike;
