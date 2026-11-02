//! The ByteHive logo on Home. It comes alive (breathes, glows, its comb cells
//! light up one after another) only while agents are working or the pointer is
//! on it, and only while the window has focus. Otherwise it is a still image
//! and asks for no frames at all.

use eframe::egui::{self, Color32, Rect, Sense, Vec2};
use std::f32::consts::TAU;
use std::time::Duration;

/// Comb cells in the order they light up, as (column, row) of the 2C comb:
/// the middle one, the ring around it, then the outer cells.
const CELLS: [(f32, f32); 13] = [(0., 0.), (1., 0.), (0., 1.), (-1., 1.), (-1., 0.), (-1., -1.), (0., -1.), (1., -1.), (1., 1.), (0., 2.), (-2., 0.), (0., -2.), (2., 0.)];
const CYCLE: f32 = 3.6;
const STAGGER: f32 = 0.16;
const BREATHE: f32 = 4.2;

struct Art {
    logo: egui::TextureHandle,
    cells: Vec<egui::TextureHandle>,
}

/// The logo texture plus one glow texture per comb cell, made once.
fn art(ctx: &egui::Context) -> std::sync::Arc<Art> {
    let id = egui::Id::unique("bytehive-logo-art");
    if let Some(a) = ctx.data(|d| d.get_temp::<std::sync::Arc<Art>>(id)) {
        return a;
    }
    let img = image::load_from_memory(include_bytes!("../assets/icons/128x128.png")).map(|i| i.into_rgba8()).unwrap_or_default();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let logo = ctx.load_texture("logo-128", egui::ColorImage::from_rgba_unmultiplied([w, h], img.as_raw()), egui::TextureOptions::LINEAR);
    let cells = CELLS
        .iter()
        .enumerate()
        .map(|(n, &(i, j))| {
            let mut px = vec![0u8; w * h * 4];
            for y in 0..h {
                for x in 0..w {
                    let a = img.as_raw()[(y * w + x) * 4 + 3];
                    // The icon shows the 24-unit viewBox from 0.5 to 23.5.
                    let vx = (x as f32 + 0.5) * 23.0 / w as f32 + 0.5;
                    let vy = (y as f32 + 0.5) * 23.0 / h as f32 + 0.5;
                    if a > 0 && in_cell(vx - cell_x(i, j), vy - (12.0 + 3.9 * j)) {
                        px[(y * w + x) * 4..][..4].copy_from_slice(&[255, 246, 208, a]);
                    }
                }
            }
            ctx.load_texture(format!("logo-cell-{n}"), egui::ColorImage::from_rgba_unmultiplied([w, h], &px), egui::TextureOptions::LINEAR)
        })
        .collect();
    let a = std::sync::Arc::new(Art { logo, cells });
    ctx.data_mut(|d| d.insert_temp(id, a.clone()));
    a
}

/// Odd comb rows sit half a cell to the right.
fn cell_x(col: f32, row: f32) -> f32 {
    12.0 + 4.5033 * (col + 0.5 * row.rem_euclid(2.0))
}

/// Inside a pointy-top comb cell centred on the origin.
fn in_cell(dx: f32, dy: f32) -> bool {
    let (w, h) = (2.2517, 2.6);
    dx.abs() <= w && dy.abs() <= h - dx.abs() / w * (h / 2.0)
}

/// How lit cell `n` is at time `t`: up to 0.55 then back to dark, in turn.
pub fn cell_light(n: usize, t: f32) -> f32 {
    let p = (t - n as f32 * STAGGER).rem_euclid(CYCLE) / CYCLE;
    let ease = |x: f32| x * x * (3.0 - 2.0 * x);
    match p {
        p if p < 0.18 => 0.55 * ease(p / 0.18),
        p if p < 0.40 => 0.55 * (1.0 - ease((p - 0.18) / 0.22)),
        _ => 0.0,
    }
}

/// Whether the logo should move this frame.
pub fn animating(working: usize, hovered: bool, focused: bool) -> bool {
    focused && (working > 0 || hovered)
}

pub fn hive_logo(ui: &mut egui::Ui, size: f32, working: usize) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let focused = ui.ctx().input(|i| i.viewport().focused.unwrap_or(true));
    let live = animating(working, resp.hovered(), focused);
    let art = art(ui.ctx());
    let t = if live { ui.input(|i| i.time) as f32 } else { 0.0 };
    // 0 at rest, 1 at the top of a breath.
    let breath = if live { 0.5 - 0.5 * (t * TAU / BREATHE).cos() } else { 0.0 };
    let painter = ui.painter();
    let halo = (0.32 + 0.36 * breath) * if live { 1.0 } else { 0.6 };
    let glow = egui::epaint::Shadow { offset: [0, 0], blur: (size * 0.6) as u8, spread: 0, color: Color32::from_rgba_unmultiplied(245, 165, 36, (halo * 90.0) as u8) };
    painter.add(glow.as_shape(rect.shrink(size * 0.22), size));
    let r = Rect::from_center_size(rect.center(), rect.size() * (1.0 + 0.035 * breath));
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(art.logo.id(), r, uv, Color32::WHITE);
    if !live {
        return;
    }
    for (n, cell) in art.cells.iter().enumerate() {
        let a = cell_light(n, t);
        if a > 0.01 {
            painter.image(cell.id(), r, uv, Color32::from_white_alpha((a * 255.0) as u8));
        }
    }
    // About 30 frames a second while alive; none at all once it stops.
    ui.ctx().request_repaint_after(Duration::from_millis(33));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_only_moves_while_agents_work_or_it_is_pointed_at() {
        assert!(!animating(0, false, true));
        assert!(animating(2, false, true));
        assert!(animating(0, true, true));
        assert!(!animating(2, true, false));
    }

    #[test]
    fn cells_light_one_after_another() {
        assert!(cell_light(0, 0.65) > 0.5);
        assert!(cell_light(1, 0.65) < cell_light(0, 0.65));
        assert_eq!(cell_light(0, 2.0), 0.0);
    }

    #[test]
    fn the_cell_test_matches_the_comb() {
        assert!(in_cell(0.0, 0.0));
        assert!(in_cell(2.0, 0.0));
        assert!(!in_cell(0.0, 2.7));
        assert!(!in_cell(2.0, 2.4));
    }
}
