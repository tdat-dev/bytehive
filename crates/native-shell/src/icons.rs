//! The shell's line icons, drawn as strokes (no image files): each is laid
//! out on a 24-unit square, like the web app's SVGs, and scaled into `at`.

use std::f32::consts::{PI, TAU};

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    House,
    Refresh,
    Search,
    Focus,
    Grid,
    /// A window with a panel down its left side: show or hide the side list.
    Sidebar,
    History,
    Gear,
    Book,
    Folder,
    Pencil,
    Dots,
    Close,
    /// A filled square: stop a running agent.
    Stop,
    Chevron { open: bool },
}

struct Pen<'a> {
    p: &'a Painter,
    at: Rect,
    s: Stroke,
}

impl Pen<'_> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.at.min + Vec2::new(x, y) * (self.at.width() / 24.0)
    }
    fn line(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::line(pts, self.s));
    }
    fn closed(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::closed_line(pts, self.s));
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.p.circle_stroke(self.pt(x, y), r * self.at.width() / 24.0, self.s);
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.p.circle_filled(self.pt(x, y), r * self.at.width() / 24.0, self.s.color);
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let k = self.at.width() / 24.0;
        let rect = Rect::from_min_size(self.pt(x, y), Vec2::new(w, h) * k);
        self.p.rect_stroke(rect, r * k, self.s, eframe::egui::StrokeKind::Middle);
    }
    /// An arc around (cx, cy), from angle a to b (radians, clockwise from +x).
    fn arc(&self, cx: f32, cy: f32, r: f32, a: f32, b: f32) -> Vec<(f32, f32)> {
        let n = 24;
        (0..=n).map(|i| a + (b - a) * i as f32 / n as f32).map(|t| (cx + r * t.cos(), cy + r * t.sin())).collect()
    }
}

/// Draw `icon` in `at` (a square) with `width` stroke at 24 units.
pub fn paint(p: &Painter, at: Rect, icon: Icon, color: Color32, width: f32) {
    let pen = Pen { p, at, s: Stroke::new(width * at.width() / 24.0 * 1.15, color) };
    match icon {
        Icon::House => {
            pen.line(&[(3.0, 10.5), (12.0, 3.0), (21.0, 10.5)]);
            pen.line(&[(5.5, 9.2), (5.5, 21.0), (10.0, 21.0), (10.0, 15.5), (14.0, 15.5), (14.0, 21.0), (18.5, 21.0), (18.5, 9.2)]);
        }
        Icon::Refresh | Icon::History => {
            // Round from the left, under and over the top, back to the left.
            let mut arc = pen.arc(12.0, 12.0, 9.0, PI, -0.75 * PI);
            arc.push((3.0, 8.0));
            pen.line(&arc);
            pen.line(&[(3.0, 3.0), (3.0, 8.0), (8.0, 8.0)]);
            if icon == Icon::History {
                pen.line(&[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)]);
            }
        }
        Icon::Search => {
            pen.circle(10.5, 10.5, 7.0);
            pen.line(&[(15.8, 15.8), (21.0, 21.0)]);
        }
        Icon::Focus => {
            pen.rect(3.0, 4.0, 18.0, 16.0, 3.0);
            pen.line(&[(7.5, 10.0), (16.5, 10.0)]);
            pen.line(&[(7.5, 14.0), (13.5, 14.0)]);
        }
        Icon::Grid => {
            for (x, y) in [(3.5, 3.5), (13.5, 3.5), (3.5, 13.5), (13.5, 13.5)] {
                pen.rect(x, y, 7.0, 7.0, 1.6);
            }
        }
        Icon::Sidebar => {
            pen.rect(3.0, 4.5, 18.0, 15.0, 2.5);
            pen.line(&[(9.5, 4.5), (9.5, 19.5)]);
        }
        Icon::Gear => {
            pen.circle(12.0, 12.0, 3.2);
            pen.circle(12.0, 12.0, 7.0);
            for i in 0..8 {
                let t = i as f32 * TAU / 8.0;
                pen.line(&[(12.0 + 7.0 * t.cos(), 12.0 + 7.0 * t.sin()), (12.0 + 9.6 * t.cos(), 12.0 + 9.6 * t.sin())]);
            }
        }
        Icon::Book => {
            pen.closed(&[(5.0, 4.5), (17.5, 4.5), (19.0, 6.0), (19.0, 20.5), (6.5, 20.5), (5.0, 19.0)]);
            pen.line(&[(9.0, 4.5), (9.0, 20.5)]);
        }
        Icon::Folder => {
            pen.closed(&[(2.5, 6.0), (3.5, 5.0), (9.0, 5.0), (11.0, 7.0), (20.5, 7.0), (21.5, 8.0), (21.5, 18.5), (20.5, 19.5), (3.5, 19.5), (2.5, 18.5)]);
        }
        Icon::Pencil => {
            pen.closed(&[(16.0, 3.5), (20.5, 8.0), (8.5, 20.0), (4.0, 20.0), (4.0, 15.5)]);
            pen.line(&[(13.5, 6.0), (18.0, 10.5)]);
        }
        Icon::Dots => {
            for x in [5.5, 12.0, 18.5] {
                pen.dot(x, 12.0, 1.5);
            }
        }
        Icon::Close => {
            pen.line(&[(6.0, 6.0), (18.0, 18.0)]);
            pen.line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::Stop => {
            let k = at.width() / 24.0;
            p.rect_filled(Rect::from_min_size(pen.pt(6.5, 6.5), Vec2::splat(11.0 * k)), 2.5 * k, color);
        }
        Icon::Chevron { open: false } => pen.line(&[(8.5, 6.0), (14.5, 12.0), (8.5, 18.0)]),
        Icon::Chevron { open: true } => pen.line(&[(6.0, 8.5), (12.0, 14.5), (18.0, 8.5)]),
    }
}

/// An icon centered in `around`, `size` pixels square.
pub fn paint_in(p: &Painter, around: Rect, size: f32, icon: Icon, color: Color32, width: f32) {
    paint(p, Rect::from_center_size(around.center(), Vec2::splat(size)), icon, color, width);
}
