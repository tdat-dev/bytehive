// How the Grid lays its terminals out so each one stays worth reading.
//
// Splitting the stage evenly by a fixed count (six agents, 3 × 2) leaves each
// card ~40 columns on a laptop and ~75 on a 27" screen: Claude Code's TUI wraps
// every line into mush, and a bigger font only makes it wrap more. So the Grid
// asks the other way round: how many cards fit at a size a CLI can lay out in,
// and which arrangement of them. The ones that don't fit wait in the list. A
// card with room to spare gets a bigger font instead of more empty columns.
//
// Pure: estimates cells from the font instead of measuring, the real fit (and
// the PTY size) still comes from xterm once the cards are placed.

import type { Area, Tile } from "./canvas";

/** Below this a CLI's TUI wraps into mush, or shows little but its input box. */
export const READ_COLS = 80;
export const READ_ROWS = 20;
/** A card with room grows its font while a line still holds this many cells. */
export const AIM_COLS = 90;
export const AIM_ROWS = 24;
/** How far past the setting the font may grow. */
export const GROW = 4;
export const GAP = 16;
/** What a card spends around its terminal: the header, the padding and the scrollbar. */
const CHROME_W = 42;
const CHROME_H = 70;
/** Geist Mono's cell is 0.6em wide; xterm draws lines at 1.2. */
const CELL_W = 0.6;
const CELL_H = 1.2;

export interface GridPlan {
  /** How many cards are on screen (the first `count` of the ones asked for). */
  count: number;
  font: number;
  tiles: Tile[];
  /** Every card reaches READ_COLS × READ_ROWS. */
  readable: boolean;
}

/** Roughly the terminal a card of this size holds at this font. */
export function cellsAt(tile: { w: number; h: number }, font: number): { cols: number; rows: number } {
  return {
    cols: Math.max(0, Math.floor((tile.w - CHROME_W) / (font * CELL_W))),
    rows: Math.max(0, Math.floor((tile.h - CHROME_H) / (font * CELL_H))),
  };
}

/** `n` cards in rows of `cols`, each row sharing the full width, so a short
 *  last row has wider cards instead of a hole. */
export function gridTiles(n: number, cols: number, area: Area, gap = GAP): Tile[] {
  if (n <= 0) return [];
  const across = Math.max(1, Math.min(cols, n));
  const rows = Math.ceil(n / across);
  const th = (area.height - (rows - 1) * gap) / rows;
  const out: Tile[] = [];
  for (let r = 0; r < rows; r++) {
    const inRow = Math.min(across, n - r * across);
    const tw = (area.width - (inRow - 1) * gap) / inRow;
    const top = r * (th + gap);
    for (let c = 0; c < inRow; c++) {
      const left = c * (tw + gap);
      const x = Math.round(left), y = Math.round(top);
      out.push({ x, y, w: Math.round(left + tw) - x, h: Math.round(top + th) - y });
    }
  }
  return out;
}

/** The smallest card of a layout, width and height each. */
function worst(tiles: Tile[]): { w: number; h: number } {
  return { w: Math.min(...tiles.map((t) => t.w)), h: Math.min(...tiles.map((t) => t.h)) };
}

/** How close the smallest card comes to a size, 1 = reaches it. */
function reach(tiles: Tile[], font: number, cols: number, rows: number): number {
  const c = cellsAt(worst(tiles), font);
  return Math.min(c.cols / cols, c.rows / rows);
}

/** The setting, grown while every card still holds AIM_COLS × AIM_ROWS. */
export function growFont(base: number, tiles: Tile[]): number {
  let f = base;
  while (f < base + GROW && reach(tiles, f + 1, AIM_COLS, AIM_ROWS) >= 1) f++;
  return f;
}

/**
 * The most of `want` cards that fit readably at font `base`, in the
 * arrangement that leaves them the most room. Two at the least: when not even
 * two fit, the least cramped way to show two.
 */
export function gridPlan(want: number, area: Area, base: number): GridPlan {
  if (want < 2) {
    const tiles = gridTiles(want, 1, area);
    return { count: tiles.length, font: base, tiles, readable: true };
  }
  for (let k = want; k >= 2; k--) {
    let best: Tile[] | null = null;
    let bestReach = 0;
    for (let cols = 1; cols <= k; cols++) {
      const tiles = gridTiles(k, cols, area);
      const r = reach(tiles, base, AIM_COLS, AIM_ROWS);
      if (reach(tiles, base, READ_COLS, READ_ROWS) >= 1 && r > bestReach) { best = tiles; bestReach = r; }
    }
    if (best) return { count: k, font: growFont(base, best), tiles: best, readable: true };
  }
  const side = gridTiles(2, 2, area);
  const stacked = gridTiles(2, 1, area);
  // a terminal reads in lines: a tie goes to the taller cards
  const tiles = reach(stacked, base, READ_COLS, READ_ROWS) > reach(side, base, READ_COLS, READ_ROWS) + 0.05 ? stacked : side;
  return { count: 2, font: base, tiles, readable: false };
}
