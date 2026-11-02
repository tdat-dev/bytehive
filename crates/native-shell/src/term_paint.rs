//! Drawing a terminal grid: cells, colours, the selection and the cursor,
//! for the live panes and the replay player alike.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::CursorShape;
use eframe::egui::{self, FontId, Pos2, Rect, Vec2};

use crate::color::{c32, rgb_of, BG_DEFAULT};
use crate::theme::TEXT;

/// Line height over the font's own, as the web app's terminal had it
/// (xterm's lineHeight 1.2).
pub const LINE_HEIGHT: f32 = 1.2;

/// One cell's (width, height) in `font`, from one measured run of glyphs.
pub fn cell_metrics(painter: &egui::Painter, font: &FontId) -> (f32, f32) {
    let g = painter.layout_no_wrap("MMMMMMMMMM".to_string(), font.clone(), TEXT);
    (g.rect.width() / 10.0, (g.rect.height() * LINE_HEIGHT).round())
}

/// Paint an emulator grid and its cursor into `rect`: the live panes and the
/// replay player draw the same way.
pub fn paint_grid<T: EventListener>(painter: &egui::Painter, term: &Term<T>, rect: Rect, size: (usize, usize), font: &FontId, cell: (f32, f32)) {
    let (cols, rows) = size;
    let (cell_w, cell_h) = cell;
    // Glyphs sit in the middle of the taller line.
    let lift = ((cell_h - cell_h / LINE_HEIGHT) / 2.0).round();
    let content = term.renderable_content();
    for indexed in content.display_iter {
        let row = indexed.point.line.0 + content.display_offset as i32;
        if row < 0 || row as usize >= rows {
            continue;
        }
        let col = indexed.point.column.0;
        if col >= cols {
            continue;
        }
        let cell = &indexed.cell;
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }
        let mut fg = rgb_of(cell.fg);
        let mut bg = rgb_of(cell.bg);
        if cell.flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        let min = Pos2::new(rect.left() + col as f32 * cell_w, rect.top() + row as f32 * cell_h);
        if content.selection.is_some_and(|s| s.contains(indexed.point)) {
            painter.rect_filled(Rect::from_min_size(min, Vec2::new(cell_w, cell_h)), 0.0, egui::Color32::from_rgba_unmultiplied(110, 150, 255, 90));
        } else if bg != BG_DEFAULT {
            painter.rect_filled(Rect::from_min_size(min, Vec2::new(cell_w, cell_h)), 0.0, c32(bg));
        }
        if cell.c != ' ' {
            painter.text(min + Vec2::new(0.0, lift), egui::Align2::LEFT_TOP, cell.c.to_string(), font.clone(), c32(fg));
        }
    }

    let cursor = &content.cursor;
    let crow = cursor.point.line.0 + content.display_offset as i32;
    let ccol = cursor.point.column.0;
    if crow >= 0 && (crow as usize) < rows && ccol < cols {
        let min = Pos2::new(rect.left() + ccol as f32 * cell_w, rect.top() + crow as f32 * cell_h);
        let cur = TEXT;
        match cursor.shape {
            CursorShape::Block => {
                painter.rect_filled(Rect::from_min_size(min, Vec2::new(cell_w, cell_h)), 0.0, cur);
            }
            CursorShape::Underline => {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(min.x, min.y + cell_h - 2.0), Vec2::new(cell_w, 2.0)),
                    0.0,
                    cur,
                );
            }
            CursorShape::Beam => {
                painter.rect_filled(Rect::from_min_size(min, Vec2::new(2.0, cell_h)), 0.0, cur);
            }
            CursorShape::HollowBlock => {
                painter.rect_stroke(Rect::from_min_size(min, Vec2::new(cell_w, cell_h)), 0.0, egui::Stroke::new(1.0, cur), egui::StrokeKind::Inside);
            }
            CursorShape::Hidden => {}
        }
    }
}

/// Plain text of every visible row: what the painter reads, as strings.
pub fn grid_text<T: EventListener>(term: &Term<T>) -> Vec<String> {
    let content = term.renderable_content();
    let mut rows = vec![String::new(); 300];
    let mut n = 0;
    for indexed in content.display_iter {
        let row = indexed.point.line.0 + content.display_offset as i32;
        if row < 0 || row as usize >= rows.len() {
            continue;
        }
        n = n.max(row as usize + 1);
        rows[row as usize].push(indexed.cell.c);
    }
    rows.truncate(n);
    rows
}
