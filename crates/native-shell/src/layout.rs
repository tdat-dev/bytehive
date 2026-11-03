//! Pure grid geometry for the pane canvas, ported 1:1 from the web app's
//! `src/canvas.ts`: squarish grids, gap-0 flush tiling with a stretched
//! last tile, and row-major free-slot packing. No egui here so the math
//! stays unit-testable; `app.rs` only maps tiles onto its rect.

/// A tile rect in canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Squarish grid: 2→2x1 (big side by side), 4→2x2, 6→3x2, …
#[cfg_attr(not(test), allow(dead_code))]
pub fn grid_dims_for(n: usize) -> (usize, usize) {
    let cols = (n as f32).sqrt().ceil().max(1.0) as usize;
    let rows = n.div_ceil(cols).max(1);
    (cols, rows)
}

/// Tile n panes to fill `area` (minus margins). A lone tile ending a short
/// last row stretches to the right.
///
/// The default gap is 0: tiles butt against each other and each tile's size
/// is derived from rounded edges rather than a rounded width — rounding x
/// and w independently drifts them apart, and at gap 0 a half-pixel of
/// drift is a visible slit between two panes.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tile_to_fit(n: usize, width: f32, height: f32, gap: f32, margin: f32, top: f32, bottom: f32) -> Vec<Tile> {
    tile_in(n, grid_dims_for(n.max(1)), width, height, gap, margin, top, bottom)
}

/// A grid where every terminal stays readable: as many columns as keep
/// each tile `min_cols` glyphs wide, and every tile at least `min_rows`
/// glyphs tall plus its `head`. (cols, rows, tile height); when the tiles
/// outgrow `height` the Grid scrolls rather than squeeze them.
pub fn readable_grid(n: usize, width: f32, height: f32, gap: f32, head: f32, cell: (f32, f32), min_cols: f32, min_rows: f32) -> (usize, usize, f32) {
    let n = n.max(1);
    let tw = |c: usize| (width - (c as f32 - 1.0) * gap) / c as f32;
    let most = (1..=n).rev().find(|&c| tw(c) >= min_cols * cell.0).unwrap_or(1);
    // As few rows as the widest fit allows, then as few columns as those
    // rows need: four on a wide screen are 2 x 2, not 3 + 1.
    let rows = n.div_ceil(most);
    let cols = n.div_ceil(rows);
    let fit = (height - (rows as f32 - 1.0) * gap) / rows as f32;
    (cols, rows, fit.max(min_rows * cell.1 + head))
}

#[allow(clippy::too_many_arguments)]
pub fn tile_in(n: usize, (cols, rows): (usize, usize), width: f32, height: f32, gap: f32, margin: f32, top: f32, bottom: f32) -> Vec<Tile> {
    let tw = (width - 2.0 * margin - (cols as f32 - 1.0) * gap) / cols as f32;
    let th = (height - top - bottom - (rows as f32 - 1.0) * gap) / rows as f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let c = i % cols;
        let r = i / cols;
        let left = margin + c as f32 * (tw + gap);
        let top_edge = top + r as f32 * (th + gap);
        let in_row = cols.min(n - r * cols);
        let right = if i == n - 1 && in_row < cols { width - margin } else { left + tw };
        let x = left.round();
        let y = top_edge.round();
        out.push(Tile { x, y, w: (right.round() - x).max(0.0), h: ((top_edge + th).round() - y).max(0.0) });
    }
    out
}

/// Density zoom bounds: never smaller than a readable floor, never so big
/// one pane eats the stage.
pub fn clamp_zoom(z: f32) -> f32 {
    z.clamp(0.6, 2.0)
}

/// First row-major grid position not colliding with `existing` top-lefts.
/// Reserved for free-drag pane placement (Native 2 follow-up).
#[allow(dead_code)]
pub fn next_slot(existing: &[(f32, f32)], cell_w: f32, cell_h: f32, gap: f32, area_w: f32) -> (f32, f32) {
    let step_x = cell_w + gap;
    let step_y = cell_h + gap;
    let cols = ((area_w + gap) / step_x).floor().max(1.0) as usize;
    for i in 0..4096 {
        let x = (i % cols) as f32 * step_x;
        let y = (i / cols) as f32 * step_y;
        let taken = existing.iter().any(|(ex, ey)| ex.round() as i32 == x as i32 && ey.round() as i32 == y as i32);
        if !taken {
            return (x, y);
        }
    }
    (0.0, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_never_shrink_below_readable() {
        let cell = (8.4, 21.0);
        // Five on a portrait 1080 x 1800: one column, each ~24 rows tall, so
        // the Grid scrolls instead of leaving 15 rows a tile.
        let (c, r, h) = readable_grid(5, 1080.0, 1800.0, 4.0, 42.0, cell, 72.0, 24.0);
        assert_eq!((c, r), (1, 5));
        assert!(h >= 24.0 * 21.0 + 42.0);
        // Two on a wide screen: side by side, filling the height.
        let (c, r, h) = readable_grid(2, 1900.0, 1000.0, 4.0, 42.0, cell, 72.0, 24.0);
        assert_eq!((c, r), (2, 1));
        assert_eq!(h, 1000.0);
        // Four on 1900 x 1000: two by two, not three and one.
        assert_eq!(readable_grid(4, 1900.0, 1000.0, 4.0, 42.0, cell, 72.0, 24.0).0, 2);
        // Five there: three and two.
        assert_eq!(readable_grid(5, 1900.0, 1000.0, 4.0, 42.0, cell, 72.0, 24.0).0, 3);
    }

    #[test]
    fn grid_keeps_2_side_by_side_4_in_2x2() {
        assert_eq!(grid_dims_for(1), (1, 1));
        assert_eq!(grid_dims_for(2), (2, 1));
        assert_eq!(grid_dims_for(4), (2, 2));
        assert_eq!(grid_dims_for(6), (3, 2));
    }

    #[test]
    fn tiles_stay_inside_with_margins() {
        for t in tile_to_fit(4, 1000.0, 800.0, 10.0, 10.0, 10.0, 80.0) {
            assert!(t.x >= 10.0, "{t:?}");
            assert!(t.y >= 10.0, "{t:?}");
            assert!(t.x + t.w <= 1000.0 - 10.0 + 0.5, "{t:?}");
            assert!(t.y + t.h <= 800.0 - 80.0 + 0.5, "{t:?}");
        }
    }

    #[test]
    fn no_double_reserve_at_the_bottom_with_defaults() {
        let tiles = tile_to_fit(4, 1000.0, 800.0, 0.0, 10.0, 10.0, 10.0);
        let bottom = tiles.iter().map(|t| t.y + t.h).fold(0.0_f32, f32::max);
        assert!(800.0 - bottom <= 20.0, "bottom-most edge leaves too much room");
    }

    #[test]
    fn lone_last_tile_stretches_to_fill_its_row() {
        let tiles = tile_to_fit(3, 1000.0, 800.0, 10.0, 10.0, 10.0, 10.0);
        assert!(tiles[2].w > tiles[0].w + 1.0, "{tiles:?}");
    }

    #[test]
    fn neighbours_share_an_edge_exactly_at_gap_zero() {
        for n in [2, 4, 6, 9] {
            let tiles = tile_to_fit(n, 1001.0, 733.0, 0.0, 10.0, 10.0, 10.0);
            let (cols, _) = grid_dims_for(n);
            for i in 0..n {
                if i % cols != cols - 1 && i + 1 < n {
                    assert_eq!(tiles[i].x + tiles[i].w, tiles[i + 1].x, "n={n} tile {i}");
                }
                if i + cols < n {
                    assert_eq!(tiles[i].y + tiles[i].h, tiles[i + cols].y, "n={n} tile {i}");
                }
            }
        }
    }

    #[test]
    fn cluster_stays_inside_at_default_margin() {
        for t in tile_to_fit(4, 1000.0, 800.0, 0.0, 10.0, 10.0, 10.0) {
            assert!(t.x >= 10.0 && t.x + t.w <= 1000.0 - 10.0, "{t:?}");
            assert!(t.y + t.h <= 800.0 - 10.0, "{t:?}");
        }
    }

    #[test]
    fn slots_pack_row_major_and_skip_taken_cells() {
        assert_eq!(next_slot(&[], 300.0, 200.0, 12.0, 1000.0), (0.0, 0.0));
        assert_eq!(next_slot(&[(0.0, 0.0)], 300.0, 200.0, 12.0, 1000.0), (312.0, 0.0));
    }

    #[test]
    fn zoom_clamps_to_sane_bounds() {
        assert_eq!(clamp_zoom(1.0), 1.0);
        assert_eq!(clamp_zoom(5.0), 2.0);
        assert_eq!(clamp_zoom(0.1), 0.6);
    }
}
