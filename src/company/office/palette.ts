/* The office's colours. Mesh colours from the approved look (style-a.webp), and the
 * text pairs on screen, which the test holds to WCAG AA. */
export const OFFICE = {
  sky: "#191d27", ground: "#23262e", shellOut: "#3a3e4a", shellEdge: "#2d303a", slabTop: "#4c505c",
  wallLit: "#efb877", wallDim: "#6d4632", wallDark: "#475a90", wallNightLow: "#272f54", floorLit: "#8a6e52", floorDark: "#3a4670", grout: "#65493a", groutNight: "#222a4a",
  honey: "#f5b83d", glow: "#ffe2ae", glass: "#9fc3d8", steel: "#6a7180",
  deskTop: "#b07a45", deskBody: "#2f3240", chair: "#262935", screenOn: "#7cc4ff", screenOff: "#1b1e24", lampNeeds: "#f5b83d",
  bead: "#efc48a", plant: "#4f8f4a", plantDeep: "#2f6a4a", pot: "#b5734a", paper: "#f2ece0",
  cork: "#b98d58", noteTodo: "#f2d15e", noteDoing: "#8fc7f2", noteDone: "#8fd9a4",
  grass: "#2f5048", grassLit: "#4a6a4e", stone: "#4a4f5e", kerb: "#3a3e4d", leaf: "#2f5e57", leafDeep: "#1f4347", bark: "#3a2f33",
  skyTop: "#0a1030", skyHigh: "#101a48", skyMid: "#1c2c6a", skyLow: "#34418a", skyGlow: "#a86f7c", haze: "#54508a", moon: "#c4c6d0",
} as const;

/** The app's colours, the same values as tokens.css: the test reads that file, so the
 *  office and the app cannot drift. Honey means it is your turn and nothing else. */
export const APP = {
  ground: "#0F1117", panel: "#151821", raised: "#1C202B", over: "#252A37",
  ink: "#ECEAE4", ink2: "#B9BBC4", ink3: "#9095A3", inkOff: "#6B7080",
  honey: "#F5B83D", honeyInk: "#17130A",
  work: "#6FD6A0", review: "#8FB8F2", stop: "#A7ABB8", danger: "#F2857A",
} as const;

/** A project's colour, for rugs, cushions and art. None of them is honey: that means your turn. */
export const ACCENTS = ["#3f8f8a", "#c4605a", "#7a5fa8", "#5d8a4e", "#4f7fc0", "#b0587f", "#3b6f96", "#a8664a"] as const;
/** Skin, for a toy: warm beads, light to deep. */
export const BEADS = ["#f3cfa0", "#efc48a", "#e0a873", "#c98a5b", "#a8693f"] as const;

export const CHIP_BG = "#14161c";
export const PANEL_BG = APP.panel;
export const TAG_INK = APP.ink;

export const TEXT_PAIRS: Array<[string, string, string, number]> = [
  ["tag text on a chip", TAG_INK, CHIP_BG, 4.5],
  ["tag detail on a chip", APP.ink2, CHIP_BG, 4.5],
  ["needs tag on a chip", OFFICE.honey, CHIP_BG, 4.5],
  ["rail text on the panel", APP.ink, PANEL_BG, 4.5],
  ["rail detail on the panel", APP.ink2, PANEL_BG, 4.5],
  ["breadcrumb on the sky", TAG_INK, OFFICE.sky, 4.5],
  ["bubble mark on honey", "#14161c", OFFICE.honey, 4.5],
  ["focus ring on the sky", APP.ink, OFFICE.sky, 3],
  ["focus halo on a lit wall", APP.ground, OFFICE.wallLit, 3],
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
