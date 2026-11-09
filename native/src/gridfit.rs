//! How the Grid lays itself out so everything on it stays worth reading
//! (src/gridfit.ts).
//!
//! A terminal only reads at about 80 columns: below that Claude Code's TUI
//! wraps every line into mush, and a bigger font only makes it wrap more. So a
//! screen holds two to four readable terminals at most, never the six or nine
//! agents someone runs. The Grid therefore shows ONE real terminal, the agent
//! you work with, as big as it can be, and every other agent as a card beside
//! it: plain text that wraps at any width (see gridcards.rs). A wide screen
//! gets two columns of cards, as long as the terminal keeps plenty of room.
//!
//! Pure: estimates cells from the font instead of measuring; the real fit (and
//! the PTY size) still comes from the terminal once it is placed.

/// Below this a CLI's TUI wraps into mush, or shows little but its input box.
pub const READ_COLS: u32 = 80;
pub const READ_ROWS: u32 = 20;
/// The terminal grows its font while a line still holds this many cells.
pub const AIM_COLS: u32 = 90;
pub const AIM_ROWS: u32 = 20;
/// How far past the setting the font may grow.
pub const GROW: f32 = 4.0;
pub const GAP: f32 = 16.0;
/// A column of cards, and the gap between two of them.
pub const CARD_W: f32 = 340.0;
pub const CARD_GAP: f32 = 12.0;
/// A second column of cards only while the terminal keeps this many columns.
pub const TWO_COLS_KEEP: u32 = 110;
/// What a pane spends around its terminal: the header, the padding and the
/// scrollbar (measured in today's app: a 784 x 456 pane has a 739 x 378 screen).
const CHROME_W: f32 = 46.0;
const CHROME_H: f32 = 80.0;
/// Geist Mono's cell is 0.6em wide; a row is 1.5em.
const CELL_W: f32 = 0.6;
const CELL_H: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailPlan {
    /// Where the terminal goes.
    pub big: Tile,
    /// Where the cards go; None with no cards.
    pub rail: Option<Tile>,
    /// Columns of cards.
    pub cols: u32,
    pub font: f32,
    /// The terminal reaches READ_COLS x READ_ROWS at the setting's font.
    pub readable: bool,
}

/// Roughly the terminal a pane of this size holds at this font: (cols, rows).
pub fn cells_at(tile: Tile, font: f32) -> (u32, u32) {
    let cols = ((tile.w - CHROME_W) / (font * CELL_W)).floor().max(0.0) as u32;
    let rows = ((tile.h - CHROME_H) / (font * CELL_H)).floor().max(0.0) as u32;
    (cols, rows)
}

/// The setting, grown while the pane still holds AIM_COLS x AIM_ROWS.
pub fn grow_font(base: f32, tile: Tile) -> f32 {
    let fits = |n: f32| {
        let (cols, rows) = cells_at(tile, n);
        cols >= AIM_COLS && rows >= AIM_ROWS
    };
    let mut f = base;
    while f < base + GROW && fits(f + 1.0) {
        f += 1.0;
    }
    f
}

/// The terminal on the left, `cards` cards on the right in one column or two.
pub fn rail_plan(cards: usize, area: Area, base: f32) -> RailPlan {
    let full = Tile { x: 0.0, y: 0.0, w: area.width.round(), h: area.height.round() };
    if cards == 0 {
        return RailPlan { big: full, rail: None, cols: 0, font: grow_font(base, full), readable: cells_at(full, base).0 >= READ_COLS };
    }
    let rail_w = |cols: u32| cols as f32 * CARD_W + (cols as f32 - 1.0) * CARD_GAP;
    let big_for = |cols: u32| Tile { w: (full.w - rail_w(cols) - GAP).max(0.0), ..full };
    // two columns only when one would not show them all and the terminal keeps plenty
    let cols = if cards > 3 && cells_at(big_for(2), base).0 >= TWO_COLS_KEEP { 2 } else { 1 };
    let big = big_for(cols);
    let rail = Tile { x: big.w + GAP, y: 0.0, w: rail_w(cols), h: full.h };
    let (c, r) = cells_at(big, base);
    RailPlan { big, rail: Some(rail), cols, font: grow_font(base, big), readable: c >= READ_COLS && r >= READ_ROWS }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: f32, height: f32) -> Area {
        Area { width, height }
    }

    #[test]
    fn a_laptops_terminal_reads_beside_one_column_of_cards() {
        let p = rail_plan(5, area(1200.0, 712.0), 15.0);
        assert_eq!(p.cols, 1);
        assert_eq!(p.big.w, 1200.0 - CARD_W - GAP);
        assert_eq!(p.rail, Some(Tile { x: p.big.w + GAP, y: 0.0, w: CARD_W, h: 712.0 }));
        assert!(cells_at(p.big, 15.0).0 >= READ_COLS);
        assert!(p.readable);
    }

    #[test]
    fn the_font_grows_on_a_1080p_screen() {
        let p = rail_plan(5, area(1584.0, 928.0), 15.0);
        assert!(p.font > 15.0);
        assert!(cells_at(p.big, p.font).0 >= 90);
    }

    #[test]
    fn two_columns_of_cards_on_a_27_inch_screen() {
        let p = rail_plan(7, area(2224.0, 1288.0), 15.0);
        assert_eq!(p.cols, 2);
        assert_eq!(p.rail.unwrap().w, 2.0 * CARD_W + 12.0);
        assert_eq!(p.font, 19.0);
    }

    #[test]
    fn one_column_for_a_few_cards() {
        assert_eq!(rail_plan(3, area(2224.0, 1288.0), 15.0).cols, 1);
    }

    #[test]
    fn too_narrow_beside_the_cards() {
        assert!(!rail_plan(4, area(944.0, 700.0), 15.0).readable);
    }

    #[test]
    fn no_cards_gives_the_terminal_everything() {
        let p = rail_plan(0, area(1000.0, 600.0), 15.0);
        assert_eq!(p.big, Tile { x: 0.0, y: 0.0, w: 1000.0, h: 600.0 });
        assert_eq!(p.rail, None);
    }
}
