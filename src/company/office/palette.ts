/* The office's colours. Mesh colours from the approved look (style-a.webp), and the
 * text pairs on screen, which the test holds to WCAG AA. */
export const OFFICE = {
  sky: "#191d27", ground: "#23262e", shellOut: "#34363c", shellEdge: "#2a2c31", slabTop: "#3f3d3c",
  wallLit: "#b57f3a", wallDim: "#5b4630", wallDark: "#2b2a2c", floorLit: "#7a6248", floorDark: "#35322f",
  honey: "#f5b83d", glow: "#ffd98a", glass: "#8fb3c9", steel: "#5a6068",
  deskTop: "#9a6a3c", deskBody: "#2f3138", chair: "#24262c", screenOn: "#7cc4ff", screenOff: "#1b1e24", lampNeeds: "#f5b83d",
  bead: "#efc48a", plant: "#4f8f4a", pot: "#a8703c", paper: "#f2ece0",
  cork: "#b98d58", noteTodo: "#f2d15e", noteDoing: "#8fc7f2", noteDone: "#8fd9a4",
} as const;

export const CHIP_BG = "#14161c";
export const PANEL_BG = "#15161a";
export const TAG_INK = "#eceae4";

export const TEXT_PAIRS: Array<[string, string, string, number]> = [
  ["tag text on a chip", TAG_INK, CHIP_BG, 4.5],
  ["tag detail on a chip", "#b4b4bc", CHIP_BG, 4.5],
  ["needs tag on a chip", OFFICE.honey, CHIP_BG, 4.5],
  ["rail text on the panel", "#f4f4f5", PANEL_BG, 4.5],
  ["rail detail on the panel", "#b4b4bc", PANEL_BG, 4.5],
  ["breadcrumb on the sky", TAG_INK, OFFICE.sky, 4.5],
  ["bubble mark on honey", "#14161c", OFFICE.honey, 4.5],
  ["focus ring on the sky", OFFICE.honey, OFFICE.sky, 3],
];

const channel = (v: number) => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
const luminance = (hex: string) => {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * channel(n >> 16) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
};

export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
