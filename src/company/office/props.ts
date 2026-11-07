/* What makes a floor somebody's room: rugs, shelves, a sofa corner, a coffee counter,
 * plants, things on the walls, and lamps. Everything is plain geometry with its colours
 * on the vertices, so a floor of it is still one draw call. A floor's name picks its
 * colour and its room, so no two floors match and a floor keeps its look. */
import {
  BufferGeometry, CircleGeometry, Color, ConeGeometry, CylinderGeometry, Matrix4, PlaneGeometry, SphereGeometry, TorusGeometry,
} from "three";
import { box, flare, hashOf, moved, paint, pool, ramp, seeded, soft, tint } from "./geometry";
import { ROOM_D, ROOM_W, SLAB, STOREY } from "./layout";
import { rgb, type Blocker, type Lamp, type RoomLight } from "./light";
import { ACCENTS, OFFICE } from "./palette";

const WALL = 0.34;
export const ROOM_H = STOREY - SLAB;
const BACK_Z = -ROOM_D / 2 + WALL / 2;
const LEFT_X = -ROOM_W / 2 + WALL / 2;
const WARM_LAMP = "#ffe0bd";
const PENDANT_Y = ROOM_H - 0.74;

const dim = (hex: string, k: number) => `#${new Color(hex).multiplyScalar(k).getHexString()}`;
const shift = (hex: string, to: string, k: number) => `#${new Color(hex).lerp(new Color(to), k).getHexString()}`;

interface Glow { x: number; y: number; z: number; size: number; hex: string; level: number }

/** One thing in a room, built facing +z with its back at z = 0, and `w` wide. */
interface Prop {
  solid: BufferGeometry[];
  lit?: BufferGeometry[];
  glows?: Glow[];
  lamps?: Lamp[];
  foot?: { w: number; d: number };
}

export interface Furnished {
  solid: BufferGeometry[];
  lit: BufferGeometry[];
  thrown: BufferGeometry[];
  lamps: Lamp[];
  blockers: Blocker[];
  accent: string;
}

/* ---- the things ---- */

function sofa(accent: string): Prop {
  const cloth = dim(accent, 0.72);
  const light = shift(accent, "#f2e4cf", 0.18);
  const cushion = (x: number) => moved(soft(1.02, 0.2, 0.74, 0.09, light), x, 0.52, 0.56);
  return {
    solid: [
      moved(soft(2.5, 0.34, 0.92, 0.12, cloth), 0, 0.3, 0.52),
      moved(soft(2.5, 0.78, 0.3, 0.13, cloth), 0, 0.76, 0.17),
      moved(soft(0.3, 0.56, 0.96, 0.13, cloth), -1.2, 0.46, 0.5),
      moved(soft(0.3, 0.56, 0.96, 0.13, cloth), 1.2, 0.46, 0.5),
      cushion(-0.52), cushion(0.52),
      moved(soft(0.46, 0.42, 0.16, 0.08, "#ead9bd"), -0.72, 0.8, 0.42),
      moved(soft(0.42, 0.4, 0.15, 0.08, dim(ACCENTS[0], 1)), 0.76, 0.8, 0.42),
      moved(box(0.09, 0.14, 0.09, OFFICE.deskBody), -1.15, 0.07, 0.86),
      moved(box(0.09, 0.14, 0.09, OFFICE.deskBody), 1.15, 0.07, 0.86),
    ],
    foot: { w: 2.6, d: 1 },
  };
}

function coffeeCounter(): Prop {
  const machineY = 0.96;
  return {
    solid: [
      moved(soft(2.5, 0.92, 0.68, 0.05, OFFICE.deskBody), 0, 0.46, 0.36),
      moved(soft(2.6, 0.07, 0.76, 0.03, OFFICE.deskTop), 0, 0.96, 0.38),
      moved(box(0.02, 0.6, 0.02, OFFICE.steel), 0.42, 0.5, 0.71),
      moved(box(0.02, 0.6, 0.02, OFFICE.steel), -0.42, 0.5, 0.71),
      moved(soft(0.56, 0.66, 0.46, 0.06, "#c7ccd6"), -0.62, machineY + 0.36, 0.3),
      moved(soft(0.42, 0.1, 0.3, 0.03, OFFICE.deskBody), -0.62, machineY + 0.1, 0.42),
      moved(tint(new CylinderGeometry(0.075, 0.06, 0.13, 12), OFFICE.paper), -0.62, machineY + 0.22, 0.46),
      ...[0.16, 0.42, 0.68].map((x, i) => moved(tint(new CylinderGeometry(0.075, 0.065, 0.14, 12), [OFFICE.paper, "#c4605a", "#6d8bc0"][i]), x, machineY + 0.1, 0.48)),
      moved(tint(new CylinderGeometry(0.12, 0.12, 0.34, 12), "#6b4a34"), 0.98, machineY + 0.2, 0.3),
      // A shelf over it, for jars.
      moved(box(2.3, 0.07, 0.34, OFFICE.deskTop), 0, 2.36, 0.17),
      ...[-0.86, -0.46, -0.08, 0.34, 0.76].map((x, i) => moved(tint(new CylinderGeometry(0.13, 0.13, 0.3 + (i % 2) * 0.1, 12), ["#d7b58a", "#8f5c38", "#e9dfcc", "#7fa58f", "#b98d58"][i]), x, 2.56 + (i % 2) * 0.05, 0.18)),
    ],
    lit: [moved(tint(new PlaneGeometry(0.28, 0.1), "#7fe0b8"), -0.62, machineY + 0.52, 0.536)],
    foot: { w: 2.6, d: 0.84 },
  };
}

const BOOK_COLOURS = ["#c4605a", "#3f8f8a", "#d7b58a", "#4f7fc0", "#e9dfcc", "#7a5fa8", "#5d8a4e", "#a8664a", "#2f3a5e", "#c9a7a0"];

function shelfRow(rand: () => number, width: number, y: number, depth: number): BufferGeometry[] {
  const out: BufferGeometry[] = [];
  let x = -width / 2 + 0.05;
  const end = width / 2 - 0.06;
  while (x < end - 0.1) {
    if (rand() < 0.16) { x += 0.16 + rand() * 0.22; continue; }
    const t = 0.06 + rand() * 0.08;
    const h = 0.34 + rand() * 0.2;
    if (x + t > end) break;
    out.push(moved(box(t, h, depth * (0.74 + rand() * 0.2), BOOK_COLOURS[Math.floor(rand() * BOOK_COLOURS.length)]), x + t / 2, y + h / 2, depth / 2 + 0.02));
    x += t + 0.008;
  }
  return out;
}

function bookshelf(rand: () => number, w: number, h: number, rows: number): Prop {
  const d = 0.42;
  const wood = "#4a3428";
  const parts = [moved(box(w, h, 0.05, dim(wood, 0.7)), 0, h / 2, 0.03), moved(box(0.07, h, d, wood), -w / 2, h / 2, d / 2), moved(box(0.07, h, d, wood), w / 2, h / 2, d / 2)];
  for (let i = 0; i <= rows; i += 1) {
    const y = 0.08 + (i * (h - 0.16)) / rows;
    parts.push(moved(box(w, 0.07, d, wood), 0, y, d / 2));
    if (i < rows) parts.push(...shelfRow(rand, w - 0.1, y + 0.035, d - 0.06));
  }
  return { solid: parts, foot: { w: w + 0.1, d: d + 0.1 } };
}

/** A big soft plant in a pot: round leaves, the toy kind. */
function plant(rand: () => number, h: number): Prop {
  const up = new Color(OFFICE.plant);
  const low = new Color(OFFICE.plantDeep);
  const blob = (r: number) => paint(tint(new SphereGeometry(r, 10, 7), OFFICE.plant), (_x, _y, _z, _nx, ny) => low.clone().lerp(up, ny * 0.5 + 0.5));
  const top = h * 0.62;
  return {
    solid: [
      moved(ramp(tint(new CylinderGeometry(0.28, 0.2, 0.42, 14), OFFICE.pot), dim(OFFICE.pot, 0.7), OFFICE.pot), 0, 0.21, 0.32),
      moved(tint(new CylinderGeometry(0.035, 0.05, h * 0.5, 7), "#4a3428"), 0, 0.42 + h * 0.22, 0.32),
      moved(blob(h * 0.25), 0, 0.42 + top * 0.8, 0.32),
      moved(blob(h * 0.19), h * 0.15 + rand() * 0.06, 0.42 + top * 0.58, 0.42),
      moved(blob(h * 0.17), -h * 0.15, 0.42 + top * 0.64, 0.26),
      moved(blob(h * 0.15), h * 0.02, 0.42 + top * 1.06, 0.36),
    ],
    foot: { w: 0.5, d: 0.5 },
  };
}

/** Tall straight leaves, the kind that lives on any office floor. */
function snakePlant(): Prop {
  const leaves = Array.from({ length: 7 }, (_, i) => {
    const turn = (i / 7) * Math.PI * 2;
    const leaf = ramp(tint(new ConeGeometry(0.08, 0.9 + (i % 3) * 0.24, 5), OFFICE.plant), OFFICE.plantDeep, "#7fb56a");
    leaf.translate(0, 0.5, 0).applyMatrix4(new Matrix4().makeRotationZ(i ? 0.14 : 0));
    return moved(leaf, Math.cos(turn) * 0.09, 0.32, 0.3 + Math.sin(turn) * 0.09, turn);
  });
  return { solid: [moved(ramp(tint(new CylinderGeometry(0.24, 0.19, 0.36, 12), "#d9d2c4"), "#a59f92", "#e4dccd"), 0, 0.18, 0.3), ...leaves], foot: { w: 0.42, d: 0.42 } };
}

function rack(): Prop {
  const led = (x: number, y: number, hex: string) => moved(tint(new PlaneGeometry(0.05, 0.03), hex), x, y, 0.71);
  const lights: BufferGeometry[] = [];
  const rand = seeded(9);
  for (let u = 0; u < 9; u += 1) {
    for (let i = 0; i < 5; i += 1) if (rand() < 0.7) lights.push(led(-0.36 + i * 0.09, 0.42 + u * 0.21, rand() < 0.8 ? "#6fd6a0" : "#8fb8f2"));
  }
  const panel = Array.from({ length: 9 }, (_, u) => moved(box(0.9, 0.17, 0.02, "#1d2029"), 0, 0.42 + u * 0.21, 0.7));
  return { solid: [moved(soft(1, 2.3, 0.7, 0.04, "#191b23"), 0, 1.15, 0.35), ...panel], lit: lights, foot: { w: 1, d: 0.76 } };
}

function arcade(accent: string): Prop {
  const body = dim(accent, 0.62);
  return {
    solid: [
      moved(soft(0.92, 1.62, 0.76, 0.05, body), 0, 0.81, 0.4),
      moved(soft(0.94, 0.3, 0.8, 0.05, dim(accent, 0.9)), 0, 1.78, 0.42),
      moved(soft(0.82, 0.08, 0.34, 0.03, "#20222c"), 0, 1.06, 0.86),
      moved(tint(new SphereGeometry(0.05, 8, 6), "#c4605a"), -0.2, 1.14, 0.88),
      moved(tint(new SphereGeometry(0.04, 8, 6), "#8fb8f2"), 0.12, 1.12, 0.88),
      moved(tint(new SphereGeometry(0.04, 8, 6), "#8fd9a4"), 0.26, 1.12, 0.88),
    ],
    lit: [moved(tint(new PlaneGeometry(0.66, 0.5), "#75b5ef"), 0, 1.38, 0.79), moved(tint(new PlaneGeometry(0.8, 0.16), "#ffe7c0"), 0, 1.8, 0.83)],
    glows: [{ x: 0, y: 1.4, z: 0.9, size: 1.5, hex: "#6aa8e8", level: 0.32 }],
    foot: { w: 0.96, d: 0.86 },
  };
}

function beanbags(accent: string): Prop {
  const bag = (hex: string, r: number) => tint(new SphereGeometry(r, 14, 10), hex).scale(1, 0.72, 1);
  const table = [moved(tint(new CylinderGeometry(0.4, 0.4, 0.05, 18), OFFICE.deskTop), 0, 0.42, 0.8), moved(tint(new CylinderGeometry(0.05, 0.08, 0.4, 8), OFFICE.deskBody), 0, 0.2, 0.8), moved(tint(new CylinderGeometry(0.07, 0.06, 0.12, 10), OFFICE.paper), 0.1, 0.5, 0.86)];
  return { solid: [moved(bag(dim(accent, 0.8), 0.52), -0.9, 0.36, 0.64), moved(bag(shift(accent, "#e9dfcc", 0.45), 0.48), 0.92, 0.33, 0.72), ...table], foot: { w: 2.5, d: 1.2 } };
}

/** A small round table with stools: where two people talk it over. */
function meetingTable(accent: string): Prop {
  const stool = (x: number, z: number) => [moved(tint(new CylinderGeometry(0.2, 0.2, 0.08, 14), accent), x, 0.46, z), moved(tint(new CylinderGeometry(0.03, 0.05, 0.44, 8), OFFICE.deskBody), x, 0.22, z)];
  return {
    solid: [
      moved(tint(new CylinderGeometry(0.62, 0.62, 0.06, 22), OFFICE.deskTop), 0, 0.76, 0.86),
      moved(tint(new CylinderGeometry(0.05, 0.08, 0.72, 10), OFFICE.deskBody), 0, 0.37, 0.86),
      moved(tint(new CylinderGeometry(0.3, 0.32, 0.03, 16), OFFICE.deskBody), 0, 0.02, 0.86),
      moved(box(0.3, 0.015, 0.22, OFFICE.paper), -0.12, 0.8, 0.8, 0.3),
      moved(tint(new CylinderGeometry(0.055, 0.05, 0.1, 10), "#e9e4d8"), 0.24, 0.84, 0.96),
      ...stool(-0.95, 0.72), ...stool(0.96, 0.82), ...stool(0.1, 0.1),
    ],
    foot: { w: 2.2, d: 1.5 },
  };
}

/** A low cabinet with things growing on it. */
function plantCabinet(rand: () => number): Prop {
  const top = 0.74;
  const pots = [-0.86, -0.3, 0.3, 0.86].map((x, i) => {
    const pot = moved(ramp(tint(new CylinderGeometry(0.15, 0.11, 0.22, 12), i % 2 ? "#d9d2c4" : OFFICE.pot), "#7a5a48", i % 2 ? "#e4dccd" : OFFICE.pot), x, top + 0.11, 0.32);
    const leaf = moved(tint(new SphereGeometry(0.17 + rand() * 0.09, 9, 7), i % 2 ? OFFICE.plant : OFFICE.plantDeep), x, top + 0.34 + rand() * 0.08, 0.32);
    return [pot, leaf];
  });
  return {
    solid: [
      moved(soft(2.4, top, 0.6, 0.04, "#3a3038"), 0, top / 2, 0.32),
      moved(box(2.44, 0.05, 0.64, OFFICE.deskTop), 0, top + 0.02, 0.32),
      ...[-0.8, 0, 0.8].map((x) => moved(box(0.02, top - 0.14, 0.01, "#2a2730"), x, top / 2, 0.625)),
      ...pots.flat(),
    ],
    foot: { w: 2.5, d: 0.72 },
  };
}

function floorLamp(on: boolean): Prop {
  const shade = tint(new CylinderGeometry(0.2, 0.3, 0.32, 16), on ? "#ffdca8" : "#b9b2a4");
  return {
    solid: [moved(tint(new CylinderGeometry(0.17, 0.19, 0.04, 14), OFFICE.deskBody), 0, 0.02, 0.32), moved(tint(new CylinderGeometry(0.018, 0.018, 1.74, 8), OFFICE.deskBody), 0, 0.9, 0.32)],
    lit: [moved(shade, 0, 1.86, 0.32)],
    glows: on ? [{ x: 0, y: 1.86, z: 0.32, size: 1.5, hex: "#ffd0a0", level: 0.24 }] : [],
    lamps: on ? [{ x: 0, y: 1.8, z: 0.32, colour: rgb(WARM_LAMP), power: 0.9, reach: 1.1 }] : [],
    foot: { w: 0.32, d: 0.32 },
  };
}

function wallArt(accent: string, rand: () => number): Prop {
  const frame = (w: number, h: number, x: number, y: number, paintings: BufferGeometry[]) => [moved(box(w + 0.1, h + 0.1, 0.05, "#2a2730"), x, y, 0.03), moved(tint(new PlaneGeometry(w, h), "#e9dfcc"), x, y, 0.06), ...paintings.map((p) => moved(p, x, y, 0.065))];
  const blocks = (w: number, h: number) => [
    tint(new PlaneGeometry(w * 0.72, h * 0.34), accent).translate(0, -h * 0.14, 0),
    tint(new PlaneGeometry(w * 0.34, h * 0.42), "#3a4670").translate(-w * 0.12, h * 0.16, 0.002),
    tint(new CircleGeometry(w * 0.15, 20), "#e0937a").translate(w * 0.2, h * 0.24, 0.003),
  ];
  const two = rand() < 0.5;
  return {
    solid: two
      ? [...frame(0.86, 1.16, -0.56, 2.24, blocks(0.86, 1.16)), ...frame(0.62, 0.62, 0.42, 2.46, [tint(new CircleGeometry(0.2, 20), accent)])]
      : [...frame(1.5, 0.96, 0, 2.3, blocks(1.5, 0.96))],
  };
}

function whiteboard(accent: string, rand: () => number): Prop {
  const w = 2.5;
  const lines: BufferGeometry[] = [];
  for (let i = 0; i < 5; i += 1) lines.push(moved(tint(new PlaneGeometry(0.5 + rand() * 0.8, 0.035), i % 2 ? accent : "#3a4670"), -0.5 + rand() * 0.24, 2.6 - i * 0.2, 0.06));
  const box2 = (x: number, y: number) => moved(tint(new PlaneGeometry(0.42, 0.26), shift(accent, "#ffffff", 0.45)), x, y, 0.06);
  return {
    solid: [
      moved(soft(w + 0.14, 1.42, 0.06, 0.02, OFFICE.steel), 0, 2.16, 0.03),
      moved(box(w, 1.28, 0.02, "#eef0ef"), 0, 2.16, 0.055),
      moved(box(w * 0.6, 0.04, 0.09, OFFICE.steel), 0, 1.46, 0.09),
      moved(box(0.14, 0.03, 0.03, accent), -0.2, 1.495, 0.1),
      ...lines, box2(0.68, 2.5), box2(0.68, 2.1),
    ],
  };
}

function cat(hex: string): Prop {
  const fur = (r: number) => tint(new SphereGeometry(r, 12, 9), hex);
  const ear = () => tint(new ConeGeometry(0.05, 0.1, 4), hex);
  const tail = tint(new TorusGeometry(0.2, 0.038, 6, 12, Math.PI * 1.1), hex).rotateX(Math.PI / 2);
  return {
    solid: [
      moved(fur(0.24).scale(1.25, 0.72, 1), 0, 0.17, 0),
      moved(fur(0.15), 0.26, 0.24, 0.08),
      moved(ear(), 0.2, 0.39, 0.06), moved(ear(), 0.33, 0.39, 0.1),
      moved(tail, -0.02, 0.06, 0.04),
    ],
  };
}

function clock(): Prop {
  const face = tint(new CylinderGeometry(0.34, 0.34, 0.06, 24), "#efe9dc").rotateX(Math.PI / 2);
  const ring = tint(new TorusGeometry(0.34, 0.03, 8, 24), "#2a2730");
  return { solid: [moved(face, 0, 2.6, 0.04), moved(ring, 0, 2.6, 0.06), moved(box(0.03, 0.22, 0.01, "#2a2730"), 0, 2.69, 0.08), moved(box(0.15, 0.03, 0.01, "#2a2730"), 0.06, 2.6, 0.08)] };
}

/** A window out on the dusk, lit by the sky, not by the room: the coolest thing on a warm wall. */
function dusk(w: number, h: number, cx: number, cy: number): BufferGeometry[] {
  const pane = new PlaneGeometry(w, h, 1, 6);
  const top = new Color("#17204e");
  const mid = new Color("#47457f");
  const low = new Color("#a8708c");
  const glass = paint(tint(pane, "#000000"), (_x, y) => {
    const k = y / h + 0.5;
    return k > 0.45 ? mid.clone().lerp(top, (k - 0.45) / 0.55) : low.clone().lerp(mid, k / 0.45);
  }).translate(cx, cy, 0.07);
  const rand = seeded(Math.round(w * 100 + cy * 10));
  const towers: BufferGeometry[] = [];
  let x = -w / 2;
  while (x < w / 2 - 0.1) {
    const bw = 0.16 + rand() * 0.22;
    const bh = 0.14 + rand() * h * 0.3;
    towers.push(moved(tint(new PlaneGeometry(Math.min(bw, w / 2 - x), bh), "#1a1c40"), cx + x + Math.min(bw, w / 2 - x) / 2, cy - h / 2 + bh / 2, 0.073));
    x += bw;
  }
  return [glass, ...towers];
}

function windowBay(rand: () => number): Prop {
  const w = 2.5;
  const h = 1.6;
  const y = 2.14;
  const bar = (x: number) => moved(box(0.05, h, 0.03, "#2a2730"), x, y, 0.09);
  return {
    solid: [
      moved(soft(w + 0.18, h + 0.18, 0.08, 0.02, "#2a2730"), 0, y, 0.02),
      bar(-w / 6), bar(w / 6),
      moved(soft(w + 0.34, 0.08, 0.3, 0.03, OFFICE.deskTop), 0, y - h / 2 - 0.1, 0.14),
      ...(rand() < 0.6 ? [moved(ramp(tint(new CylinderGeometry(0.13, 0.1, 0.2, 12), OFFICE.pot), dim(OFFICE.pot, 0.7), OFFICE.pot), -0.8, y - h / 2 + 0.03, 0.14), moved(tint(new SphereGeometry(0.2, 9, 7), OFFICE.plant), -0.8, y - h / 2 + 0.26, 0.14)] : []),
    ],
    lit: dusk(w, h, 0, y),
  };
}

function coatRack(): Prop {
  return { solid: [moved(tint(new CylinderGeometry(0.03, 0.03, 1.9, 8), OFFICE.deskBody), 0, 0.95, 0.28), moved(tint(new CylinderGeometry(0.2, 0.22, 0.04, 12), OFFICE.deskBody), 0, 0.02, 0.28), moved(soft(0.4, 0.7, 0.22, 0.1, "#57617f"), 0.14, 1.42, 0.28)], foot: { w: 0.4, d: 0.4 } };
}

/** A hexagon rug, in the project's colour, under the middle of the room. */
function hexRug(accent: string, x: number, z: number, r: number): BufferGeometry[] {
  const plate = (radius: number, y: number, hex: string) => moved(tint(new CylinderGeometry(radius, radius, 0.02, 6), hex), x, y, z, Math.PI / 6);
  return [plate(r, 0.018, dim(accent, 0.74)), plate(r - 0.2, 0.024, shift(accent, "#e9dfcc", 0.2)), plate(r - 0.38, 0.03, dim(accent, 0.92))];
}

/* ---- the lights of a room ---- */

type PendantKind = "dome" | "globe" | "cone";

function pendant(kind: PendantKind, accent: string, x: number, z: number, on: boolean): { solid: BufferGeometry[]; lit: BufferGeometry[]; glows: Glow[] } {
  const cord = moved(tint(new CylinderGeometry(0.012, 0.012, ROOM_H - PENDANT_Y, 6), "#1d1f28"), x, (ROOM_H + PENDANT_Y) / 2, z);
  const bulb = on ? OFFICE.glow : "#39405e";
  const solid = [cord];
  const lit: BufferGeometry[] = [];
  if (kind === "globe") {
    lit.push(moved(ramp(tint(new SphereGeometry(0.19, 14, 10), bulb), on ? "#ffc78f" : bulb, bulb), x, PENDANT_Y, z));
  } else {
    const hood = kind === "dome"
      ? tint(new SphereGeometry(0.34, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2), dim(accent, 0.8)).translate(0, -0.14, 0)
      : tint(new ConeGeometry(0.32, 0.34, 16, 1, true), "#23252f");
    solid.push(moved(hood, x, PENDANT_Y + 0.08, z));
    lit.push(moved(tint(new SphereGeometry(0.13, 10, 8), bulb), x, PENDANT_Y - 0.1, z));
  }
  return { solid, lit, glows: on ? [{ x, y: PENDANT_Y - 0.06, z, size: 1.5, hex: "#ffd8a0", level: 0.2 }] : [] };
}

/* ---- a room ---- */

const shuffled = <T>(list: readonly T[], rand: () => number): T[] => {
  const out = [...list];
  for (let i = out.length - 1; i > 0; i -= 1) {
    const j = Math.floor(rand() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
};

const spin = (x: number, z: number, turn: number) => ({ x: x * Math.cos(turn) + z * Math.sin(turn), z: -x * Math.sin(turn) + z * Math.cos(turn) });

/** The colour of a floor: from its name, and stepped by its height so two floors in a row differ. */
export const accentOf = (name: string, index = 0): string => ACCENTS[(hashOf(name) + index * 3) % ACCENTS.length];

interface RoomPlan { id: string; index: number; lit: boolean; board: boolean }

/** Everything in a floor but its desks, from its name and its height in the tower. */
export function furnish(room: RoomPlan): Furnished {
  const hash = hashOf(room.id);
  const rand = seeded(hash * 31 + room.index * 7919);
  const accent = accentOf(room.id, room.index);
  const out: Furnished = { solid: [], lit: [], thrown: [], lamps: [], blockers: [], accent };

  const put = (p: Prop, x: number, z: number, turn = 0) => {
    out.solid.push(...p.solid.map((g) => moved(g, x, 0, z, turn)));
    out.lit.push(...(p.lit ?? []).map((g) => moved(g, x, 0, z, turn)));
    for (const g of p.glows ?? []) {
      const at = spin(g.x, g.z, turn);
      out.thrown.push(flare(x + at.x, g.y, z + at.z, g.size, g.hex, g.level));
    }
    for (const l of p.lamps ?? []) {
      const at = spin(l.x, l.z, turn);
      out.lamps.push({ ...l, x: x + at.x, z: z + at.z });
    }
    if (p.foot) {
      const c = spin(0, p.foot.d / 2, turn);
      const sideways = Math.abs(Math.sin(turn)) > 0.5;
      out.blockers.push({ x: x + c.x, z: z + c.z, w: sideways ? p.foot.d : p.foot.w, d: sideways ? p.foot.w : p.foot.d, dark: 0.5 });
    }
  };

  // The back wall: left, middle, and right unless the board has it. Each room draws its own.
  // From the viewer's corner the left of the back wall is behind the people, and what stands
  // there stays low, or it fights them for the eye. Anything tall goes on the far right.
  const low = shuffled<() => Prop>([() => sofa(accent), () => beanbags(accent), coffeeCounter, () => meetingTable(accent), () => plantCabinet(rand)], rand);
  const tall = shuffled<() => Prop>([() => bookshelf(rand, 2.5, 2.7, 4), rack, () => arcade(accent)], rand);
  const hung: Array<() => Prop> = shuffled([() => wallArt(accent, rand), clock, () => wallArt(accent, rand)], rand);
  const onWall: Array<() => Prop> = shuffled([() => windowBay(rand), () => whiteboard(accent, rand), () => windowBay(rand)], rand);
  const slots = [-3 + rand() * 0.3, 0.2 + rand() * 0.3, 3.1];

  if (room.lit) {
    // People are working here, and they are the picture: one quiet corner, one thing on the wall, one plant.
    const quiet = shuffled<() => Prop>([() => sofa(accent), coffeeCounter, () => plantCabinet(rand)], rand);
    put(quiet[0](), slots[0], BACK_Z);
    put(room.index % 2 ? windowBay(rand) : hung[0](), LEFT_X, -1.5, Math.PI / 2);
    put(rand() < 0.5 ? plant(rand, 1.7) : snakePlant(), LEFT_X + 0.1, 2.5, Math.PI / 2);
  } else {
    // Nobody is in, and the room can be a room.
    put(low[0](), slots[0], BACK_Z);
    put(hung[0](), slots[0], BACK_Z);
    put(low[1](), slots[1], BACK_Z);
    put(hung[1](), slots[1], BACK_Z);
    if (!room.board) put(tall[0](), slots[2], BACK_Z);
    put(onWall[0](), LEFT_X, -1.5, Math.PI / 2);
    const standing: Array<() => Prop> = shuffled([() => bookshelf(rand, 1.9, 2.3, 3), () => plant(rand, 2.1), () => arcade(accent), coatRack], rand);
    put(standing[0](), LEFT_X, 1.6, Math.PI / 2);
    put(rand() < 0.5 ? snakePlant() : plant(rand, 1.7), LEFT_X + 0.1, -2.75, Math.PI / 2);
    out.solid.push(...hexRug(shift(accent, "#6a6a72", 0.42), 0, 0.06, 2.7));
    if (rand() < 0.45) put(rand() < 0.5 ? plant(rand, 1.5) : snakePlant(), -4.2, 2.75);
  }
  if (rand() < 0.5) put(cat(["#3a3d4a", "#d8d2c8", "#8a8f9c"][hash % 3]), 3.2, 2.7, 0.6);

  // The lamps. Nobody in, and it is the sky that lights the room, and one lamp left on.
  const kind = (["dome", "globe", "cone"] as PendantKind[])[hash % 3];
  for (const x of [-3.1, 0, 3.1]) {
    const p = pendant(kind, accent, x, 0.1, room.lit);
    out.solid.push(...p.solid);
    out.lit.push(...p.lit);
    for (const g of p.glows) out.thrown.push(flare(g.x, g.y, g.z, g.size, g.hex, g.level));
    if (room.lit) out.thrown.push(pool(x, 0.03, 0.1, 5.4, 4.6, "#ffcf9e", 0.09));
    if (room.lit) out.lamps.push({ x, y: PENDANT_Y - 0.1, z: 0.1, colour: rgb(WARM_LAMP), power: 1.05, reach: 2.2 });
  }
  if (!room.lit) put(floorLamp(true), -1.25, BACK_Z);
  return out;
}

/** The colours of a room's walls and tiles, lit or left for the night. */
export const roomAmbient = (lit: boolean): RoomLight["ambient"] => (lit ? [0.68, 0.62, 0.55] : [0.56, 0.62, 0.86]);
