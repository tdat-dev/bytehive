//! The renderer's pipelines, one for each look, and the sun's depth-only one:
//! how each three.js material blends, culls and uses depth.

use super::gpu::Vertex;
use super::render_types::Look;

pub const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// three's `antialias: true`.
pub const SAMPLES: u32 = 4;

pub struct Layouts {
    pub frame: wgpu::BindGroupLayout,
    pub frame_shadow: wgpu::BindGroupLayout,
    pub draw: wgpu::BindGroupLayout,
    pub picture: wgpu::BindGroupLayout,
}

fn uniform(binding: u32, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: None }, count: None }
}

pub fn layouts(device: &wgpu::Device) -> Layouts {
    let make = |label, entries: &[wgpu::BindGroupLayoutEntry]| device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries });
    let fragment = wgpu::ShaderStages::FRAGMENT;
    Layouts {
        frame: make("frame", &[
            uniform(0, false),
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: fragment, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            wgpu::BindGroupLayoutEntry { binding: 2, visibility: fragment, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
        ]),
        frame_shadow: make("frame for the sun", &[uniform(0, false)]),
        draw: make("draw", &[uniform(0, true)]),
        picture: make("picture", &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: fragment, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: fragment, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
        ]),
    }
}

fn vertex_layout(attributes: &[wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'_> {
    wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes }
}

const ADD: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
    alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
};

/// (vertex entry, fragment entry, blend, depth write, depth test, cull).
fn state(look: Look) -> (&'static str, &'static str, Option<wgpu::BlendState>, bool, wgpu::CompareFunction, Option<wgpu::Face>) {
    use wgpu::CompareFunction::{Always, LessEqual};
    let over = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
    let back = Some(wgpu::Face::Back);
    match look {
        Look::Lit => ("vs", "fs_lit", None, true, LessEqual, back),
        Look::Glass => ("vs", "fs_lit", over, false, LessEqual, back),
        Look::Flat => ("vs", "fs_flat", None, true, LessEqual, back),
        Look::FlatBoth => ("vs", "fs_flat", None, true, LessEqual, None),
        Look::Backdrop => ("vs", "fs_picture", None, false, LessEqual, None),
        Look::Picture => ("vs", "fs_picture", over, true, LessEqual, None),
        Look::Light => ("vs", "fs_light", Some(ADD), false, LessEqual, None),
        Look::LightOver => ("vs", "fs_light", Some(ADD), false, Always, None),
        Look::Over => ("vs", "fs_flat", over, false, Always, None),
        Look::Star => ("vs_star", "fs_sprite", Some(ADD), false, LessEqual, None),
        Look::Mark => ("vs_mark", "fs_mark", Some(ADD), false, LessEqual, None),
    }
}

pub fn pipeline(device: &wgpu::Device, shader: &wgpu::ShaderModule, layout: &wgpu::PipelineLayout, look: Look) -> wgpu::RenderPipeline {
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
    let (vs, fs, blend, write, compare, cull) = state(look);
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("office look"),
        layout: Some(layout),
        vertex: wgpu::VertexState { module: shader, entry_point: Some(vs), compilation_options: Default::default(), buffers: &[Some(vertex_layout(&attributes))] },
        primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(write), depth_compare: Some(compare), stencil: Default::default(), bias: Default::default() }),
        multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
        fragment: Some(wgpu::FragmentState { module: shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: COLOR, blend, write_mask: wgpu::ColorWrites::ALL })] }),
        multiview_mask: None,
        cache: None,
    })
}

/// The sun's map: depth only, back faces, as three draws one-sided materials into it.
pub fn shadow_pipeline(device: &wgpu::Device, shader: &wgpu::ShaderModule, layout: &wgpu::PipelineLayout) -> wgpu::RenderPipeline {
    let wide = [wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 }];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sun shadow"),
        layout: Some(layout),
        vertex: wgpu::VertexState { module: shader, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &[Some(vertex_layout(&wide))] },
        primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Front), ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default() }),
        multisample: Default::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}
