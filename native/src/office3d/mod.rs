//! The Company's office in 3D, drawn by wgpu off GPUI's own renderer (see
//! gpu.rs for why). P0 is the spike; the plan is
//! docs/superpowers/plans/2026-11-11-native-3d-office.md.

// The port lands piece by piece, each tested against three.js, before the
// scene that uses it (P3): until then some pieces have no caller.
#![allow(dead_code)]

mod earcut;
pub mod geom;
pub mod gpu;
pub mod lathe;
pub mod mesh;
pub mod prims;
#[cfg(test)]
mod prims_tests;
mod readback;
pub mod slab;
pub mod spike;
