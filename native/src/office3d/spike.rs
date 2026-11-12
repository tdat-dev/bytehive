//! P0 spike (BYTEHIVE_OFFICE3D=spike): the stand-in office turning in the
//! Company page's floors panel, each frame drawn by wgpu and shown by GPUI as
//! an image, with what it costs written over it. A go or stop gate for the
//! plan in docs/superpowers/plans/2026-11-11-native-3d-office.md.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

use gpui_kit::*;

use super::gpu::{Frame, Gpu};
use super::mesh::{spike_scene, spike_view};

struct Spike {
    gpu: Result<Gpu, String>,
    started: Instant,
    shown: Option<Arc<RenderImage>>,
    /// The frame before: dropped one frame late, so GPUI's atlas texture never
    /// empties (it would destroy it and make a new one every frame).
    before: Option<Arc<RenderImage>>,
    /// Frames in the last second, and when that second began.
    frames: u32,
    second: Instant,
    fps: u32,
    /// GPUI-thread time of the last draw, ms (draw, wait, copy, wrap).
    draw_ms: f32,
}

thread_local! {
    static SPIKE: RefCell<Option<Spike>> = const { RefCell::new(None) };
}

pub fn wanted() -> bool {
    std::env::var("BYTEHIVE_OFFICE3D").is_ok_and(|v| v.starts_with("spike"))
}

/// spike-still: one frame, then the same image: what the device costs alone.
fn still() -> bool {
    std::env::var("BYTEHIVE_OFFICE3D").is_ok_and(|v| v == "spike-still")
}

/// What the spike measured, for the overlay and the log.
pub fn line() -> String {
    SPIKE.with(|s| match s.borrow().as_ref() {
        Some(Spike { gpu: Err(e), .. }) => format!("3D off: {e}"),
        Some(s) => format!("{} fps · {:.1} ms a frame on the UI thread · {}", s.fps, s.draw_ms, s.gpu.as_ref().map(|g| g.adapter.as_str()).unwrap_or("")),
        None => "starting".into(),
    })
}

/// The panel: a canvas that draws a frame at its own size and asks for the next.
pub fn view() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |bounds, _, window, _| {
            let scale = window.scale_factor();
            let w = (f32::from(bounds.size.width) * scale).round() as u32;
            let h = (f32::from(bounds.size.height) * scale).round() as u32;
            let image = SPIKE.with(|cell| {
                let mut cell = cell.borrow_mut();
                let s = cell.get_or_insert_with(|| Spike { gpu: Gpu::new(&spike_scene()), started: Instant::now(), shown: None, before: None, frames: 0, second: Instant::now(), fps: 0, draw_ms: 0.0 });
                // spike-still: one frame, then the same image.
                if still() && let Some(shown) = &s.shown {
                    return Some((shown.clone(), None));
                }
                let begun = Instant::now();
                let t = s.started.elapsed().as_secs_f32() * 0.35;
                let frame = Frame { view_proj: spike_view(t, w as f32 / h.max(1) as f32).to_cols_array_2d(), model: glam::Mat4::IDENTITY.to_cols_array_2d(), sun: [0.4, 0.8, 0.3, 0.0] };
                let gpu = s.gpu.as_mut().ok()?;
                // Nothing new yet (both frames still on the card): keep the picture.
                let Some((fw, fh, bytes)) = gpu.draw(w, h, &frame, [0.020, 0.020, 0.024]).ok()? else {
                    return s.shown.clone().map(|shown| (shown, None));
                };
                let buffer = image::ImageBuffer::from_raw(fw, fh, bytes)?;
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
                s.draw_ms = begun.elapsed().as_secs_f32() * 1000.0;
                s.frames += 1;
                if s.second.elapsed().as_secs_f32() >= 1.0 {
                    s.fps = s.frames;
                    s.frames = 0;
                    s.second = Instant::now();
                }
                let old = std::mem::replace(&mut s.before, s.shown.replace(image.clone()));
                Some((image, old))
            });
            if let Some((image, old)) = image {
                if let Some(old) = old {
                    let _ = window.drop_image(old);
                }
                let _ = window.paint_image(bounds, bounds, Corners::default(), image, 0, false);
            }
            if !still() {
                window.request_animation_frame();
            }
        },
    )
    .size_full()
}
