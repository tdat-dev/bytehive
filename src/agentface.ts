// Each agent's mark: a small pixel face of its own, worked out from its name,
// in its CLI's colour. A letter was not enough: two agents named Ana and Alex
// wore the same "A", and every Claude agent the same orange. The face comes
// from four parts (head, top, eyes, mouth), 192 in all, so agents side by
// side look different at a glance, and the same agent always looks the same.
// Pure: returns markup, no DOM.

import { PERSONA_NAMES } from "./crew";

/** One half of an 8×8 face, left to right (the other half mirrors it): each
 *  string is a row of 4 cells, "#" the head, "o" a dark cell (eyes, mouth),
 *  "." empty. Parts are laid over each other in order. */
type Half = string[];

const HEADS: Half[] = [
  // round
  ["....", "....", ".###", "####", "####", "####", "####", ".###"],
  // square
  ["....", "....", "####", "####", "####", "####", "####", "####"],
  // tall
  ["....", ".###", ".###", ".###", ".###", ".###", ".###", ".###"],
];

const TOPS: Half[] = [
  // antenna
  ["...#", "...#", "....", "....", "....", "....", "....", "...."],
  // ears
  ["....", "#...", "#...", "....", "....", "....", "....", "...."],
  // hair tuft
  ["..##", ".###", "....", "....", "....", "....", "....", "...."],
  // nothing on top
  ["....", "....", "....", "....", "....", "....", "....", "...."],
];

const EYES: Half[] = [
  // dots
  ["....", "....", "....", "....", ".o..", "....", "....", "...."],
  // wide
  ["....", "....", "....", "....", ".oo.", "....", "....", "...."],
  // tall
  ["....", "....", "....", ".o..", ".o..", "....", "....", "...."],
  // visor
  ["....", "....", "....", "....", ".ooo", "....", "....", "...."],
];

const MOUTHS: Half[] = [
  // small
  ["....", "....", "....", "....", "....", "....", "...o", "...."],
  // wide
  ["....", "....", "....", "....", "....", "....", "..oo", "...."],
  // smile
  ["....", "....", "....", "....", "....", "..o.", "...o", "...."],
  // none
  ["....", "....", "....", "....", "....", "....", "....", "...."],
];

/** FNV-1a: a stable number for a string. */
function hash(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}

export interface FaceParts { head: number; top: number; eyes: number; mouth: number }

/** Which parts a name gets. The same name always gets the same face. ByteHive's
 *  own names (Ana, Bob, …) each get a face of their own, spread so neighbours
 *  differ in most parts; any other name gets one from its hash. */
export function faceParts(seed: string): FaceParts {
  const all = HEADS.length * TOPS.length * EYES.length * MOUTHS.length;
  const k = PERSONA_NAMES.findIndex((n) => n.toLowerCase() === seed.trim().toLowerCase());
  // 97 shares no factor with 192: stepping by it visits every face once
  let h = k >= 0 ? (k * 97) % all : hash(seed.trim().toLowerCase() || "?");
  const pick = (n: number) => { const v = h % n; h = Math.floor(h / n); return v; };
  return { head: pick(HEADS.length), top: pick(TOPS.length), eyes: pick(EYES.length), mouth: pick(MOUTHS.length) };
}

/** The face as an 8×8 grid of cells: "#" head, "o" dark, "." empty. */
export function faceGrid(p: FaceParts): string[] {
  const rows: string[][] = Array.from({ length: 8 }, () => Array(8).fill("."));
  const lay = (half: Half, only?: (c: string) => boolean) => {
    half.forEach((row, y) => [...row].forEach((c, x) => {
      if (c === "." || (only && !only(rows[y][x]))) return;
      rows[y][x] = c;
      rows[y][7 - x] = c;
    }));
  };
  lay(HEADS[p.head]);
  lay(TOPS[p.top]);
  // eyes and mouth only sit on the head
  lay(EYES[p.eyes], (under) => under === "#");
  lay(MOUTHS[p.mouth], (under) => under === "#");
  return rows.map((r) => r.join(""));
}

/** The face as SVG: the head in `color`, eyes and mouth dark. Runs of the same
 *  cell in a row are one rect, so a face is a handful of shapes. */
export function faceSvg(seed: string, color: string): string {
  const grid = faceGrid(faceParts(seed));
  const rects: string[] = [];
  grid.forEach((row, y) => {
    let x = 0;
    while (x < 8) {
      const c = row[x];
      let w = 1;
      while (x + w < 8 && row[x + w] === c) w++;
      if (c !== ".") rects.push(`<rect x="${x}" y="${y}" width="${w}" height="1"${c === "o" ? ` class="fo"` : ""}/>`);
      x += w;
    }
  });
  return `<svg class="face" viewBox="0 0 8 8" shape-rendering="crispEdges" fill="${color.replace(/[^#\w(),.%\s-]/g, "")}" aria-hidden="true">${rects.join("")}</svg>`;
}
