//! Pictures for the card: sRGB RGBA with their mipmaps made here, as three
//! makes them for a power-of-two CanvasTexture, and the soft round pool every
//! thrown light is drawn with (build.ts poolPaint).

/// The pool: white, its alpha falling off from the middle in a smooth curve,
/// squared, as the canvas's radial gradient of eleven stops paints it.
pub fn paint_pool(size: usize) -> Vec<u8> {
    let stop = |k: f64| (1.0 - k * k * (3.0 - 2.0 * k)).powi(2);
    let mut out = Vec::with_capacity(size * size * 4);
    let c = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let r = ((x as f64 + 0.5 - c).hypot(y as f64 + 0.5 - c) / c).min(1.0);
            // Between the stops at tenths, the gradient runs straight.
            let i = ((r * 10.0).floor() as usize).min(9);
            let t = r * 10.0 - i as f64;
            let a = stop(i as f64 / 10.0) + (stop((i + 1) as f64 / 10.0) - stop(i as f64 / 10.0)) * t;
            out.extend([255, 255, 255, (a * 255.0).round() as u8]);
        }
    }
    out
}

fn to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb(c: f32) -> u8 {
    let v = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The next level down: each pixel the mean of four, colour averaged in linear light.
fn halved(rgba: &[u8], w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = Vec::with_capacity(nw * nh * 4);
    for y in 0..nh {
        for x in 0..nw {
            let taps = [(2 * x, 2 * y), ((2 * x + 1).min(w - 1), 2 * y), (2 * x, (2 * y + 1).min(h - 1)), ((2 * x + 1).min(w - 1), (2 * y + 1).min(h - 1))];
            let px = |(tx, ty): (usize, usize)| &rgba[(ty * w + tx) * 4..(ty * w + tx) * 4 + 4];
            for ch in 0..3 {
                out.push(to_srgb(taps.iter().map(|&t| to_linear(px(t)[ch])).sum::<f32>() / 4.0));
            }
            out.push((taps.iter().map(|&t| px(t)[3] as u32).sum::<u32>() as f32 / 4.0).round() as u8);
        }
    }
    (out, nw, nh)
}

/// Every level, the picture first.
pub fn mip_chain(rgba: Vec<u8>, w: usize, h: usize) -> Vec<(Vec<u8>, usize, usize)> {
    let mut levels = vec![(rgba, w, h)];
    while let Some((last, lw, lh)) = levels.last()
        && (*lw > 1 || *lh > 1)
    {
        let next = halved(last, *lw, *lh);
        levels.push(next);
    }
    levels
}

/// A picture on the card, with its mipmaps, sampled smooth.
pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, rgba: Vec<u8>, w: usize, h: usize) -> wgpu::TextureView {
    let levels = mip_chain(rgba, w, h);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("picture"),
        size: wgpu::Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, (bytes, lw, lh)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(*lw as u32 * 4), rows_per_image: Some(*lh as u32) },
            wgpu::Extent3d { width: *lw as u32, height: *lh as u32, depth_or_array_layers: 1 },
        );
    }
    texture.create_view(&Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pool_is_full_in_the_middle_and_gone_at_the_edge() {
        let p = paint_pool(128);
        let a = |x: usize, y: usize| p[(y * 128 + x) * 4 + 3];
        assert!(a(64, 64) > 250 && a(0, 0) == 0 && a(127, 64) < 3);
        assert!(a(64, 64) > a(80, 64) && a(80, 64) > a(100, 64));
        let levels = mip_chain(p, 128, 128);
        assert_eq!(levels.len(), 8);
        assert_eq!((levels[7].1, levels[7].2), (1, 1));
    }
}
