//! The window's own frame: ByteHive draws without Windows' title bar, so the
//! top bar is where the window is dragged (double-click maximizes), the
//! minimize, maximize and close buttons sit at its right, and the edges
//! resize it.

use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, ResizeDirection, Sense, Stroke, Vec2, ViewportCommand};

/// Width the window buttons take at the top right.
pub const CONTROLS_W: f32 = 46.0 * 3.0;
const EDGE: f32 = 5.0;

fn maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

/// Make `rect` drag the window; double-click maximizes or restores. Call
/// before the widgets on top of it so they keep their clicks.
pub fn drag_area(ui: &mut egui::Ui, rect: Rect) {
    let r = ui.interact(rect, ui.make_persistent_id("window-drag"), Sense::click_and_drag());
    if r.double_clicked() {
        let m = maximized(ui.ctx());
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!m));
    } else if r.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// Minimize, maximize and close at the window's top right.
pub fn controls(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let right = ctx.content_rect().right();
    let top = ctx.content_rect().top();
    let max = maximized(&ctx);
    for (i, kind) in [0u8, 1, 2].into_iter().enumerate() {
        let rect = Rect::from_min_size(Pos2::new(right - CONTROLS_W + i as f32 * 46.0, top), Vec2::new(46.0, 38.0));
        let r = ui.interact(rect, ui.make_persistent_id(("win-btn", kind)), Sense::click());
        let p = ui.painter();
        let hot = r.hovered();
        if hot {
            let fill = if kind == 2 { Color32::from_rgb(0xe3, 0x40, 0x2f) } else { Color32::from_white_alpha(if r.is_pointer_button_down_on() { 14 } else { 26 }) };
            p.rect_filled(rect, 0.0, fill);
        }
        let ink = if hot && kind == 2 { Color32::WHITE } else if hot { Color32::from_gray(230) } else { Color32::from_rgb(0x83, 0x83, 0x8c) };
        let c = rect.center();
        let s = 5.0;
        match kind {
            0 => {
                p.line_segment([c + Vec2::new(-s, 0.0), c + Vec2::new(s, 0.0)], Stroke::new(1.0, ink));
            }
            1 if max => {
                let back = Rect::from_center_size(c + Vec2::new(1.5, -1.5), Vec2::splat(7.5));
                let front = Rect::from_center_size(c + Vec2::new(-1.0, 1.0), Vec2::splat(7.5));
                p.rect_stroke(back, 1.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
                p.rect_filled(front, 1.0, Color32::from_rgb(5, 5, 6));
                p.rect_stroke(front, 1.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
            }
            1 => {
                p.rect_stroke(Rect::from_center_size(c, Vec2::splat(9.0)), 1.0, Stroke::new(1.0, ink), egui::StrokeKind::Middle);
            }
            _ => {
                let st = Stroke::new(1.2, ink);
                p.line_segment([c + Vec2::new(-s, -s), c + Vec2::new(s, s)], st);
                p.line_segment([c + Vec2::new(-s, s), c + Vec2::new(s, -s)], st);
            }
        }
        if r.clicked() {
            match kind {
                0 => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                1 => ctx.send_viewport_cmd(ViewportCommand::Maximized(!max)),
                _ => ctx.send_viewport_cmd(ViewportCommand::Close),
            }
        }
    }
}

/// Which edge or corner `pos` is on, if any.
pub fn edge_at(window: Rect, pos: Pos2) -> Option<ResizeDirection> {
    let (l, r) = (pos.x - window.left() < EDGE, window.right() - pos.x < EDGE);
    let (t, b) = (pos.y - window.top() < EDGE, window.bottom() - pos.y < EDGE);
    Some(match (l, r, t, b) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return None,
    })
}

/// The edges resize the window (not while it is maximized).
pub fn resize_edges(ctx: &egui::Context) {
    if maximized(ctx) {
        return;
    }
    let window = ctx.content_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let Some(dir) = edge_at(window, pos) else { return };
    ctx.set_cursor_icon(match dir {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        _ => CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_and_corners() {
        let w = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        assert_eq!(edge_at(w, Pos2::new(1.0, 1.0)), Some(ResizeDirection::NorthWest));
        assert_eq!(edge_at(w, Pos2::new(798.0, 300.0)), Some(ResizeDirection::East));
        assert_eq!(edge_at(w, Pos2::new(400.0, 598.0)), Some(ResizeDirection::South));
        assert_eq!(edge_at(w, Pos2::new(400.0, 300.0)), None);
    }
}
