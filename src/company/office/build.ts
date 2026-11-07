/* The building's meshes, as in style-a.webp. Static parts are merged, one mesh per
 * material for each floor, and every colour of a matte part sits on its vertices, so
 * a whole floor of furniture is one draw call. Nothing here reads the page: textures
 * are drawn on an OffscreenCanvas, so this runs in a Worker too. */
import {
  AdditiveBlending, BoxGeometry, BufferAttribute, BufferGeometry, CanvasTexture, Color, ConeGeometry, CylinderGeometry, ExtrudeGeometry, Group,
  Matrix4, Mesh, MeshBasicMaterial, MeshStandardMaterial, PlaneGeometry, Shape, SphereGeometry, SRGBColorSpace, type Material, type Texture,
} from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";
import { mergeGeometries, toCreasedNormals } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import { FOUNDER_DESK, GROUND, LIFT_W, ROOM_D, ROOM_W, SLAB, STOREY, chairSpot, type FloorSpot, type Seat, type Tower } from "./layout";
import type { BoardCounts, OfficeFloor, OfficeModel, OfficeState } from "./model";
import { OFFICE } from "./palette";

const BEVEL = 0.11;
const WALL = 0.34;
const ROOM_H = STOREY - SLAB;
const SLAB_W = ROOM_W + LIFT_W + 0.5;
const SLAB_D = ROOM_D + 0.5;
/** The slab reaches over the lift, so it is not centred on the room. */
const SLAB_X = LIFT_W / 2;
const DESK_TOP = 0.78;
/** The ground plate's top: the lowest slab stands on it. */
const PLATE_TOP = GROUND - SLAB;
const MAX_STICKIES = 12;
const BOARD_X = 3.1;
const BOARD_Y = 1.32;

/** The right edge of a floor, where its tag goes. */
export const FLOOR_EDGE = { x: SLAB_X + SLAB_W / 2, z: -ROOM_D / 2 };
/** What a pick ray tests a floor against. */
export const FLOOR_BOX = { w: SLAB_W, h: STOREY, d: SLAB_D, x: SLAB_X };

/* ---- pure parts ---- */

/** Up to twelve notes for the wall board: a list with cards never vanishes, the rest in proportion. */
export function stickies(b: BoardCounts | null): string[] {
  if (!b) return [];
  const lists: Array<[number, string]> = [[b.todo, OFFICE.noteTodo], [b.doing, OFFICE.noteDoing], [b.done, OFFICE.noteDone]];
  const total = lists.reduce((n, [c]) => n + c, 0);
  if (!total) return [];
  const share = lists.map(([c]) => (total <= MAX_STICKIES ? c : c ? Math.max(1, Math.floor((c * MAX_STICKIES) / total)) : 0));
  const sum = () => share.reduce((a, c) => a + c, 0);
  const longest = () => share.indexOf(Math.max(...share));
  while (sum() > MAX_STICKIES) share[longest()] -= 1;
  while (total > MAX_STICKIES && sum() < MAX_STICKIES) share[longest()] += 1;
  return lists.flatMap(([, colour], i) => Array<string>(share[i]).fill(colour));
}

const CORNERS = [{ x: 4.5, z: -2.9 }, { x: -4.5, z: -2.9 }, { x: -4.5, z: 2.9 }];
const KEEP_CLEAR = 1.7;

/** A free corner for the plant, or none. The back right is behind the wall board when there is one. */
export function plantSpot(desks: Seat[], board: boolean): { x: number; z: number } | null {
  return CORNERS.slice(board ? 1 : 0).find((c) => desks.every((d) => Math.hypot(d.x - c.x, d.z - c.z) > KEEP_CLEAR)) ?? null;
}

/* ---- geometry helpers ---- */

/** Keeps position and normal, and puts the colour on the vertices. `top` paints what faces up. */
function tint(g: BufferGeometry, hex: string, top?: string): BufferGeometry {
  const flat = g.index ? g.toNonIndexed() : g;
  flat.deleteAttribute("uv");
  const normal = flat.getAttribute("normal");
  const body = new Color(hex);
  const up = new Color(top ?? hex);
  const colors = new Float32Array(normal.count * 3);
  for (let i = 0; i < normal.count; i += 1) (normal.getY(i) > 0.9 ? up : body).toArray(colors, i * 3);
  flat.setAttribute("color", new BufferAttribute(colors, 3));
  return flat;
}

const moved = (g: BufferGeometry, x: number, y: number, z: number, turn = 0): BufferGeometry =>
  g.clone().applyMatrix4(new Matrix4().makeRotationY(turn).setPosition(x, y, z));
const merged = (parts: BufferGeometry[]): BufferGeometry => mergeGeometries(parts, false);
const box = (w: number, h: number, d: number, hex: string) => tint(new BoxGeometry(w, h, d), hex);
const soft = (w: number, h: number, d: number, r: number, hex: string) => tint(new RoundedBoxGeometry(w, h, d, 2, r), hex);

/** A plate with rounded corners and a soft edge, its top at y = 0. Most of the toy look. */
export function roundedSlab(w: number, d: number, h: number, r: number): BufferGeometry {
  const hw = w / 2 - BEVEL;
  const hd = d / 2 - BEVEL;
  const q = Math.max(0.01, Math.min(r - BEVEL, hw, hd));
  const s = new Shape();
  s.absarc(hw - q, hd - q, q, 0, Math.PI / 2, false);
  s.absarc(-hw + q, hd - q, q, Math.PI / 2, Math.PI, false);
  s.absarc(-hw + q, -hd + q, q, Math.PI, Math.PI * 1.5, false);
  s.absarc(hw - q, -hd + q, q, Math.PI * 1.5, Math.PI * 2, false);
  const depth = h - BEVEL * 2;
  const g = new ExtrudeGeometry(s, { depth, bevelEnabled: true, bevelSize: BEVEL, bevelThickness: BEVEL, bevelSegments: 3, curveSegments: 8 });
  g.rotateX(-Math.PI / 2);
  g.translate(0, -(depth + BEVEL), 0);
  return flatTopped(g);
}

/** Smooth round the edge, while the top stays one flat face. */
function flatTopped(g: BufferGeometry): BufferGeometry {
  const s = toCreasedNormals(g, 0.9);
  const pos = g.getAttribute("position");
  const normal = s.getAttribute("normal");
  for (let i = 0; i < pos.count; i += 3) {
    const flat = Math.abs(pos.getY(i) - pos.getY(i + 1)) < 1e-5 && Math.abs(pos.getY(i) - pos.getY(i + 2)) < 1e-5;
    if (flat) for (let k = 0; k < 3; k += 1) normal.setXYZ(i + k, 0, pos.getY(i) < -1e-5 ? -1 : 1, 0);
  }
  return s;
}

/* ---- textures ---- */

type Painter = (c: OffscreenCanvasRenderingContext2D, w: number, h: number) => void;

function painted(w: number, h: number, paint: Painter): Texture {
  const canvas = new OffscreenCanvas(w, h);
  paint(canvas.getContext("2d")!, w, h);
  const t = new CanvasTexture(canvas);
  t.colorSpace = SRGBColorSpace;
  return t;
}

const mixed = (a: string, b: string, k: number) => `#${new Color(a).lerp(new Color(b), k).getHexString()}`;

/** Light from the ceiling down, and the corner where it does not reach. u = 0 is the corner. */
const wallPaint = (top: string, bottom: string, corner: number): Painter => (c, w, h) => {
  const down = c.createLinearGradient(0, 0, 0, h);
  for (let i = 0; i <= 8; i += 1) {
    const k = i / 8;
    down.addColorStop(k, mixed(top, bottom, k * k * (3 - 2 * k)));
  }
  c.fillStyle = down;
  c.fillRect(0, 0, w, h);
  const across = c.createLinearGradient(0, 0, w, 0);
  for (let i = 0; i <= 8; i += 1) {
    const k = i / 8;
    const fade = (1 - Math.min(1, k / 0.4)) ** 2;
    across.addColorStop(k, `rgba(10,8,6,${corner * fade})`);
  }
  c.fillStyle = across;
  c.fillRect(0, 0, w, h);
};

/** Warm under the desks, out to the colour of the slab, so the edge of the plane cannot be seen. */
const floorPaint = (centre: string, edge: string): Painter => (c, w, h) => {
  c.fillStyle = edge;
  c.fillRect(0, 0, w, h);
  c.filter = `blur(${w * 0.035}px)`;
  c.fillStyle = centre;
  c.fillRect(w * 0.06, h * 0.06, w * 0.88, h * 0.88);
  c.filter = "none";
};

/** A round pool of light that fades to nothing, for additive planes. */
const poolPaint = (r: number, g: number, b: number, strength: number): Painter => (c, w, h) => {
  const pool = c.createRadialGradient(w / 2, h / 2, 0, w / 2, h / 2, w / 2);
  pool.addColorStop(0, `rgba(${r},${g},${b},${strength})`);
  pool.addColorStop(0.45, `rgba(${r},${g},${b},${strength * 0.42})`);
  pool.addColorStop(1, `rgba(${r},${g},${b},0)`);
  c.fillStyle = pool;
  c.fillRect(0, 0, w, h);
};

/* ---- the kit: everything shared across floors ---- */

export interface Kit {
  matte: MeshStandardMaterial;
  glow: MeshBasicMaterial;
  glass: MeshStandardMaterial;
  wallLit: MeshBasicMaterial;
  wallDark: MeshBasicMaterial;
  floorLit: MeshStandardMaterial;
  floorDark: MeshStandardMaterial;
  pool: MeshBasicMaterial;
  shell: BufferGeometry;
  slab: BufferGeometry;
  walls: BufferGeometry;
  room: BufferGeometry;
  desk: BufferGeometry;
  chair: BufferGeometry;
  plant: BufferGeometry;
  boardFrame: BufferGeometry;
  ground: Group;
  /** The rooms' light on the ground: off when no room is lit. */
  groundLight: Group;
  dispose(): void;
}

function wallPlanes(): BufferGeometry {
  const inset = WALL / 2 + 0.005;
  const back = new PlaneGeometry(ROOM_W - inset, ROOM_H);
  back.translate(inset / 2, ROOM_H / 2, -ROOM_D / 2 + inset);
  const left = new PlaneGeometry(ROOM_D - inset, ROOM_H);
  const uv = left.getAttribute("uv");
  for (let i = 0; i < uv.count; i += 1) uv.setX(i, 1 - uv.getX(i));
  left.rotateY(Math.PI / 2);
  left.translate(-ROOM_W / 2 + inset, ROOM_H / 2, inset / 2);
  return merged([back, left]);
}

function shellGeometry(): { shell: BufferGeometry; slab: BufferGeometry } {
  const slab = moved(tint(roundedSlab(SLAB_W, SLAB_D, SLAB, 0.28), OFFICE.shellOut, OFFICE.slabTop), SLAB_X, 0, 0);
  const wall = (len: number) => tint(roundedSlab(len, WALL, ROOM_H, 0.16), OFFICE.shellOut);
  const back = moved(wall(ROOM_W + WALL), 0, ROOM_H, -ROOM_D / 2);
  const left = moved(wall(ROOM_D + WALL), -ROOM_W / 2, ROOM_H, 0, Math.PI / 2);
  return { slab, shell: merged([slab, back, left]) };
}

function deskGeometry(): BufferGeometry {
  return merged([
    moved(soft(1.5, 0.08, 0.8, 0.03, OFFICE.deskTop), 0, DESK_TOP - 0.04, 0),
    moved(soft(0.46, DESK_TOP - 0.08, 0.72, 0.03, OFFICE.deskBody), -0.5, (DESK_TOP - 0.08) / 2, 0),
    moved(soft(0.46, DESK_TOP - 0.08, 0.72, 0.03, OFFICE.deskBody), 0.5, (DESK_TOP - 0.08) / 2, 0),
    moved(box(0.56, 0.4, 0.04, OFFICE.deskBody), 0, 0.46, -0.3),
    moved(box(0.62, 0.4, 0.05, OFFICE.deskBody), 0, DESK_TOP + 0.31, -0.2),
    moved(box(0.08, 0.12, 0.05, OFFICE.deskBody), 0, DESK_TOP + 0.06, -0.22),
    moved(box(0.28, 0.02, 0.18, OFFICE.deskBody), 0, DESK_TOP + 0.01, -0.22),
  ]);
}

function chairGeometry(): BufferGeometry {
  const parts = [
    moved(soft(0.56, 0.11, 0.54, 0.05, OFFICE.chair), 0, 0.5, 0),
    moved(soft(0.52, 0.62, 0.11, 0.05, OFFICE.chair), 0, 0.87, 0.27),
    moved(tint(new CylinderGeometry(0.04, 0.05, 0.36, 8), OFFICE.chair), 0, 0.28, 0),
  ];
  for (let i = 0; i < 5; i += 1) {
    const turn = (i / 5) * Math.PI * 2;
    parts.push(moved(box(0.32, 0.045, 0.07, OFFICE.chair), Math.cos(turn) * 0.16, 0.1, -Math.sin(turn) * 0.16, turn));
    parts.push(moved(tint(new SphereGeometry(0.05, 6, 4), OFFICE.chair), Math.cos(turn) * 0.3, 0.05, -Math.sin(turn) * 0.3));
  }
  return merged(parts);
}

function plantGeometry(): BufferGeometry {
  const parts = [moved(tint(new CylinderGeometry(0.24, 0.17, 0.4, 14), OFFICE.pot), 0, 0.2, 0)];
  for (let i = 0; i < 5; i += 1) {
    const leaf = tint(new ConeGeometry(0.1, 0.95, 6), OFFICE.plant);
    const lean = i ? 0.28 : 0;
    const turn = (i / 4) * Math.PI * 2;
    leaf.translate(0, 0.47, 0);
    leaf.applyMatrix4(new Matrix4().makeRotationZ(lean));
    parts.push(moved(leaf, 0, 0.36, 0, turn));
  }
  return merged(parts);
}

const boardGeometry = () => box(2.6, 1.3, 0.06, OFFICE.deskTop);

function groundGroup(pool: MeshBasicMaterial, spill: MeshBasicMaterial, matte: MeshStandardMaterial): { group: Group; light: Group; owned: BufferGeometry[] } {
  const group = new Group();
  const plate = moved(tint(roundedSlab(ROOM_W + LIFT_W + 3, ROOM_D + 3, 0.36, 0.5), OFFICE.ground), SLAB_X, PLATE_TOP, 0);
  const plateMesh = new Mesh(plate, matte);
  plateMesh.receiveShadow = true;
  const light = new Group();
  const flat = (material: MeshBasicMaterial, w: number, d: number, x: number, y: number, z: number) => {
    const g = new PlaneGeometry(w, d).rotateX(-Math.PI / 2).translate(x, y, z);
    const m = new Mesh(g, material);
    m.renderOrder = 1;
    light.add(m);
    return g;
  };
  group.add(plateMesh, light);
  // The rooms' light on the plate in front, and what gets past it onto the floor of the world.
  const patch = flat(pool, ROOM_W + LIFT_W + 2.2, 3.6, SLAB_X, PLATE_TOP + 0.006, ROOM_D / 2 + 0.2);
  const beyond = flat(spill, 44, 34, SLAB_X + 4, PLATE_TOP - 0.4, 8);
  return { group, light, owned: [plate, patch, beyond] };
}

export function createKit(): Kit {
  const textures = [
    painted(256, 256, wallPaint(OFFICE.wallLit, OFFICE.wallDim, 0.55)),
    painted(128, 128, wallPaint(OFFICE.wallDark, OFFICE.floorDark, 0.5)),
    painted(256, 256, floorPaint(OFFICE.floorLit, OFFICE.slabTop)),
    painted(256, 256, floorPaint(OFFICE.floorDark, OFFICE.slabTop)),
    painted(256, 256, floorPaint(OFFICE.floorLit, "#000000")),
    painted(256, 256, poolPaint(255, 170, 80, 0.42)),
  ];
  const [wallLitMap, wallDarkMap, floorLitMap, floorDarkMap, floorGlowMap, poolMap] = textures;

  const floorPlane = new PlaneGeometry(SLAB_W - 0.5, SLAB_D - 0.5);
  floorPlane.rotateX(-Math.PI / 2);
  floorPlane.translate(SLAB_X, 0.004, 0);

  const matte = new MeshStandardMaterial({ vertexColors: true, roughness: 0.78, metalness: 0 });
  const glow = new MeshBasicMaterial({ vertexColors: true, toneMapped: false });
  const glass = new MeshStandardMaterial({ color: OFFICE.glass, transparent: true, opacity: 0.16, roughness: 0.15, metalness: 0, depthWrite: false });
  const wallLit = new MeshBasicMaterial({ map: wallLitMap, toneMapped: false, dithering: true });
  const wallDark = new MeshBasicMaterial({ map: wallDarkMap, toneMapped: false });
  // A lit floor holds some light of its own, which the shadows of the desks do not take away.
  const floorLit = new MeshStandardMaterial({ map: floorLitMap, roughness: 0.82, emissive: 0xffffff, emissiveMap: floorGlowMap, emissiveIntensity: 0.22, dithering: true });
  const floorDark = new MeshStandardMaterial({ map: floorDarkMap, roughness: 0.82 });
  const pool = new MeshBasicMaterial({ map: poolMap, transparent: true, blending: AdditiveBlending, depthWrite: false, toneMapped: false, dithering: true });
  const spill = pool.clone();
  spill.opacity = 0.45;

  const { shell, slab } = shellGeometry();
  const walls = wallPlanes();
  const desk = deskGeometry();
  const chair = chairGeometry();
  const plant = plantGeometry();
  const boardFrame = boardGeometry();
  const { group: ground, light: groundLight, owned } = groundGroup(pool, spill, matte);

  const geometries = [shell, slab, walls, floorPlane, desk, chair, plant, boardFrame, ...owned];
  const materials: Material[] = [matte, glow, glass, wallLit, wallDark, floorLit, floorDark, pool, spill];
  return {
    matte, glow, glass, wallLit, wallDark, floorLit, floorDark, pool, shell, slab, walls, room: floorPlane, desk, chair, plant, boardFrame, ground, groundLight,
    dispose() {
      for (const g of geometries) g.dispose();
      for (const m of materials) m.dispose();
      for (const t of textures) t.dispose();
    },
  };
}

/* ---- what the model decides ---- */

/** Task 7 replaces this with the pose. */
const screenColour = (state: OfficeState | undefined): string => (state === "working" ? OFFICE.screenOn : state === "stopped" ? OFFICE.screenOff : "#3b5c7a");

function screen(d: Seat, state: OfficeState | undefined): BufferGeometry {
  const plane = tint(new PlaneGeometry(0.56, 0.34), screenColour(state));
  plane.translate(0, DESK_TOP + 0.31, -0.2 + 0.028);
  return moved(plane, d.x, 0, d.z, d.rot);
}

function deskParts(d: Seat, kit: Kit): BufferGeometry[] {
  const chair = chairSpot(d);
  return [moved(kit.desk, d.x, 0, d.z, d.rot), moved(kit.chair, chair.x, 0, chair.z, d.rot)];
}

function noteParts(board: BoardCounts | null): BufferGeometry[] {
  return stickies(board).map((colour, i) => {
    const note = tint(new PlaneGeometry(0.34, 0.3), colour);
    note.applyMatrix4(new Matrix4().makeRotationZ((((i * 37) % 7) - 3) * 0.035));
    return moved(note, -0.84 + (i % 4) * 0.56, 0.36 - Math.floor(i / 4) * 0.36, 0.042);
  });
}

function mesh(g: BufferGeometry, m: Material, o: { cast?: boolean; receive?: boolean } = {}): Mesh {
  const out = new Mesh(g, m);
  out.castShadow = !!o.cast;
  out.receiveShadow = !!o.receive;
  out.userData.owned = true;
  return out;
}

/** Frees what was built for one floor or for the top, and leaves the kit alone. */
export function disposeBuilt(group: Group): void {
  group.traverse((o) => { if (o instanceof Mesh && o.userData.owned) (o.geometry as BufferGeometry).dispose(); });
}

export function buildFloor(f: OfficeFloor, spot: FloorSpot, kit: Kit): Group {
  const group = new Group();
  group.position.y = spot.y;

  const shell = new Mesh(kit.shell, kit.matte);
  shell.receiveShadow = true;
  shell.castShadow = true;
  const walls = new Mesh(kit.walls, f.lit ? kit.wallLit : kit.wallDark);
  const room = new Mesh(kit.room, f.lit ? kit.floorLit : kit.floorDark);
  room.receiveShadow = true;
  group.add(shell, walls, room);

  const board = !!f.board;
  const stateOf = (id: string) => f.people.find((p) => p.id === id)?.state;
  const solid = spot.desks.flatMap((d) => deskParts(d, kit));
  const lit = spot.desks.map((d) => screen(d, stateOf(d.personId)));
  if (board) {
    // The slab above shades this wall, and the notes are what the board is for, so on a lit floor they do not take light.
    const z = -ROOM_D / 2 + WALL / 2 + 0.04;
    solid.push(moved(kit.boardFrame, BOARD_X, BOARD_Y, z));
    (f.lit ? lit : solid).push(moved(merged([moved(tint(new PlaneGeometry(2.44, 1.14), OFFICE.cork), 0, 0, 0.034), ...noteParts(f.board)]), BOARD_X, BOARD_Y, z));
  }
  const plant = plantSpot(spot.desks, board);
  if (plant) solid.push(moved(kit.plant, plant.x, 0, plant.z));
  if (f.lit) lit.push(moved(box(ROOM_W * 0.72, 0.05, 0.16, OFFICE.glow), 0, ROOM_H - 0.04, -ROOM_D / 2 + 0.5));
  if (solid.length) group.add(mesh(merged(solid), kit.matte, { cast: true, receive: true }));
  if (lit.length) group.add(mesh(merged(lit), kit.glow));
  return group;
}

/** The roof deck, the penthouse, the lift and the mast: all that depends on the height. */
export function buildTop(m: OfficeModel, t: Tower, kit: Kit): Group {
  const group = new Group();
  const solid: BufferGeometry[] = [];
  const lit: BufferGeometry[] = [];
  const clear: BufferGeometry[] = [];

  // Roof deck.
  const y = t.roofY;
  const roof = new Mesh(kit.slab, kit.matte);
  roof.position.y = y;
  roof.receiveShadow = true;
  roof.castShadow = true;
  group.add(roof);

  // Glass rail on three sides, with a steel top.
  const edge = { l: -ROOM_W / 2 - 0.05, r: SLAB_X + SLAB_W / 2 - 0.3, f: SLAB_D / 2 - 0.3, b: -SLAB_D / 2 + 0.3 };
  const railH = 0.72;
  const spanX = (x0: number, x1: number, z: number) => {
    clear.push(moved(tint(new BoxGeometry(x1 - x0, railH, 0.04), OFFICE.glass), (x0 + x1) / 2, y + railH / 2, z));
    solid.push(moved(box(x1 - x0 + 0.05, 0.05, 0.06, OFFICE.steel), (x0 + x1) / 2, y + railH, z));
  };
  const spanZ = (z0: number, z1: number, x: number) => {
    clear.push(moved(tint(new BoxGeometry(0.04, railH, z1 - z0), OFFICE.glass), x, y + railH / 2, (z0 + z1) / 2));
    solid.push(moved(box(0.06, 0.05, z1 - z0 + 0.05, OFFICE.steel), x, y + railH, (z0 + z1) / 2));
  };
  spanX(edge.l, edge.r, edge.f);
  spanZ(edge.b, edge.f, edge.l);
  spanZ(t.lift.z + LIFT_W / 2 + 0.1, edge.f, edge.r);

  // Penthouse: a box, a flat overhanging roof, a lit door and one lit window.
  const house = { x: 1.2, z: -1.75, w: 6, d: 3.4, h: 2.5 };
  solid.push(moved(tint(roundedSlab(house.w, house.d, house.h, 0.18), OFFICE.shellOut), house.x, y + house.h, house.z));
  solid.push(moved(tint(roundedSlab(house.w + 0.7, house.d + 0.7, 0.36, 0.24), OFFICE.shellOut, OFFICE.slabTop), house.x, y + house.h + 0.34, house.z));
  const front = house.z + house.d / 2;
  const door = { x: house.x - 0.7, h: 1.95, w: 1.1 };
  solid.push(moved(box(door.w + 0.16, door.h + 0.1, 0.05, OFFICE.deskTop), door.x, y + (door.h + 0.1) / 2, front + 0.01));
  lit.push(moved(tint(new PlaneGeometry(door.w * 0.52, door.h - 0.04), OFFICE.glow), door.x + door.w * 0.22, y + door.h / 2, front + 0.04));
  solid.push(moved(box(door.w * 0.44, door.h - 0.04, 0.03, "#7a5534"), door.x - door.w * 0.26, y + door.h / 2, front + 0.05));
  const side = house.x + house.w / 2;
  const pane = { w: 1.5, h: 1.05, z: house.z + 0.1, y: y + 1.5 };
  solid.push(moved(box(0.05, pane.h + 0.16, pane.w + 0.16, OFFICE.deskTop), side + 0.01, pane.y, pane.z));
  lit.push(moved(tint(new PlaneGeometry(pane.w, pane.h).rotateY(Math.PI / 2), OFFICE.glow), side + 0.045, pane.y, pane.z));
  solid.push(moved(box(0.02, pane.h, 0.05, OFFICE.deskTop), side + 0.055, pane.y, pane.z));

  // The Founder's desk, out front.
  solid.push(...deskParts(FOUNDER_DESK, kit).map((g) => moved(g, 0, y, 0)));
  lit.push(moved(screen(FOUNDER_DESK, m.founder?.state), 0, y, 0));

  // The lift: posts from the ground to just over the roof, a glass box, a car parked at the
  // bottom, a lamp at each floor, and the mast from there up to the top of the tower.
  const { x, z, top } = t.lift;
  const half = LIFT_W / 2 - 0.06;
  const bottom = PLATE_TOP;
  const shaftTop = t.roofY + 1.1;
  const height = shaftTop - bottom;
  for (const [dx, dz] of [[-1, -1], [1, -1], [-1, 1], [1, 1]]) {
    solid.push(moved(box(0.1, height, 0.1, OFFICE.steel), x + dx * half, bottom + height / 2, z + dz * half));
  }
  const rings = [...t.floors.map((f) => f.y), t.roofY, shaftTop];
  for (const ry of rings) {
    for (const s of [-1, 1]) {
      solid.push(moved(box(half * 2, 0.08, 0.08, OFFICE.steel), x, ry, z + s * half));
      solid.push(moved(box(0.08, 0.08, half * 2, OFFICE.steel), x + s * half, ry, z));
    }
  }
  clear.push(moved(tint(new BoxGeometry(half * 2, height, half * 2), OFFICE.glass), x, bottom + height / 2, z));
  const car = Math.min(2.1, height - 0.2);
  solid.push(moved(soft(half * 1.7, car, half * 1.7, 0.05, OFFICE.shellEdge), x, bottom + 0.03 + car / 2, z));
  solid.push(moved(box(0.03, car * 0.85, 0.6, OFFICE.steel), x + half * 0.87, bottom + 0.03 + car * 0.46, z));
  for (const f of t.floors) lit.push(moved(box(0.34, 0.1, 0.16, OFFICE.glow), x - half + 0.4, f.y + 2.72, z + half - 0.02));
  const mast = top - shaftTop;
  solid.push(moved(tint(new CylinderGeometry(0.07, 0.1, mast, 8), OFFICE.steel), x + 0.25, shaftTop + mast / 2, z - 0.25));
  solid.push(moved(tint(new CylinderGeometry(0.045, 0.06, mast * 0.72, 8), OFFICE.steel), x - 0.22, shaftTop + mast * 0.36, z + 0.1));

  group.add(mesh(merged(solid), kit.matte, { cast: true, receive: true }));
  group.add(mesh(merged(lit), kit.glow));
  const glass = mesh(merged(clear), kit.glass);
  glass.renderOrder = 2;
  group.add(glass);
  return group;
}
