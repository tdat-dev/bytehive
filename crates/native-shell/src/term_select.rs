//! Mouse work on a terminal: drag to select text (Ctrl+C then copies it
//! instead of interrupting), double-click a word, a right-click menu (Copy,
//! Paste, Select all), and dropping files from Explorer types their paths in,
//! quoted when they hold a space, the way the web app's terminal took a drop.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Term;
use eframe::egui::{self, Pos2, Rect};

/// The grid cell under a screen position, and which half of it.
pub fn cell_at<T: EventListener>(term: &Term<T>, rect: Rect, cell: (f32, f32), pos: Pos2) -> (Point, Side) {
    let (w, h) = cell;
    let cols = term.columns().max(1);
    let rows = term.screen_lines().max(1);
    let x = ((pos.x - rect.left()) / w).max(0.0);
    let col = (x as usize).min(cols - 1);
    let row = (((pos.y - rect.top()) / h).max(0.0) as usize).min(rows - 1);
    let side = if x.fract() < 0.5 { Side::Left } else { Side::Right };
    let offset = term.grid().display_offset() as i32;
    (Point::new(Line(row as i32 - offset), Column(col)), side)
}

/// A file dropped from Explorer, as typed into a terminal.
pub fn drop_text(paths: &[std::path::PathBuf]) -> String {
    paths
        .iter()
        .map(|p| {
            let s = p.to_string_lossy();
            if s.contains(' ') { format!("\"{s}\"") } else { s.to_string() }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What the right-click menu asked for.
pub enum MenuPick {
    Copy,
    Paste,
}

/// Start, extend or clear a selection from this frame's pointer, and show
/// the right-click menu. Returns a menu pick.
pub fn mouse<T: EventListener>(term: &mut Term<T>, resp: &egui::Response, rect: Rect, cell: (f32, f32)) -> Option<MenuPick> {
    // A press starts a new selection where it lands (an empty one reads as
    // none, so a plain click clears), a drag stretches it, a double-click
    // takes the word.
    let (pressed, origin, at) = resp.ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.press_origin(), i.pointer.interact_pos()));
    if resp.double_clicked() {
        if let Some(pos) = at {
            let (p, side) = cell_at(term, rect, cell, pos);
            term.selection = Some(Selection::new(SelectionType::Semantic, p, side));
        }
    } else if pressed && origin.is_some_and(|o| rect.contains(o)) {
        let (p, side) = cell_at(term, rect, cell, origin.unwrap_or_default());
        term.selection = Some(Selection::new(SelectionType::Simple, p, side));
    } else if resp.dragged_by(egui::PointerButton::Primary) {
        if let Some(pos) = at {
            let (p, side) = cell_at(term, rect, cell, pos);
            if let Some(s) = term.selection.as_mut() {
                s.update(p, side);
            }
        }
    }
    let has = term.selection.as_ref().is_some_and(|s| !s.is_empty());
    let mut pick = None;
    let mut all = false;
    resp.context_menu(|ui| {
        if ui.add_enabled(has, egui::Button::new("Copy")).clicked() {
            pick = Some(MenuPick::Copy);
            ui.close();
        }
        if ui.button("Paste").clicked() {
            pick = Some(MenuPick::Paste);
            ui.close();
        }
        if ui.button("Select all").clicked() {
            all = true;
            ui.close();
        }
    });
    if all {
        let top = Point::new(Line(-(term.history_size() as i32)), Column(0));
        let bottom = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns().saturating_sub(1)));
        let mut s = Selection::new(SelectionType::Simple, top, Side::Left);
        s.update(bottom, Side::Right);
        term.selection = Some(s);
    }
    pick
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term_pane::GridSize;
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    #[test]
    fn selects_what_the_mouse_covers() {
        let mut term = Term::new(Config::default(), &GridSize { rows: 4, cols: 20 }, VoidListener);
        Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new().advance(&mut term, b"hello world\r\nsecond");
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 40.0));
        let (a, _) = cell_at(&term, rect, (10.0, 10.0), Pos2::new(1.0, 1.0));
        let (b, _) = cell_at(&term, rect, (10.0, 10.0), Pos2::new(48.0, 5.0));
        assert_eq!((a.line.0, a.column.0, b.column.0), (0, 0, 4));
        let mut s = Selection::new(SelectionType::Simple, a, Side::Left);
        s.update(b, Side::Right);
        term.selection = Some(s);
        assert_eq!(term.selection_to_string().as_deref(), Some("hello"));
        let (far, _) = cell_at(&term, rect, (10.0, 10.0), Pos2::new(999.0, 999.0));
        assert_eq!((far.line.0, far.column.0), (3, 19), "clamped to the grid");
    }

    /// One frame with these pointer events, the terminal at 0,0 200x40.
    fn frame(ctx: &egui::Context, term: &mut Term<VoidListener>, events: Vec<egui::Event>) {
        let raw = egui::RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 200.0))), ..Default::default() };
        ctx.run_ui(raw, |ui| {
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 40.0));
            let resp = ui.interact(rect, egui::Id::unique("term-test"), egui::Sense::click_and_drag());
            mouse(term, &resp, rect, (10.0, 10.0));
        })
        .drop_without_applying_deltas();
    }

    fn button(x: f32, y: f32, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: Pos2::new(x, y), button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    #[test]
    fn a_mouse_drag_selects_and_a_click_clears() {
        let mut term = Term::new(Config::default(), &GridSize { rows: 4, cols: 20 }, VoidListener);
        Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new().advance(&mut term, b"hello world");
        let ctx = egui::Context::default();
        frame(&ctx, &mut term, vec![egui::Event::PointerMoved(Pos2::new(1.0, 5.0))]);
        frame(&ctx, &mut term, vec![button(1.0, 5.0, true)]);
        for x in [20.0, 35.0, 48.0] {
            frame(&ctx, &mut term, vec![egui::Event::PointerMoved(Pos2::new(x, 5.0))]);
        }
        frame(&ctx, &mut term, vec![button(48.0, 5.0, false)]);
        frame(&ctx, &mut term, vec![]);
        assert_eq!(term.selection_to_string().as_deref(), Some("hello"));
        frame(&ctx, &mut term, vec![button(100.0, 25.0, true)]);
        frame(&ctx, &mut term, vec![button(100.0, 25.0, false)]);
        assert!(term.selection.as_ref().is_none_or(|s| s.is_empty()), "a click clears it");
    }

    #[test]
    fn dropped_paths_are_quoted_when_spaced() {
        let p = [std::path::PathBuf::from("C:/a b/x.png"), std::path::PathBuf::from("D:/y.txt")];
        assert_eq!(drop_text(&p), "\"C:/a b/x.png\" D:/y.txt");
    }
}
