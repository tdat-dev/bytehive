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
pub fn tile_to_fit(n: usize, width: f32, height: f32, gap: f32, margin: f32, top: f32, bottom: f32) -> Vec<Tile> {
    let (cols, rows) = grid_dims_for(n.max(1));
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

/// Tiling with the web app's defaults: flush, 10px outer breathing room.
pub fn tidy_tiles(n: usize, width: f32, height: f32) -> Vec<Tile> {
    tile_to_fit(n, width, height, 0.0, 10.0, 10.0, 10.0)
}

/// Density zoom bounds: never smaller than a readable floor, never so big
/// one pane eats the stage.
pub fn clamp_zoom(z: f32) -> f32 {
    z.clamp(0.6, 2.0)
}

/// The header label cut to fit what's left of the chrome on the right.
pub fn fit_header(label: &str, head_width: f32, reserve_right: f32) -> String {
    let max_chars = ((head_width - reserve_right) / 7.0).floor().max(8.0) as usize;
    if label.chars().count() > max_chars {
        format!("{}…", label.chars().take(max_chars.saturating_sub(1)).collect::<String>())
    } else {
        label.to_string()
    }
}

/// One header button's slot, laid from the header's right edge inward.
#[derive(Debug, PartialEq)]
pub struct HeaderButton {
    pub x: f32,
    pub w: f32,
}

/// Everything positioned on the right side of a pane header: the cut label,
/// the badge word slot, and one slot per button from the edge inward.
/// Pure math so tests pin the lockup: labels never wrap into the chrome and
/// buttons never overlap each other.
#[derive(Debug)]
pub struct HeaderLayout {
    pub label: String,
    pub word_x: f32,
    pub buttons: Vec<HeaderButton>,
}

pub fn header_layout(label: &str, head_left: f32, head_width: f32, word_w: f32, btn_ws: &[f32]) -> HeaderLayout {
    let chrome: f32 = btn_ws.iter().sum::<f32>() + 4.0 * btn_ws.len() as f32;
    let label = fit_header(label, head_width, chrome + 78.0);
    let word_x = (head_left + head_width - chrome - 8.0 - word_w).max(head_left + 8.0);
    let mut bx = head_left + head_width;
    let mut buttons = Vec::with_capacity(btn_ws.len());
    for w in btn_ws {
        bx -= w + 4.0;
        buttons.push(HeaderButton { x: bx, w: *w });
    }
    HeaderLayout { label, word_x, buttons }
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
        let tiles = tidy_tiles(4, 1000.0, 800.0);
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
            let tiles = tidy_tiles(n, 1001.0, 733.0);
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
        for t in tidy_tiles(4, 1000.0, 800.0) {
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

    #[test]
    fn header_label_fits_beside_badge_and_buttons() {
        assert_eq!(fit_header("shell 1", 700.0, 340.0), "shell 1");
        let long = "agent-with-a-long-name · feat/branch · abcdef12 · exited (1)";
        let cut = fit_header(long, 700.0, 340.0);
        assert!(cut.ends_with('…') && cut.chars().count() == 51, "{cut}");
        assert_eq!(fit_header(long, 200.0, 340.0).chars().count(), 8);
        assert_eq!(fit_header(long, 900.0, 0.0), long);
    }

    #[test]
    fn header_chrome_never_overlaps() {
        // Real numbers from a staged pane: buttons right-aligned, the word
        // clear of the leftmost button, the label cut to fit what's left.
        let laid = header_layout("shell 1", 0.0, 1035.0, 55.0, &[52.0, 60.0, 68.0]);
        assert_eq!(laid.label, "shell 1");
        let mut right = 1035.0;
        for b in &laid.buttons {
            assert!(b.x + b.w <= right + 0.01, "{laid:?}");
            assert!(b.x >= 0.0, "{laid:?}");
            right = b.x;
        }
        assert!(laid.word_x + 55.0 <= right + 0.01, "{laid:?}");
        assert!(laid.word_x >= 8.0);
        // Narrow header: everything still inside, label shrinks instead.
        let small = header_layout("a very long pane name indeed", 0.0, 200.0, 55.0, &[52.0]);
        assert!(small.buttons.iter().all(|b| b.x >= 0.0), "{small:?}");
        assert_eq!(small.label, "a very l…");
    }
}
