// How the Grid lays itself out so everything on it stays worth reading.
//
// A terminal only reads at about 80 columns: below that Claude Code's TUI wraps
// every line into mush, and a bigger font only makes it wrap more. So a screen
// holds two to four readable terminals at most, never the six or nine agents
// someone runs. The Grid therefore shows ONE real terminal, the agent you work
// with, as big as it can be, and every other agent as a card beside it: plain
// text that wraps at any width (see gridcards.ts). A wide screen gets two
// columns of cards, as long as the terminal keeps plenty of room.
//
// Pure: estimates cells from the font instead of measuring; the real fit (and
// the PTY size) still comes from xterm once the terminal is placed.

import type { Area, Tile } from "./canvas";

/** Below this a CLI's TUI wraps into mush, or shows little but its input box. */
export const READ_COLS = 80;
export const READ_ROWS = 20;
/** The terminal grows its font while a line still holds this many cells. */
export const AIM_COLS = 90;
export const AIM_ROWS = 20;
/** How far past the setting the font may grow. */
export const GROW = 4;
export const GAP = 16;
/** A column of cards, and the gap between two of them. */
export const CARD_W = 340;
export const CARD_GAP = 12;
/** A second column of cards only while the terminal keeps this many columns. */
export const TWO_COLS_KEEP = 110;
/** What a pane spends around its terminal: the header, the padding and the
 *  scrollbar (measured in the app: a 784 x 456 pane has a 739 x 378 screen). */
const CHROME_W = 46;
const CHROME_H = 80;
/** Geist Mono's cell is 0.6em wide; a row is 1.5em (the font's own line box
 *  times xterm's lineHeight 1.2: 21px at 14px). */
const CELL_W = 0.6;
const CELL_H = 1.5;

export interface RailPlan {
  /** Where the terminal goes. */
  big: Tile;
  /** Where the cards go; null with no cards. */
  rail: Tile | null;
  /** Columns of cards. */
  cols: number;
  font: number;
  /** The terminal reaches READ_COLS × READ_ROWS at the setting's font. */
  readable: boolean;
}

/** Roughly the terminal a pane of this size holds at this font. */
export function cellsAt(tile: { w: number; h: number }, font: number): { cols: number; rows: number } {
  return {
    cols: Math.max(0, Math.floor((tile.w - CHROME_W) / (font * CELL_W))),
    rows: Math.max(0, Math.floor((tile.h - CHROME_H) / (font * CELL_H))),
  };
}

/** The setting, grown while the pane still holds AIM_COLS × AIM_ROWS. */
export function growFont(base: number, tile: { w: number; h: number }): number {
  let f = base;
  const fits = (n: number) => { const c = cellsAt(tile, n); return c.cols >= AIM_COLS && c.rows >= AIM_ROWS; };
  while (f < base + GROW && fits(f + 1)) f++;
  return f;
}

/** The terminal on the left, `cards` cards on the right in one column or two. */
export function railPlan(cards: number, area: Area, base: number): RailPlan {
  const full = { x: 0, y: 0, w: Math.round(area.width), h: Math.round(area.height) };
  if (cards <= 0) return { big: full, rail: null, cols: 0, font: growFont(base, full), readable: cellsAt(full, base).cols >= READ_COLS };
  const railW = (cols: number) => cols * CARD_W + (cols - 1) * CARD_GAP;
  const bigFor = (cols: number) => ({ ...full, w: Math.max(0, full.w - railW(cols) - GAP) });
  // two columns only when one would not show them all and the terminal keeps plenty
  const cols = cards > 3 && cellsAt(bigFor(2), base).cols >= TWO_COLS_KEEP ? 2 : 1;
  const big = bigFor(cols);
  const rail = { x: big.w + GAP, y: 0, w: railW(cols), h: full.h };
  const c = cellsAt(big, base);
  return { big, rail, cols, font: growFont(base, big), readable: c.cols >= READ_COLS && c.rows >= READ_ROWS };
}
