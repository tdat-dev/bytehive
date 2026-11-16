//! The sun's shadow, as three.js r170 casts it (PCFSoftShadowMap): a 1024
//! depth map seen from the sun through an orthographic box fitted to the tower
//! (fitSun in scene.ts), back faces drawn into it as three does for one-sided
//! materials, then read in office.wgsl with a 3 x 3 compare whose outer taps
//! are blended by where the point falls between texels.

use glam::{Mat4, Vec3};

pub const MAP: u32 = 1024;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// scene.ts SUN: low, from the camera's left.
pub fn sun_dir() -> Vec3 {
    Vec3::new(0.25, 0.45, 1.0).normalize()
}

/// fitSun: the light's view-projection for a tower `roof_y` high whose floors
/// centre on x = `x`.
pub fn fit_sun(x: f32, roof_y: f32) -> Mat4 {
    let sun = sun_dir();
    let mid = roof_y / 2.0;
    let reach = (roof_y / 2.0 + 6.0).max(11.0);
    let far = reach * 3.0;
    let target = Vec3::new(x, mid, 0.0);
    let from = Vec3::new(x + sun.x * far, mid + sun.y * far, sun.z * far);
    Mat4::orthographic_rh(-reach, reach, -reach, reach, 1.0, far * 2.0) * Mat4::look_at_rh(from, target, Vec3::Y)
}

pub struct Shadow {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub pipeline: wgpu::RenderPipeline,
    pub bind: wgpu::BindGroup,
}

impl Shadow {
    /// The map, its compare sampler, and the depth-only pipeline that fills it
    /// (vs_shadow in office.wgsl, reading only the frame's uniforms).
    pub fn new(device: &wgpu::Device, shader: &wgpu::ShaderModule, uniforms: &wgpu::Buffer, vertex: wgpu::VertexBufferLayout) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sun shadow"),
            size: wgpu::Extent3d { width: MAP, height: MAP, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        // texture2DCompare: lit where the point is no deeper than the map.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("sun shadow"), compare: Some(wgpu::CompareFunction::LessEqual), ..Default::default() });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow frame"),
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("shadow frame"), layout: &layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }] });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("shadow"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: shader, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &[Some(vertex)] },
            // three draws the back faces of one-sided materials into the map.
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Front), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        Self { view, sampler, pipeline, bind }
    }

    /// The map's pass: depth from the sun, nothing else.
    pub fn pass(&self, encoder: &mut wgpu::CommandEncoder, vertices: &wgpu::Buffer, count: u32) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sun shadow"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.draw(0..count, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_box_holds_the_whole_tower() {
        let light = fit_sun(0.0, 16.0);
        for p in [Vec3::new(-6.0, 0.0, -5.0), Vec3::new(6.0, 16.0, 5.0), Vec3::new(0.0, 8.0, 0.0)] {
            let c = light.project_point3(p);
            assert!(c.x.abs() <= 1.0 && c.y.abs() <= 1.0 && (0.0..=1.0).contains(&c.z), "{p} -> {c}");
        }
    }
}
