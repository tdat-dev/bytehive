//! The office's renderer: wgpu on DX12 draws the scene offscreen, four samples
//! a pixel, and the frame is read back for GPUI to show (see gpu.rs for why).
//! It keeps geometry and pictures on the card by id, and draws a list of
//! draws in the order given, each with its look (render_types.rs).

use glam::Mat4;

use super::geom::Geom;
use super::gpu::Vertex;
use super::readback::Readback;
use super::render_pipes::{COLOR, DEPTH, Layouts, SAMPLES, layouts, pipeline, shadow_pipeline};
use super::render_tex::{paint_pool, upload};
use super::render_types::{DRAW_STRIDE, Draw, DrawU, FrameIn, FrameU, LOOKS, Look, MeshId, TexId};
use super::shadow::{MAP, sun_dir};

struct Mesh {
    buffer: wgpu::Buffer,
    count: u32,
    room: u64,
}

struct Target {
    width: u32,
    height: u32,
    msaa: wgpu::TextureView,
    color: wgpu::Texture,
    depth: wgpu::TextureView,
    readback: Readback,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layouts: Layouts,
    pipes: Vec<(Look, wgpu::RenderPipeline)>,
    shadow_pipe: wgpu::RenderPipeline,
    shadow_view: wgpu::TextureView,
    frame_buf: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    frame_shadow_bind: wgpu::BindGroup,
    draw_buf: wgpu::Buffer,
    draw_bind: wgpu::BindGroup,
    draw_room: u64,
    sampler: wgpu::Sampler,
    meshes: Vec<Option<Mesh>>,
    pictures: Vec<Option<wgpu::BindGroup>>,
    target: Option<Target>,
    pub adapter: String,
    /// Frames handed to the card so far.
    pub drawn: u64,
}

impl Renderer {
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let power = if std::env::var("BYTEHIVE_OFFICE3D_GPU").is_ok_and(|v| v == "high") { wgpu::PowerPreference::HighPerformance } else { wgpu::PowerPreference::LowPower };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: power, ..Default::default() })).map_err(|e| format!("no graphics adapter: {e}"))?;
        let name = adapter.get_info().name;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("office"), memory_hints: wgpu::MemoryHints::MemoryUsage, ..Default::default() })).map_err(|e| format!("no device: {e}"))?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("office scene"), source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()) });
        let layouts = layouts(&device);
        let main = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("office"), bind_group_layouts: &[Some(&layouts.frame), Some(&layouts.draw), Some(&layouts.picture)], immediate_size: 0 });
        let sun = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sun"), bind_group_layouts: &[Some(&layouts.frame_shadow), Some(&layouts.draw)], immediate_size: 0 });
        let pipes = LOOKS.iter().map(|&l| (l, pipeline(&device, &shader, &main, l))).collect();
        let shadow_pipe = shadow_pipeline(&device, &shader, &sun);
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sun shadow"),
            size: wgpu::Extent3d { width: MAP, height: MAP, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&Default::default());
        let compare = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("sun shadow"), compare: Some(wgpu::CompareFunction::LessEqual), ..Default::default() });
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("frame"), size: std::mem::size_of::<FrameU>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &layouts.frame,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&compare) },
            ],
        });
        let frame_shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("frame for the sun"), layout: &layouts.frame_shadow, entries: &[wgpu::BindGroupEntry { binding: 0, resource: frame_buf.as_entire_binding() }] });
        let (draw_buf, draw_bind) = draw_buffer(&device, &layouts, 64);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("picture"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 4,
            ..Default::default()
        });
        let mut out = Self { device, queue, layouts, pipes, shadow_pipe, shadow_view, frame_buf, frame_bind, frame_shadow_bind, draw_buf, draw_bind, draw_room: 64, sampler, meshes: vec![], pictures: vec![], target: None, adapter: name, drawn: 0 };
        out.picture(vec![255; 4], 1, 1);
        out.picture(paint_pool(128), 128, 128);
        Ok(out)
    }

    fn upload_mesh(&self, vertices: &[Vertex]) -> Mesh {
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        let room = (bytes.len() as u64).max(64);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor { label: Some("mesh"), size: room, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        self.queue.write_buffer(&buffer, 0, bytes);
        Mesh { buffer, count: vertices.len() as u32, room }
    }

    /// Geometry onto the card. An empty one is kept as nothing to draw.
    pub fn geom(&mut self, g: &Geom) -> MeshId {
        self.mesh(&g.vertices())
    }

    pub fn mesh(&mut self, vertices: &[Vertex]) -> MeshId {
        let mesh = self.upload_mesh(vertices);
        match self.meshes.iter().position(Option::is_none) {
            Some(i) => {
                self.meshes[i] = Some(mesh);
                MeshId(i)
            }
            None => {
                self.meshes.push(Some(mesh));
                MeshId(self.meshes.len() - 1)
            }
        }
    }

    /// New vertices for a mesh that changes every frame (the rising marks), in place when they fit.
    pub fn remesh(&mut self, id: MeshId, vertices: &[Vertex]) {
        let bytes: &[u8] = bytemuck::cast_slice(vertices);
        match self.meshes.get_mut(id.0).and_then(Option::as_mut) {
            Some(m) if bytes.len() as u64 <= m.room => {
                self.queue.write_buffer(&m.buffer, 0, bytes);
                m.count = vertices.len() as u32;
            }
            _ => {
                let fresh = self.upload_mesh(vertices);
                if id.0 >= self.meshes.len() {
                    self.meshes.resize_with(id.0 + 1, || None);
                }
                self.meshes[id.0] = Some(fresh);
            }
        }
    }

    pub fn free(&mut self, id: MeshId) {
        if let Some(m) = self.meshes.get_mut(id.0) {
            *m = None;
        }
    }

    /// A picture onto the card: RGBA in sRGB, top row first.
    pub fn picture(&mut self, rgba: Vec<u8>, w: usize, h: usize) -> TexId {
        let view = upload(&self.device, &self.queue, rgba, w, h);
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("picture"),
            layout: &self.layouts.picture,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) }],
        });
        self.pictures.push(Some(bind));
        TexId(self.pictures.len() - 1)
    }

    fn target(&mut self, width: u32, height: u32) {
        if self.target.as_ref().is_some_and(|t| t.width == width && t.height == height) {
            return;
        }
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = |format, samples, usage| self.device.create_texture(&wgpu::TextureDescriptor { label: Some("office"), size, mip_level_count: 1, sample_count: samples, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] });
        let msaa = texture(COLOR, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
        let color = texture(COLOR, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let depth = texture(DEPTH, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
        let readback = Readback::new(&self.device, width, height);
        self.target = Some(Target { width, height, msaa, color, depth, readback });
    }

    fn uniforms(&mut self, f: &FrameIn, width: u32, height: u32, draws: &[Draw]) {
        let lit = |hex: &str, k: f32| {
            let v = super::geom::hex(hex) * k;
            [v.x, v.y, v.z, 0.0]
        };
        let sun = sun_dir();
        let frame = FrameU {
            view: f.view.to_cols_array_2d(),
            proj: f.proj.to_cols_array_2d(),
            light: f.light.to_cols_array_2d(),
            sun: [sun.x, sun.y, sun.z, 0.0],
            sun_color: lit("#ffe2c0", 1.25),
            sky: lit("#b9c8f0", 1.1),
            ground: lit("#4a3a34", 1.1),
            eye: [f.eye.x, f.eye.y, f.eye.z, 1.05],
            shadow: [-0.0006, 0.03, 0.62, if f.shadows { 1.0 } else { 0.0 }],
            band: [f.band.x, f.band.y, f.band.z, 0.0],
            viewport: [width as f32, height as f32, f.star_half, 0.0],
        };
        self.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&frame));
        if draws.len() as u64 > self.draw_room {
            self.draw_room = (draws.len() as u64).next_power_of_two();
            (self.draw_buf, self.draw_bind) = draw_buffer(&self.device, &self.layouts, self.draw_room);
        }
        let mut bytes = vec![0u8; draws.len() * DRAW_STRIDE as usize];
        for (i, d) in draws.iter().enumerate() {
            let u: DrawU = d.uniforms();
            bytes[i * DRAW_STRIDE as usize..][..std::mem::size_of::<DrawU>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
        self.queue.write_buffer(&self.draw_buf, 0, &bytes);
    }

    /// Draw a frame if a readback buffer is free, and hand back the newest
    /// finished one as (width, height, BGRA bytes). Never waits on the GPU.
    pub fn draw(&mut self, width: u32, height: u32, f: &FrameIn, draws: &[Draw]) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
        let (width, height) = (width.max(1), height.max(1));
        self.device.poll(wgpu::PollType::Poll).map_err(|e| format!("poll: {e}"))?;
        let done = self.target.as_mut().and_then(|t| t.readback.take().map(|bytes| (t.width, t.height, bytes)));
        self.target(width, height);
        let Some(slot) = self.target.as_ref().unwrap().readback.free() else { return Ok(done) };
        self.uniforms(f, width, height, draws);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("office") });
        if f.reshadow {
            self.sun_pass(&mut encoder, draws);
        }
        let t = self.target.as_ref().unwrap();
        let view = t.color.create_view(&Default::default());
        {
            let c = f.clear;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("office"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &t.msaa, depth_slice: None, resolve_target: Some(&view), ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: c.x as f64, g: c.y as f64, b: c.z as f64, a: 1.0 }), store: wgpu::StoreOp::Discard } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &t.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_bind, &[]);
            let mut current: Option<Look> = None;
            for (i, d) in draws.iter().enumerate() {
                let Some(Some(mesh)) = self.meshes.get(d.mesh.0) else { continue };
                let Some(Some(picture)) = self.pictures.get(d.texture.0) else { continue };
                if mesh.count == 0 {
                    continue;
                }
                if current != Some(d.look) {
                    let pipe = &self.pipes.iter().find(|(l, _)| *l == d.look).unwrap().1;
                    pass.set_pipeline(pipe);
                    current = Some(d.look);
                }
                pass.set_bind_group(1, &self.draw_bind, &[(i as u64 * DRAW_STRIDE) as u32]);
                pass.set_bind_group(2, picture, &[]);
                pass.set_vertex_buffer(0, mesh.buffer.slice(..));
                pass.draw(0..mesh.count, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &t.color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: t.readback.buffer(slot), layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(t.readback.row), rows_per_image: Some(height) } },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        self.target.as_mut().unwrap().readback.submitted(slot);
        self.drawn += 1;
        Ok(done)
    }

    /// The newest finished frame, without drawing another; and whether one is still on its way.
    pub fn collect(&mut self) -> (Option<(u32, u32, Vec<u8>)>, bool) {
        let _ = self.device.poll(wgpu::PollType::Poll);
        match self.target.as_mut() {
            Some(t) => (t.readback.take().map(|bytes| (t.width, t.height, bytes)), t.readback.busy()),
            None => (None, false),
        }
    }

    /// Wait for the card to finish what it was given (tests and shots only).
    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// The sun's map, from everything that casts.
    fn sun_pass(&self, encoder: &mut wgpu::CommandEncoder, draws: &[Draw]) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sun shadow"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.shadow_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.shadow_pipe);
        pass.set_bind_group(0, &self.frame_shadow_bind, &[]);
        for (i, d) in draws.iter().enumerate().filter(|(_, d)| d.cast) {
            let Some(Some(mesh)) = self.meshes.get(d.mesh.0) else { continue };
            pass.set_bind_group(1, &self.draw_bind, &[(i as u64 * DRAW_STRIDE) as u32]);
            pass.set_vertex_buffer(0, mesh.buffer.slice(..));
            pass.draw(0..mesh.count, 0..1);
        }
    }
}

fn draw_buffer(device: &wgpu::Device, layouts: &Layouts, room: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("draws"), size: room * DRAW_STRIDE, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("draws"),
        layout: &layouts.draw,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64) }) }],
    });
    (buffer, bind)
}

/// The sun's view-projection for a tower `roof_y` high, floors centred on `x`.
pub fn light_for(x: f32, roof_y: f32) -> Mat4 {
    super::shadow::fit_sun(x, roof_y)
}

#[cfg(test)]
mod tests {
    fn private_mb() -> u64 {
        let out = std::process::Command::new("powershell").args(["-NoProfile", "-c", &format!("(Get-Process -Id {}).PrivateMemorySize64", std::process::id())]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap_or(0) / 1_048_576
    }

    /// Where the renderer's own memory goes (BYTEHIVE_OFFICE_MEM=1).
    #[test]
    #[ignore]
    fn weigh_the_device() {
        if std::env::var("BYTEHIVE_OFFICE_MEM").is_err() {
            return;
        }
        let m0 = private_mb();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::LowPower, ..Default::default() })).unwrap();
        let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: None, memory_hints: wgpu::MemoryHints::MemoryUsage, ..Default::default() })).unwrap();
        let m1 = private_mb();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()) });
        let layouts = super::layouts(&device);
        let main = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layouts.frame), Some(&layouts.draw), Some(&layouts.picture)], immediate_size: 0 });
        let one = super::pipeline(&device, &shader, &main, super::Look::Lit);
        let m2 = private_mb();
        let rest: Vec<_> = super::LOOKS.iter().map(|&l| super::pipeline(&device, &shader, &main, l)).collect();
        let m3 = private_mb();
        eprintln!("device +{} one pipeline +{} ten more +{}", m1 - m0, m2 - m1, m3 - m2);
        drop((one, rest));
    }

    #[test]
    fn the_scene_shader_parses_and_validates() {
        let text = include_str!("scene.wgsl");
        let module = naga::front::wgsl::parse_str(text).unwrap_or_else(|e| panic!("{}", e.emit_to_string(text)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&module).expect("valid WGSL");
    }

    #[test]
    fn uniforms_are_laid_out_as_the_shader_reads_them() {
        assert_eq!(std::mem::size_of::<super::FrameU>(), 3 * 64 + 8 * 16);
        assert_eq!(std::mem::size_of::<super::DrawU>(), 64 + 4 * 16);
        assert!(std::mem::size_of::<super::DrawU>() as u64 <= super::DRAW_STRIDE);
    }
}
