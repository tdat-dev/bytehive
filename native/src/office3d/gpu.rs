//! The office's own GPU, apart from GPUI's: wgpu on DX12 draws into an offscreen
//! BGRA texture and the frame is read back, because GPUI on Windows has no way
//! to show a GPU surface (its DirectComposition target is topmost, and
//! paint_surface is macOS only). GPUI then shows the bytes as an image.

use std::sync::mpsc;

use bytemuck::{Pod, Zeroable};

/// A vertex: where, which way it faces, its colour.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// What every vertex needs this frame.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Frame {
    pub view_proj: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    /// The sun's direction (towards the light), w unused.
    pub sun: [f32; 4],
}

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    count: u32,
    target: Option<Target>,
    pub adapter: String,
}

/// The textures and the readback buffer for one size.
struct Target {
    width: u32,
    height: u32,
    color: wgpu::Texture,
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
    /// Bytes per row in the readback buffer: a multiple of 256, as wgpu wants.
    row: u32,
}

/// The card GPUI draws on (the first adapter, on a laptop the one wired to the
/// screen): waking the other one costs a second driver in memory and battery.
/// BYTEHIVE_OFFICE3D_GPU=high asks for the fast one, to compare.
fn power() -> wgpu::PowerPreference {
    if std::env::var("BYTEHIVE_OFFICE3D_GPU").is_ok_and(|v| v == "high") { wgpu::PowerPreference::HighPerformance } else { wgpu::PowerPreference::LowPower }
}

impl Gpu {
    /// A device on a real graphics card, or why there is none.
    pub fn new(mesh: &[Vertex]) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: power(), ..Default::default() }))
            .map_err(|e| format!("no graphics adapter: {e}"))?;
        let name = adapter.get_info().name;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("office"), memory_hints: wgpu::MemoryHints::MemoryUsage, ..Default::default() })).map_err(|e| format!("no device: {e}"))?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("office"), source: wgpu::ShaderSource::Wgsl(include_str!("office.wgsl").into()) });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor { label: Some("frame"), size: std::mem::size_of::<Frame>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("frame"), layout: &layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }] });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("office"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("office"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })] },
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: COLOR, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            multiview_mask: None,
            cache: None,
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh"), size: std::mem::size_of_val(mesh).max(16) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        queue.write_buffer(&vertices, 0, bytemuck::cast_slice(mesh));
        Ok(Self { device, queue, pipeline, uniforms, bind, vertices, count: mesh.len() as u32, target: None, adapter: name })
    }

    fn target(&mut self, width: u32, height: u32) -> &Target {
        let stale = self.target.as_ref().is_none_or(|t| t.width != width || t.height != height);
        if stale {
            let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
            let texture = |format, usage| self.device.create_texture(&wgpu::TextureDescriptor { label: Some("office"), size, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] });
            let color = texture(COLOR, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
            let depth = texture(DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
            let row = (width * 4).div_ceil(256) * 256;
            let readback = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: (row * height) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
            self.target = Some(Target { width, height, color, depth, readback, row });
        }
        self.target.as_ref().unwrap()
    }

    /// Draw one frame and hand back its BGRA bytes, rows packed tight.
    pub fn draw(&mut self, width: u32, height: u32, frame: &Frame, clear: [f64; 3]) -> Result<Vec<u8>, String> {
        let (width, height) = (width.max(1), height.max(1));
        self.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(frame));
        self.target(width, height);
        let t = self.target.as_ref().unwrap();
        let view = t.color.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("office") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("office"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: clear[0], g: clear[1], b: clear[2], a: 1.0 }), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &t.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..self.count, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &t.color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &t.readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(t.row), rows_per_image: Some(height) } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let slice = t.readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| format!("poll: {e}"))?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| format!("map: {e}"))?;
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        {
            let data = slice.get_mapped_range().map_err(|e| format!("map: {e:?}"))?;
            for y in 0..height as usize {
                let start = y * t.row as usize;
                out.extend_from_slice(&data[start..start + width as usize * 4]);
            }
        }
        t.readback.unmap();
        Ok(out)
    }
}
