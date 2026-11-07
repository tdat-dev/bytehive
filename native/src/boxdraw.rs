//! Box drawing and block elements, drawn as shapes instead of glyphs, as xterm
//! does (customGlyphs). A font's own versions leave gaps and fall out of the
//! cell grid, which breaks Claude Code's input frame and logo.

use gpui_kit::*;

/// How thick an arm of a line is.
const NONE: u8 = 0;
const LIGHT: u8 = 1;
const HEAVY: u8 = 2;
const DOUBLE: u8 = 3;

/// Arms of a line glyph: left, right, up, down.
fn arms(c: char) -> Option<[u8; 4]> {
    let (n, l, h, d) = (NONE, LIGHT, HEAVY, DOUBLE);
    Some(match c {
        '─' => [l, l, n, n],
        '━' => [h, h, n, n],
        '│' => [n, n, l, l],
        '┃' => [n, n, h, h],
        '┌' | '╭' => [n, l, n, l],
        '┐' | '╮' => [l, n, n, l],
        '└' | '╰' => [n, l, l, n],
        '┘' | '╯' => [l, n, l, n],
        '┏' => [n, h, n, h],
        '┓' => [h, n, n, h],
        '┗' => [n, h, h, n],
        '┛' => [h, n, h, n],
        '├' => [n, l, l, l],
        '┤' => [l, n, l, l],
        '┬' => [l, l, n, l],
        '┴' => [l, l, l, n],
        '┼' => [l, l, l, l],
        '┣' => [n, h, h, h],
        '┫' => [h, n, h, h],
        '┳' => [h, h, n, h],
        '┻' => [h, h, h, n],
        '╋' => [h, h, h, h],
        '═' => [d, d, n, n],
        '║' => [n, n, d, d],
        '╔' => [n, d, n, d],
        '╗' => [d, n, n, d],
        '╚' => [n, d, d, n],
        '╝' => [d, n, d, n],
        '╠' => [n, d, d, d],
        '╣' => [d, n, d, d],
        '╦' => [d, d, n, d],
        '╩' => [d, d, d, n],
        '╬' => [d, d, d, d],
        '╴' => [l, n, n, n],
        '╶' => [n, l, n, n],
        '╵' => [n, n, l, n],
        '╷' => [n, n, n, l],
        _ => return None,
    })
}

/// Rectangles of a block element in cell fractions: x0, y0, x1, y1, opacity.
fn block(c: char) -> Option<Vec<[f32; 5]>> {
    let eighth = |n: u32| n as f32 / 8.0;
    let quads = |tl: bool, tr: bool, bl: bool, br: bool| {
        let mut out = Vec::new();
        for (on, x, y) in [(tl, 0.0, 0.0), (tr, 0.5, 0.0), (bl, 0.0, 0.5), (br, 0.5, 0.5)] {
            if on {
                out.push([x, y, x + 0.5, y + 0.5, 1.0]);
            }
        }
        out
    };
    Some(match c {
        '▀' => vec![[0.0, 0.0, 1.0, 0.5, 1.0]],
        '▁'..='█' => vec![[0.0, 1.0 - eighth(c as u32 - 0x2580), 1.0, 1.0, 1.0]],
        '▉'..='▏' => vec![[0.0, 0.0, eighth(0x2590 - c as u32), 1.0, 1.0]],
        '▐' => vec![[0.5, 0.0, 1.0, 1.0, 1.0]],
        '░' => vec![[0.0, 0.0, 1.0, 1.0, 0.25]],
        '▒' => vec![[0.0, 0.0, 1.0, 1.0, 0.5]],
        '▓' => vec![[0.0, 0.0, 1.0, 1.0, 0.75]],
        '▔' => vec![[0.0, 0.0, 1.0, 0.125, 1.0]],
        '▕' => vec![[0.875, 0.0, 1.0, 1.0, 1.0]],
        '▖' => quads(false, false, true, false),
        '▗' => quads(false, false, false, true),
        '▘' => quads(true, false, false, false),
        '▙' => quads(true, false, true, true),
        '▚' => quads(true, false, false, true),
        '▛' => quads(true, true, true, false),
        '▜' => quads(true, true, false, true),
        '▝' => quads(false, true, false, false),
        '▞' => quads(false, true, true, false),
        '▟' => quads(false, true, true, true),
        _ => return None,
    })
}

/// Whether this character is drawn here rather than by a font.
pub fn is_drawn(c: char) -> bool {
    matches!(c, '\u{2500}'..='\u{259f}') && (arms(c).is_some() || block(c).is_some())
}

fn rounded_corner(c: char) -> Option<Corners<bool>> {
    let mut corners = Corners { top_left: false, top_right: false, bottom_left: false, bottom_right: false };
    match c {
        '╭' => corners.top_left = true,
        '╮' => corners.top_right = true,
        '╰' => corners.bottom_left = true,
        '╯' => corners.bottom_right = true,
        _ => return None,
    }
    Some(corners)
}

/// Paint `c` filling the cell at `origin`.
pub fn paint(c: char, origin: Point<Pixels>, cell: Size<Pixels>, color: Hsla, window: &mut Window) {
    let (w, h) = (f32::from(cell.width), f32::from(cell.height));
    // Cells are a fractional width wide: every edge snaps to a device pixel, so
    // two neighbours share one edge and a line has no seam between them.
    let scale = window.scale_factor();
    let snap = |v: f32| (v * scale).round() / scale;
    let (ox, oy) = (f32::from(origin.x), f32::from(origin.y));
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let (l, t, r, b) = (snap(ox + x0), snap(oy + y0), snap(ox + x1), snap(oy + y1));
        Bounds::new(point(px(l), px(t)), size(px(r - l), px(b - t)))
    };

    if let Some(parts) = block(c) {
        for [x0, y0, x1, y1, alpha] in parts {
            window.paint_quad(fill(rect(x0 * w, y0 * h, x1 * w, y1 * h), color.opacity(alpha)));
        }
        return;
    }
    let Some([left, right, up, down]) = arms(c) else { return };
    let (cx, cy) = ((w / 2.0).floor(), (h / 2.0).floor());
    let stroke = |weight: u8| if weight == HEAVY { 2.0 } else { 1.0 };

    if let Some(corner) = rounded_corner(c) {
        // A quarter ring: a box from the cell's centre to its far edges, with
        // two sides stroked and one corner rounded.
        let (x0, x1) = if corner.top_left || corner.bottom_left { (cx, w) } else { (0.0, cx + 1.0) };
        let (y0, y1) = if corner.top_left || corner.top_right { (cy, h) } else { (0.0, cy + 1.0) };
        let radius = px((w / 2.0).min(h / 2.0));
        let on = |b: bool| if b { radius } else { px(0.0) };
        let edge = |b: bool| if b { px(1.0) } else { px(0.0) };
        window.paint_quad(quad(
            rect(x0, y0, x1, y1),
            Corners { top_left: on(corner.top_left), top_right: on(corner.top_right), bottom_left: on(corner.bottom_left), bottom_right: on(corner.bottom_right) },
            transparent_black(),
            Edges {
                top: edge(corner.top_left || corner.top_right),
                bottom: edge(corner.bottom_left || corner.bottom_right),
                left: edge(corner.top_left || corner.bottom_left),
                right: edge(corner.top_right || corner.bottom_right),
            },
            color,
            BorderStyle::Solid,
        ));
        return;
    }

    let mut bar = |x0: f32, y0: f32, x1: f32, y1: f32| window.paint_quad(fill(rect(x0, y0, x1, y1), color));
    // Double lines run as two strokes each side of the centre.
    let across = |weight: u8| -> Vec<(f32, f32)> {
        match weight {
            NONE => vec![],
            DOUBLE => vec![(-1.0, 0.0), (1.0, 2.0)],
            _ => vec![(0.0, stroke(weight))],
        }
    };
    for (a, b) in across(left) {
        bar(0.0, cy + a, cx + 1.0, cy + b);
    }
    for (a, b) in across(right) {
        bar(cx, cy + a, w, cy + b);
    }
    for (a, b) in across(up) {
        bar(cx + a, 0.0, cx + b, cy + 1.0);
    }
    for (a, b) in across(down) {
        bar(cx + a, cy, cx + b, h);
    }
}
