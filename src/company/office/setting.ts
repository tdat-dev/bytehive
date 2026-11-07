/* Where the tower stands: a small plot at the blue hour, the edge of a town behind it and
 * the sky. All of it is built once and never changes, a handful of draw calls in all. The
 * lens is long and looks down, so a real horizon would sit over the top of the frame: as
 * in a model photographed on a table, the town stands close and the sky is a painted
 * sheet behind it. Nothing here reads the page. */
import {
  AdditiveBlending, BufferAttribute, BufferGeometry, CanvasTexture, CircleGeometry, Color, CylinderGeometry, Group, Mesh, MeshBasicMaterial, PlaneGeometry, Points, PointsMaterial,
  SphereGeometry, SRGBColorSpace, type Material, type MeshStandardMaterial, type Texture,
} from "three";
import { box, flare, merged, moved, paint, pool, ramp, roundedSlab, seeded, soft, tint } from "./geometry";
import { GROUND, LIFT_W, PLOT, ROOM_D, ROOM_W, SLAB, VIEW } from "./layout";
import { OFFICE } from "./palette";

const PLATE_TOP = GROUND - SLAB;
/** Everything here is laid out from the viewer: +x is the screen's right, -z is away. */
const FACING = Math.atan2(VIEW.x, VIEW.z);
const SKY = { z: -330, w: 1000, y0: -92, y1: 72 };
/** Where the sky's painted light sits, as a share of the sheet from the bottom. */
const SKY_GLOW = 0.37;
const MOON = { x: -50, y: 5, r: 1.7 };
const TOWN = { near: -30, far: -74, edge: -92 };

type Painter = (c: OffscreenCanvasRenderingContext2D, w: number, h: number) => void;

const stops = (g: CanvasGradient, list: Array<[number, string]>) => { for (const [k, c] of list) g.addColorStop(k, c); };

/** The dusk, from the night overhead to the last of the light low down, and its clouds. */
const paintSky: Painter = (c, w, h) => {
  const air = c.createLinearGradient(0, 0, 0, h);
  const glow = 1 - SKY_GLOW;
  stops(air, [[0, OFFICE.skyTop], [glow - 0.42, OFFICE.skyHigh], [glow - 0.2, OFFICE.skyMid], [glow - 0.1, OFFICE.skyLow], [glow, OFFICE.skyGlow], [1, OFFICE.skyGlow]]);
  c.fillStyle = air;
  c.fillRect(0, 0, w, h);
  const rand = seeded(11);
  // Long clouds holding what light is left. Lit from under, so warmer the lower they lie.
  c.filter = `blur(${h * 0.012}px)`;
  for (let i = 0; i < 26; i += 1) {
    const y = h * (glow - 0.02 - rand() * 0.34);
    const low = 1 - (glow - y / h) / 0.36;
    c.fillStyle = low > 0.6 ? "rgba(190,130,150,.1)" : "rgba(100,120,190,.08)";
    c.beginPath();
    c.ellipse(rand() * w, y, w * (0.04 + rand() * 0.09), h * (0.004 + rand() * 0.012), 0, 0, Math.PI * 2);
    c.fill();
  }
  c.filter = "none";
};

function painted(w: number, h: number, paintIt: Painter): Texture {
  const canvas = new OffscreenCanvas(w, h);
  paintIt(canvas.getContext("2d")!, w, h);
  const t = new CanvasTexture(canvas);
  t.colorSpace = SRGBColorSpace;
  return t;
}

function starField(): BufferGeometry {
  const rand = seeded(5);
  const count = 340;
  const at = new Float32Array(count * 3);
  const colour = new Float32Array(count * 3);
  const warm = new Color("#ffe9c9");
  const cold = new Color("#cdd9ff");
  for (let i = 0; i < count; i += 1) {
    const height = rand() ** 0.7;
    at.set([(rand() - 0.5) * SKY.w * 0.7, -20 + height * 92, SKY.z + 3], i * 3);
    const c = (rand() < 0.3 ? warm : cold).clone().multiplyScalar((0.25 + rand() ** 3 * 0.75) * Math.min(1, height * 2.2));
    c.toArray(colour, i * 3);
  }
  const g = new BufferGeometry();
  g.setAttribute("position", new BufferAttribute(at, 3));
  g.setAttribute("color", new BufferAttribute(colour, 3));
  return g;
}

/** The moon, low on the left of the tower, with its light on the air round it. */
function moonGeometry(): BufferGeometry {
  const face = tint(new CircleGeometry(MOON.r, 40), OFFICE.moon);
  const shade = tint(new CircleGeometry(MOON.r * 0.24, 16), "#c9c4b8");
  const shade2 = tint(new CircleGeometry(MOON.r * 0.15, 14), "#c9c4b8");
  return merged([face, moved(shade, -MOON.r * 0.26, MOON.r * 0.22, 0.05), moved(shade2, MOON.r * 0.3, -MOON.r * 0.33, 0.05)]);
}

interface Layer { z: number; low: number; high: number; top: string; foot: string; lit: number; glow: number }

/** A row of blocks, misted at the foot, with a few windows still on. */
function townRow(l: Layer, rand: () => number, windows: BufferGeometry[]): BufferGeometry[] {
  const out: BufferGeometry[] = [];
  let x = -260;
  while (x < 260) {
    const w = 3 + rand() * 6;
    const h = l.low + (l.high - l.low) * rand() ** 1.8;
    const d = 5;
    out.push(moved(ramp(box(w, h, d, l.top), l.foot, l.top), x, h / 2 + PLATE_TOP, l.z));
    const roof = rand();
    if (roof < 0.28) out.push(moved(box(w * 0.5, 0.9 + rand() * 1.6, d * 0.7, l.top), x + (rand() - 0.5) * w * 0.3, h + 0.4 + PLATE_TOP, l.z));
    else if (roof < 0.42) out.push(moved(tint(new CylinderGeometry(0.5, 0.5, 0.9, 8), l.top), x + (rand() - 0.5) * w * 0.5, h + 0.75 + PLATE_TOP, l.z), moved(box(0.8, 0.3, 0.8, l.top), x, h + 0.15 + PLATE_TOP, l.z));
    else if (roof < 0.5) out.push(moved(box(0.07, 2.4, 0.07, l.top), x + w * 0.2, h + 1.2 + PLATE_TOP, l.z));
    const cols = Math.floor(w / 0.8);
    const rows = Math.floor(h / 0.95);
    for (let r = 1; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        if (rand() > l.lit) continue;
        const hue = rand() < 0.75 ? "#ffcf94" : "#a9c6ef";
        const k = (0.4 + rand() * 0.5) * l.glow;
        windows.push(moved(tint(new PlaneGeometry(0.34, 0.46), `#${new Color(hue).multiplyScalar(k).getHexString()}`), x - w / 2 + 0.5 + c * 0.8, r * 0.95 + PLATE_TOP, l.z + d / 2 + 0.03));
      }
    }
    x += w + rand() * 1.6;
  }
  return out;
}

function* townParts(out: BufferGeometry[]): Generator<void, void> {
  const rand = seeded(23);
  const windows: BufferGeometry[] = [];
  const layers: Layer[] = [
    { z: TOWN.far, low: 5, high: 17, top: "#26316a", foot: OFFICE.haze, lit: 0.04, glow: 0.55 },
    { z: (TOWN.far + TOWN.near) / 2, low: 3.5, high: 11, top: "#1a2452", foot: "#343f7c", lit: 0.07, glow: 0.7 },
    { z: TOWN.near, low: 2.2, high: 6.5, top: "#111838", foot: "#26305e", lit: 0.09, glow: 0.85 },
  ];
  for (const l of layers) {
    out.push(...townRow(l, rand, windows));
    yield;
  }
  out.push(...windows);
}

/** The floor of the world, into the mist at the foot of the town. */
function fieldGeometry(): BufferGeometry {
  const near = new Color("#0e1430");
  const mid = new Color("#161e46");
  const far = new Color(OFFICE.haze);
  return paint(tint(new PlaneGeometry(1000, 300, 1, 24).rotateX(-Math.PI / 2).translate(0, PLATE_TOP - 0.36, 150 + TOWN.edge), "#000000"), (_x, _y, z) => {
    const k = (20 - z) / (20 - TOWN.edge);
    return k < 0 ? near.clone() : near.clone().lerp(mid, Math.min(1, k * 1.6)).lerp(far, Math.max(0, k - 0.6) / 0.4);
  });
}

/** A tree of a few round clumps. Cold on the side of the sky, warm on the side of the tower. */
function tree(rand: () => number, tower: { x: number; z: number }, h: number): BufferGeometry {
  const leaf = new Color(OFFICE.leaf);
  const deep = new Color(OFFICE.leafDeep);
  const warm = new Color("#a98a4a");
  const lit = (g: BufferGeometry, ox: number, oy: number, oz: number) => paint(g, (_x, _y, _z, nx, ny, nz) => {
    const up = ny * 0.5 + 0.5;
    const toTower = Math.max(0, nx * tower.x + nz * tower.z);
    return deep.clone().lerp(leaf, up).lerp(warm, toTower * toTower * 0.42 * (1 - up * 0.6));
  }).translate(ox, oy, oz);
  const blob = (r: number) => tint(new SphereGeometry(r, 12, 9), OFFICE.leaf);
  const r = h * 0.34;
  return merged([
    moved(ramp(tint(new CylinderGeometry(0.16, 0.26, h * 0.6, 8), OFFICE.bark), "#2a2228", OFFICE.bark), 0, h * 0.3, 0),
    lit(blob(r), 0, h * 0.72, 0),
    lit(blob(r * 0.78), r * (0.5 + rand() * 0.3), h * 0.6, r * 0.3),
    lit(blob(r * 0.74), -r * 0.6, h * 0.6, -r * 0.2),
    lit(blob(r * 0.64), r * 0.1, h * 0.92, r * 0.2),
  ]);
}

function bench(): BufferGeometry {
  const slat = (y: number, z: number) => moved(soft(1.7, 0.07, 0.15, 0.03, OFFICE.deskTop), 0, y, z);
  const leg = (x: number) => moved(box(0.09, 0.46, 0.5, OFFICE.deskBody), x, 0.23, 0);
  return merged([slat(0.47, -0.17), slat(0.47, 0), slat(0.47, 0.17), slat(0.8, -0.27), slat(0.64, -0.27), leg(-0.7), leg(0.7), moved(box(0.09, 0.45, 0.09, OFFICE.deskBody), -0.7, 0.72, -0.27), moved(box(0.09, 0.45, 0.09, OFFICE.deskBody), 0.7, 0.72, -0.27)]);
}

const STREET_LAMP_H = 3.5;

function streetLamp(): BufferGeometry {
  return merged([
    moved(tint(new CylinderGeometry(0.2, 0.26, 0.24, 10), OFFICE.deskBody), 0, 0.12, 0),
    moved(tint(new CylinderGeometry(0.06, 0.09, STREET_LAMP_H, 8), OFFICE.deskBody), 0, STREET_LAMP_H / 2, 0),
    moved(tint(new CylinderGeometry(0.24, 0.06, 0.1, 10), OFFICE.deskBody), 0, STREET_LAMP_H + 0.34, 0),
  ]);
}

const LAMPS = [{ x: 9.8, z: -0.8 }, { x: -6.6, z: 6.4 }];
const TREES = [
  { x: -8, z: -3.4, h: 6.2 }, { x: -7.8, z: 2.2, h: 4.8 }, { x: -5.6, z: -6.2, h: 5.4 },
  { x: 9.2, z: -5.2, h: 6 }, { x: 7.6, z: -6.6, h: 4.8 }, { x: -7.9, z: 6.5, h: 3.8 },
];
/** Stones from the edge of the plot to the foot of the lift. */
const PATH = [[7.1, 4.6], [7.7, 5.3], [8.3, 5.9], [8.9, 6.5], [9.5, 7.1], [10.2, 7.8]];

function hexStone(x: number, z: number, r: number, hex: string): BufferGeometry {
  return moved(tint(new CylinderGeometry(r, r, 0.08, 6), hex), x, 0, z, 0.5);
}

function shrub(rand: () => number, x: number, z: number, r: number): BufferGeometry {
  const up = new Color(OFFICE.grassLit);
  const down = new Color(OFFICE.leafDeep);
  const blob = (k: number) => paint(tint(new SphereGeometry(r * k, 10, 7), OFFICE.leaf), (_x, _y, _z, _nx, ny) => down.clone().lerp(up, ny * 0.5 + 0.5));
  return merged([moved(blob(1), x, r * 0.7, z), moved(blob(0.74), x + r * (0.5 + rand() * 0.4), r * 0.5, z + r * 0.3), moved(blob(0.66), x - r * 0.6, r * 0.45, z - r * 0.2)]);
}

/** The plot: lawn, kerb, stones, trees and a bench, for one mesh that takes the shadows. */
function* plotParts(parts: BufferGeometry[]): Generator<void, void> {
  const rand = seeded(41);
  const lawn = moved(tint(roundedSlab(PLOT.w, PLOT.d, 0.55, 1.6), OFFICE.kerb, OFFICE.grass), PLOT.x, PLATE_TOP - 0.08, PLOT.z);
  const foot = moved(tint(roundedSlab(ROOM_W + LIFT_W + 2.6, ROOM_D + 2.6, 0.42, 0.5), OFFICE.shellEdge, OFFICE.stone), LIFT_W / 2, PLATE_TOP, 0);
  const tower = (t: { x: number; z: number }) => { const l = Math.hypot(t.x, t.z); return { x: -t.x / l, z: -t.z / l }; };
  parts.push(lawn, foot);
  yield;
  for (const t of TREES) {
    parts.push(moved(tree(rand, tower(t), t.h), t.x, PLATE_TOP - 0.08, t.z));
    yield;
  }
  for (const [x, z] of PATH) parts.push(moved(hexStone(0, 0, 0.42 + rand() * 0.08, OFFICE.stone), x, PLATE_TOP - 0.05, z));
  parts.push(moved(bench(), 9.9, PLATE_TOP - 0.08, 3, -Math.PI / 2));
  for (const l of LAMPS) parts.push(moved(streetLamp(), l.x, PLATE_TOP - 0.08, l.z));
  yield;
  for (let i = 0; i < 9; i += 1) parts.push(moved(shrub(rand, 0, 0, 0.34 + rand() * 0.24), -5.2 + i * 1.04 + rand() * 0.2, PLATE_TOP - 0.08, ROOM_D / 2 + 1.95 + rand() * 0.3));
  for (let i = 0; i < 3; i += 1) parts.push(moved(shrub(rand, 0, 0, 0.44), 8.5 + i * 0.7, PLATE_TOP - 0.08, 0.6 + i * 1.1));
}

export interface Setting {
  group: Group;
  /** The rooms' light on the plot: off when no room is lit. */
  light: Group;
  /** What sits behind everything, for the small shift a moving mouse gives it. */
  far: Group;
  /** Every material here, for compiling their shaders ahead of the first frame. */
  looks: Material[];
  /** The sky first: it is what the first frame needs. */
  sky(): void;
  /** All the rest, a little at a time. */
  rest(): Generator<void, void>;
  dispose(): void;
}

export function createSetting(o: { outdoor: MeshStandardMaterial; thrown: MeshBasicMaterial }): Setting {
  const group = new Group();
  const far = new Group();
  const light = new Group();
  far.rotation.y = FACING;
  const floor = new Group();
  floor.rotation.y = FACING;
  group.add(far, floor, light);
  const owned: Array<BufferGeometry | Material | Texture> = [];
  const flat = new MeshBasicMaterial({ vertexColors: true, toneMapped: false, dithering: true });
  owned.push(flat);
  const looks: Material[] = [flat];

  function sky(): void {
    const sheet = new PlaneGeometry(SKY.w, SKY.y1 - SKY.y0).translate(0, (SKY.y0 + SKY.y1) / 2, SKY.z);
    const skyMap = painted(1024, 512, paintSky);
    const air = new MeshBasicMaterial({ map: skyMap, toneMapped: false, dithering: true, depthWrite: false, fog: false });
    const skyMesh = new Mesh(sheet, air);
    skyMesh.renderOrder = -3;
    const starMaterial = new PointsMaterial({ size: 1.6, sizeAttenuation: false, vertexColors: true, toneMapped: false, transparent: true, depthWrite: false, blending: AdditiveBlending });
    const stars = new Points(starField(), starMaterial);
    stars.renderOrder = -2;
    const moon = new Mesh(moved(moonGeometry(), MOON.x, MOON.y, SKY.z + 5), flat);
    moon.renderOrder = -2;
    const halo = new Mesh(flare(MOON.x, MOON.y, SKY.z + 4, 16, "#7f92d8", 0.2), o.thrown);
    halo.renderOrder = -2;
    far.add(skyMesh, stars, moon, halo);
    owned.push(sheet, skyMap, air, starMaterial, stars.geometry, moon.geometry, halo.geometry);
    looks.push(air, starMaterial);
  }

  function* rest(): Generator<void, void> {
    const field = new Mesh(fieldGeometry(), flat);
    floor.add(field);
    owned.push(field.geometry);
    yield;

    const parts: BufferGeometry[] = [];
    yield* plotParts(parts);
    const plot = new Mesh(merged(parts), o.outdoor);
    plot.castShadow = true;
    plot.receiveShadow = true;
    yield;

    const glows = merged([
      ...LAMPS.flatMap((l) => [flare(l.x, PLATE_TOP + STREET_LAMP_H + 0.5, l.z, 3.4, "#ffe2c0", 0.4), pool(l.x, PLATE_TOP - 0.02, l.z, 8, 8, "#ffd9b0", 0.26)]),
    ]);
    const lamps = new Mesh(glows, o.thrown);
    lamps.renderOrder = 1;
    const globes = new Mesh(merged(LAMPS.map((l) => moved(tint(new SphereGeometry(0.24, 12, 9), OFFICE.glow), l.x, PLATE_TOP + STREET_LAMP_H + 0.5, l.z))), flat);
    // The rooms' light on the ground in front, and on the lawn round it.
    const spill = merged([
      pool(LIFT_W / 2, PLATE_TOP + 0.01, ROOM_D / 2 + 2, ROOM_W + LIFT_W + 5, 6.4, "#ffb066", 0.34),
      pool(ROOM_W / 2 + LIFT_W + 2, PLATE_TOP + 0.01, 0, 6, ROOM_D + 4, "#ffb066", 0.24),
    ]);
    const spilled = new Mesh(spill, o.thrown);
    spilled.renderOrder = 1;
    light.add(spilled);
    group.add(plot, lamps, globes);
    owned.push(plot.geometry, glows, globes.geometry, spill);
    yield;

    const rows: BufferGeometry[] = [];
    yield* townParts(rows);
    const town = new Mesh(merged(rows), flat);
    far.add(town);
    owned.push(town.geometry);
  }

  return { group, light, far, looks, sky, rest, dispose() { for (const x of owned) x.dispose(); } };
}
