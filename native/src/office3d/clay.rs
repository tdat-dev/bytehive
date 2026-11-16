//! The office's light and its clay looks, the values of scene.ts and build.ts:
//! a HemisphereLight(0xb9c8f0, 0x4a3a34, 1.1), a DirectionalLight(0xffe2c0,
//! 1.25), ACES at exposure 1.05, and the three clays (matte outside, indoor,
//! night). Colours go linear here, as three keeps them.

use glam::{Mat4, Vec3};

use super::geom::hex;
use super::gpu::Frame;

/// layout.ts: where the first floor stands, a storey's height, a slab's.
pub const GROUND: f32 = 0.5;
pub const STOREY: f32 = 4.0;
pub const SLAB: f32 = 0.56;

/// ClayLook in clay.ts.
#[derive(Clone, Copy)]
pub struct Clay {
    pub self_: &'static str,
    pub self_level: f32,
    pub rim: &'static str,
    pub rim_level: f32,
    pub foot: f32,
}

pub const MATTE: Clay = Clay { self_: "#8fa0d8", self_level: 0.2, rim: "#7f92d8", rim_level: 0.08, foot: 0.0 };
pub const INDOOR: Clay = Clay { self_: "#ffcf9a", self_level: 0.34, rim: "#9fb4e6", rim_level: 0.2, foot: 0.3 };
pub const NIGHT: Clay = Clay { self_: "#6f7fc0", self_level: 0.24, rim: "#7d90d8", rim_level: 0.12, foot: 0.26 };

const EXPOSURE: f32 = 1.05;

fn lit(c: &str, level: f32) -> [f32; 4] {
    let v = hex(c) * level;
    [v.x, v.y, v.z, 0.0]
}

/// Everything a frame's draw needs: the camera, the lights, one clay.
pub fn frame(view_proj: Mat4, eye: Vec3, sun_dir: Vec3, light: Mat4, clay: Clay) -> Frame {
    let sun = sun_dir.normalize();
    Frame {
        view_proj: view_proj.to_cols_array_2d(),
        model: Mat4::IDENTITY.to_cols_array_2d(),
        sun: [sun.x, sun.y, sun.z, 0.0],
        sun_color: lit("#ffe2c0", 1.25),
        sky: lit("#b9c8f0", 1.1),
        ground: lit("#4a3a34", 1.1),
        eye: [eye.x, eye.y, eye.z, EXPOSURE],
        room: lit(clay.self_, clay.self_level),
        rim: lit(clay.rim, clay.rim_level),
        storey: [clay.foot, GROUND, STOREY, STOREY - SLAB],
        // scene.ts: bias -0.0006, normalBias 0.03, intensity 0.62.
        light: light.to_cols_array_2d(),
        shadow: [-0.0006, 0.03, 0.62, 1.0],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_office_shader_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("office.wgsl")).unwrap_or_else(|e| panic!("{}", e.emit_to_string(include_str!("office.wgsl"))));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&module).expect("valid WGSL");
    }

    #[test]
    fn the_frame_is_laid_out_as_the_shader_reads_it() {
        // Two matrices, eight vec4s, the light's matrix, one vec4: no padding.
        assert_eq!(std::mem::size_of::<super::Frame>(), 2 * 64 + 8 * 16 + 64 + 16);
    }
}
