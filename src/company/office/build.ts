/* The building's meshes, as in style-a.webp. Static parts are merged, one mesh per
 * material for each floor, and every colour of a matte part sits on its vertices, so
 * a whole floor of furniture is one draw call. The light of a room is worked out here and
 * left on those vertices too. Nothing here reads the page: textures are drawn on an
 * OffscreenCanvas, so this runs in a Worker too. */
import {
  AdditiveBlending, BoxGeometry, BufferAttribute, BufferGeometry, CanvasTexture, Color, CylinderGeometry, DoubleSide, Group, Matrix4, Mesh, MeshBasicMaterial, MeshStandardMaterial,
  PlaneGeometry, SphereGeometry, SRGBColorSpace, Vector3, type Material, type Object3D, type Texture,
} from "three";
import { clay } from "./clay";
import { fallsBack, focusBand, type FocusBand } from "./focus";
import { DESK_TOP, box, flare, hashOf, merged, moved, paint, pool, ramp, roundedSlab, seeded, soft, tint } from "./geometry";
import { FOUNDER_DESK, GROUND, LIFT_W, ROOM_D, ROOM_W, SLAB, STOREY, chairSpot, type FloorSpot, type Seat, type Tower } from "./layout";
import { bake, deskBlockers, lightAt, rgb, shadeAt, type Lamp, type RGB, type RoomLight } from "./light";
import type { BoardCounts, OfficeFloor, OfficeModel, OfficePerson } from "./model";
import { OFFICE } from "./palette";
import { buildPerson, deskGlow, deskProp, lampGeometry, paperDesk, paperStack, planeGeometry, pose, screenLight, seatPerson, type PeopleKit, type Pose } from "./people";
import { ROOM_H, furnish, roomAmbient } from "./props";
import { createSetting } from "./setting";
import { badgeInk, createTextures } from "./textures";

const WALL = 0.34;
const SLAB_W = ROOM_W + LIFT_W + 0.5;
const SLAB_D = ROOM_D + 0.5;
/** The slab reaches over the lift, so it is not centred on the room. */
const SLAB_X = LIFT_W / 2;
/** The ground plate's top: the lowest slab stands on it. */
const PLATE_TOP = GROUND - SLAB;
const MAX_STICKIES = 12;
const BOARD_X = 2.9;
const BOARD_Y = 1.9;
/** Where the board's counts go, off its right edge, on the wall. */
export const BOARD_EDGE = { x: BOARD_X + 1.35, y: BOARD_Y, z: -ROOM_D / 2 + WALL };
/** The roof sign. `logo` sizes the picture to the plate: the cell's radius is 11.2 of the logo's 24. */
const SIGN = { x: 2.6, z: -2, r: 0.78, thick: 0.28, post: 0.36, turn: 0.42, logo: (0.94 * 24) / 11.2 };

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

/* ---- textures ---- */

type Painter = (c: OffscreenCanvasRenderingContext2D, w: number, h: number) => void;

function painted(w: number, h: number, paintIt: Painter): Texture {
  const canvas = new OffscreenCanvas(w, h);
  paintIt(canvas.getContext("2d")!, w, h);
  const t = new CanvasTexture(canvas);
  t.colorSpace = SRGBColorSpace;
  return t;
}

/** A round pool of light that fades to nothing, for additive planes. */
const poolPaint: Painter = (c, w, h) => {
  const pool = c.createRadialGradient(w / 2, h / 2, 0, w / 2, h / 2, w / 2);
  for (let i = 0; i <= 10; i += 1) {
    const k = i / 10;
    pool.addColorStop(k, `rgba(255,255,255,${(1 - k * k * (3 - 2 * k)) ** 2})`);
  }
  c.fillStyle = pool;
  c.fillRect(0, 0, w, h);
};

/* ---- the kit: everything shared across floors ---- */

export interface Kit {
  /** Outside: the shell, the roof, the plot. Cool, and in the dusk. */
  matte: MeshStandardMaterial;
  /** A room with the lamps on, and one left for the night. */
  indoor: MeshStandardMaterial;
  night: MeshStandardMaterial;
  glow: MeshBasicMaterial;
  glass: MeshStandardMaterial;
  shell: BufferGeometry;
  slab: BufferGeometry;
  desk: BufferGeometry;
  chair: BufferGeometry;
  chairBack: BufferGeometry;
  boardFrame: BufferGeometry;
  lamp: { solid: BufferGeometry; lit: BufferGeometry };
  /** Light thrown on things and glowing in the air, in the colour on its vertices. Quality turns it off whole. */
  deskLight: MeshBasicMaterial;
  glowWork: BufferGeometry;
  glowLamp: BufferGeometry;
  /** What a lit room gives to the air in front of it, seen from far off. The eye reads it as a glow. */
  hazeLight: MeshBasicMaterial;
  haze: BufferGeometry;
  /** The paper plane: one shape, as many flights as there are. */
  plane: BufferGeometry;
  paper: MeshBasicMaterial;
  /** Stands over everything, so no slab ever hides that someone needs you. */
  bubble: MeshBasicMaterial;
  logo: MeshBasicMaterial;
  logoPlate: BufferGeometry;
  /** One material for each CLI on a badge. */
  badge(cli: string): Material;
  shapes: PeopleKit["shapes"];
  ground: Group;
  /** The rooms' light on the ground: off when no room is lit. */
  groundLight: Group;
  /** Sky and town, for the small shift a moving mouse gives them. */
  far: Group;
  /** What stays lit at a Floor. */
  focus: FocusBand;
  dispose(): void;
}

function shellGeometry(): { shell: BufferGeometry; slab: BufferGeometry } {
  const slab = moved(tint(roundedSlab(SLAB_W, SLAB_D, SLAB, 0.42, 0.17), OFFICE.shellOut, OFFICE.slabTop), SLAB_X, 0, 0);
  const wall = (len: number) => tint(roundedSlab(len, WALL, ROOM_H, 0.17), OFFICE.shellOut);
  const back = moved(wall(ROOM_W + WALL), 0, ROOM_H, -ROOM_D / 2);
  const left = moved(wall(ROOM_D + WALL), -ROOM_W / 2, ROOM_H, 0, Math.PI / 2);
  return { slab, shell: merged([slab, back, left]) };
}

function deskGeometry(): BufferGeometry {
  return merged([
    moved(soft(1.5, 0.09, 0.8, 0.035, OFFICE.deskTop), 0, DESK_TOP - 0.045, 0),
    moved(soft(0.42, DESK_TOP - 0.09, 0.72, 0.03, OFFICE.deskBody), -0.5, (DESK_TOP - 0.09) / 2, 0),
    moved(soft(0.42, DESK_TOP - 0.09, 0.72, 0.03, OFFICE.deskBody), 0.5, (DESK_TOP - 0.09) / 2, 0),
    moved(box(0.56, 0.4, 0.04, OFFICE.deskBody), 0, 0.46, -0.3),
    moved(soft(0.66, 0.44, 0.05, 0.02, "#22252f"), 0, DESK_TOP + 0.33, -0.2),
    moved(box(0.07, 0.14, 0.05, OFFICE.deskBody), 0, DESK_TOP + 0.07, -0.22),
    moved(soft(0.28, 0.02, 0.18, 0.008, OFFICE.deskBody), 0, DESK_TOP + 0.01, -0.22),
    moved(soft(0.42, 0.018, 0.15, 0.008, "#d6d3cc"), -0.02, DESK_TOP + 0.01, 0.14),
    moved(tint(new CylinderGeometry(0.052, 0.045, 0.1, 10), "#e9e4d8"), -0.52, DESK_TOP + 0.05, 0.14),
  ]);
}

const CHAIR_HINGE = { y: 0.56, z: 0.27 };

/** The back of a chair, apart from the rest, for tipping it. */
const chairBackGeometry = () => soft(0.52, 0.62, 0.11, 0.05, OFFICE.chair).translate(0, 0.31, 0);

function chairGeometry(): BufferGeometry {
  const parts = [
    moved(soft(0.56, 0.11, 0.54, 0.05, OFFICE.chair), 0, 0.5, 0),
    moved(tint(new CylinderGeometry(0.04, 0.05, 0.36, 8), OFFICE.chair), 0, 0.28, 0),
  ];
  for (let i = 0; i < 5; i += 1) {
    const turn = (i / 5) * Math.PI * 2;
    parts.push(moved(box(0.32, 0.045, 0.07, OFFICE.chair), Math.cos(turn) * 0.16, 0.1, -Math.sin(turn) * 0.16, turn));
    parts.push(moved(tint(new SphereGeometry(0.05, 6, 4), OFFICE.chair), Math.cos(turn) * 0.3, 0.05, -Math.sin(turn) * 0.3));
  }
  return merged(parts);
}

const boardGeometry = () => soft(2.6, 1.3, 0.06, 0.02, "#3a2c24");

export function createKit(loaded: () => void = () => {}): Kit {
  const pictures = createTextures(loaded);
  const deskLightMap = painted(128, 128, poolPaint);

  const focus = focusBand();
  const matte = clay({ self: "#8fa0d8", selfLevel: 0.2, rim: "#7f92d8", rimLevel: 0.08, foot: 0 });
  const indoor = clay({ self: "#ffcf9a", selfLevel: 0.34, rim: "#9fb4e6", rimLevel: 0.2, foot: 0.3 });
  indoor.color.set("#fff1d8");
  const night = clay({ self: "#6f7fc0", selfLevel: 0.24, rim: "#7d90d8", rimLevel: 0.12, foot: 0.26 });
  const glow = new MeshBasicMaterial({ vertexColors: true, toneMapped: false, dithering: true });
  const glass = new MeshStandardMaterial({ color: OFFICE.glass, transparent: true, opacity: 0.13, roughness: 0.15, metalness: 0, depthWrite: false });
  const deskLight = new MeshBasicMaterial({ map: deskLightMap, vertexColors: true, transparent: true, blending: AdditiveBlending, depthWrite: false, toneMapped: false, dithering: true });
  for (const m of [matte, indoor, night, glow, glass]) fallsBack(m, focus);
  const hazeLight = deskLight.clone();
  hazeLight.depthTest = false;
  fallsBack(deskLight, focus);
  const haze = flare(LIFT_W / 2, ROOM_H / 2, 0.8, 17, "#ffb070", 0.1);
  const paper = new MeshBasicMaterial({ vertexColors: true, side: DoubleSide, toneMapped: false });
  const bubble = new MeshBasicMaterial({ vertexColors: true, toneMapped: false, transparent: true, depthTest: false, depthWrite: false });
  const logo = fallsBack(new MeshBasicMaterial({ map: pictures.logoTexture(), transparent: true, toneMapped: false }), focus);
  const badges = new Map<string, MeshBasicMaterial>();
  const badge = (cli: string): Material => {
    let m = badges.get(cli);
    if (!m) {
      m = fallsBack(new MeshBasicMaterial({ map: pictures.badgeTexture(cli), color: badgeInk(cli), transparent: true, toneMapped: false }), focus);
      badges.set(cli, m);
    }
    return m;
  };

  const { shell, slab } = shellGeometry();
  const desk = deskGeometry();
  const chair = chairGeometry();
  const chairBack = chairBackGeometry();
  const boardFrame = boardGeometry();
  const lamp = lampGeometry();
  const glowWork = deskGlow("work");
  const glowLamp = deskGlow("lamp");
  const plane = planeGeometry();
  const logoPlate = new PlaneGeometry(SIGN.r * SIGN.logo, SIGN.r * SIGN.logo);
  const setting = createSetting({ outdoor: matte, thrown: deskLight });
  const shapes: NonNullable<PeopleKit["shapes"]> = new Map();

  const geometries = [shell, slab, desk, chair, chairBack, boardFrame, lamp.solid, lamp.lit, glowWork, glowLamp, haze, plane, logoPlate];
  const materials: Material[] = [matte, indoor, night, glow, glass, deskLight, hazeLight, paper, bubble, logo];
  return {
    matte, indoor, night, glow, glass, shell, slab, desk, chair, chairBack, boardFrame, lamp, deskLight, hazeLight, haze, glowWork, glowLamp, plane, paper, bubble, logo, logoPlate, badge, shapes,
    ground: setting.group, groundLight: setting.light, far: setting.far, focus,
    dispose() {
      for (const g of geometries) g.dispose();
      for (const s of shapes.values()) for (const g of Object.values(s)) g.dispose();
      shapes.clear();
      for (const m of [...materials, ...badges.values()]) m.dispose();
      deskLightMap.dispose();
      setting.dispose();
      pictures.dispose();
      badges.clear();
    },
  };
}

/* ---- a room's surfaces, with its light on them ---- */

const BACK_Z = -ROOM_D / 2 + WALL / 2;
const LEFT_X = -ROOM_W / 2 + WALL / 2;
const smooth = (a: number, b: number, x: number) => { const k = Math.min(1, Math.max(0, (x - a) / (b - a))); return k * k * (3 - 2 * k); };
const times = (c: Color, l: RGB) => c.setRGB(c.r * l[0], c.g * l[1], c.b * l[2]);

/** The two walls, as a fine grid with the light on each point: bright under the ceiling,
 *  dark in the corner, and a pool round every lamp. Unlit by anything else. */
function wallGrid(room: RoomLight, lit: boolean): BufferGeometry {
  // Lights set in the ceiling along each wall: they throw arcs down it, and light nothing else.
  const washes: Lamp[] = !lit ? [] : [
    ...[-4, -1.3, 1.4, 4.1].map((x) => ({ x, y: ROOM_H + 0.25, z: BACK_Z + 0.42, colour: rgb("#ffe6c8"), power: 0.92, reach: 0.95 })),
    ...[-2.2, 0.6, 3].map((z) => ({ x: LEFT_X + 0.42, y: ROOM_H + 0.25, z, colour: rgb("#ffe6c8"), power: 0.92, reach: 0.95 })),
  ];
  const onWall: RoomLight = { ...room, lamps: [...room.lamps, ...washes] };
  const top = new Color(lit ? OFFICE.wallLit : OFFICE.wallDark);
  const low = new Color(lit ? OFFICE.wallDim : OFFICE.wallNightLow);
  const inset = 0.005;
  const back = new PlaneGeometry(ROOM_W - WALL / 2, ROOM_H, 44, 14).translate(WALL / 4, ROOM_H / 2, BACK_Z + inset);
  const left = new PlaneGeometry(ROOM_D - WALL / 2, ROOM_H, 30, 14).rotateY(Math.PI / 2).translate(LEFT_X + inset, ROOM_H / 2, WALL / 4);
  const at = (x: number, y: number, z: number) => {
    const corner = Math.min(x - LEFT_X, z - BACK_Z);
    const dark = (1 - 0.5 * (1 - smooth(0, 2, corner))) * (1 - 0.42 * (1 - smooth(0, 0.9, y))) * (1 - 0.3 * smooth(ROOM_H - 0.5, ROOM_H, y));
    const c = low.clone().lerp(top, smooth(ROOM_H * 0.12, ROOM_H * 0.94, y));
    return times(c, lightAt(onWall, x, y, z)).multiplyScalar(dark);
  };
  return paint(tint(merged([back, left]), "#000000"), at);
}

const HEX_R = 0.46;

/** The floor: hexagon tiles, each its own shade, with the room's light on every corner. */
function hexFloor(room: RoomLight, lit: boolean, accent: string, seed: number): BufferGeometry {
  const rand = seeded(seed);
  const x0 = -ROOM_W / 2 + 0.05;
  const x1 = ROOM_W / 2 + LIFT_W;
  const z0 = -ROOM_D / 2 + 0.05;
  const z1 = ROOM_D / 2 + 0.05;
  const tile = new Color(lit ? OFFICE.floorLit : OFFICE.floorDark);
  const spice = new Color(accent).lerp(tile, lit ? 0.62 : 0.72);
  const position: number[] = [];
  const colour: number[] = [];
  const clampX = (x: number) => Math.min(x1, Math.max(x0, x));
  const clampZ = (z: number) => Math.min(z1, Math.max(z0, z));
  const put = (x: number, z: number, c: Color) => {
    const l = lightAt(room, x, 0, z);
    const s = shadeAt(room.blockers, x, z);
    const edge = 1 - 0.34 * (1 - smooth(0, 1.2, Math.min(x - LEFT_X, z - BACK_Z)));
    position.push(x, 0.006, z);
    colour.push(c.r * l[0] * s * edge, c.g * l[1] * s * edge, c.b * l[2] * s * edge);
  };
  const dx = HEX_R * Math.sqrt(3);
  const dz = HEX_R * 1.5;
  const inner = HEX_R * 0.95;
  for (let row = 0, z = z0; z < z1 + HEX_R; row += 1, z += dz) {
    for (let x = x0 + (row % 2 ? dx / 2 : 0); x < x1 + dx; x += dx) {
      const c = (rand() < 0.05 ? spice : tile).clone().multiplyScalar(0.9 + rand() * 0.2);
      for (let k = 0; k < 6; k += 1) {
        const a = Math.PI / 6 + (k * Math.PI) / 3;
        const b = a + Math.PI / 3;
        put(clampX(x), clampZ(z), c);
        put(clampX(x + Math.cos(b) * inner), clampZ(z + Math.sin(b) * inner), c);
        put(clampX(x + Math.cos(a) * inner), clampZ(z + Math.sin(a) * inner), c);
      }
    }
  }
  const g = new BufferGeometry();
  g.setAttribute("position", new BufferAttribute(new Float32Array(position), 3));
  g.setAttribute("color", new BufferAttribute(new Float32Array(colour), 3));
  const count = position.length / 3;
  const normal = new Float32Array(count * 3);
  for (let i = 0; i < count; i += 1) normal[i * 3 + 1] = 1;
  g.setAttribute("normal", new BufferAttribute(normal, 3));

  const grout = new Color(lit ? OFFICE.grout : OFFICE.groutNight);
  const under = paint(tint(new PlaneGeometry(x1 - x0, z1 - z0, 14, 10).rotateX(-Math.PI / 2).translate((x0 + x1) / 2, 0.002, (z0 + z1) / 2), "#000000"), (x, _y, z) =>
    times(grout.clone(), lightAt(room, x, 0, z)).multiplyScalar(shadeAt(room.blockers, x, z)));
  return merged([under, g]);
}

/* ---- what the model decides ---- */

/** A lit screen, and at work a lit panel on its back: from a desk facing the viewer that is all that shows. */
function screen(d: Seat, state: Pose["screen"]): BufferGeometry {
  const colour = `#${screenLight(state).getHexString()}`;
  const parts = [tint(new PlaneGeometry(0.58, 0.36), colour).translate(0, DESK_TOP + 0.33, -0.2 + 0.03)];
  if (state === "work") parts.push(tint(new PlaneGeometry(0.46, 0.28), colour).rotateY(Math.PI).translate(0, DESK_TOP + 0.33, -0.2 - 0.03));
  return moved(merged(parts), d.x, 0, d.z, d.rot);
}

function chairParts(d: Seat, lean: number, kit: Kit): BufferGeometry[] {
  const c = chairSpot(d);
  const back = kit.chairBack.clone().applyMatrix4(new Matrix4().makeRotationX(lean).setPosition(0, CHAIR_HINGE.y, CHAIR_HINGE.z));
  return [moved(kit.chair, c.x, 0, c.z, d.rot), moved(back, c.x, 0, c.z, d.rot)];
}

/** One desk as its person leaves it: what is matte, what holds its own light, and the light it throws. */
function deskFor(d: Seat, p: OfficePerson | undefined, papers: number, kit: Kit): { solid: BufferGeometry[]; lit: BufferGeometry[]; thrown: BufferGeometry[] } {
  const look = p ? pose(p.state, 0, p.seat) : pose("stopped", 0, 0);
  const onDesk = (g: BufferGeometry) => moved(g, d.x, 0, d.z, d.rot);
  const solid = [onDesk(kit.desk), ...chairParts(d, look.lean, kit)];
  const lit = [screen(d, look.screen)];
  const thrown: BufferGeometry[] = [];
  if (look.screen === "work") {
    thrown.push(onDesk(kit.glowWork));
    const front = -0.1;
    thrown.push(flare(d.x + Math.sin(d.rot) * front, DESK_TOP + 0.36, d.z + Math.cos(d.rot) * front, 1.5, OFFICE.screenOn, 0.34));
  }
  const prop = p ? deskProp(p.prop) : null;
  if (prop) solid.push(onDesk(prop));
  if (p) solid.push(onDesk(deskThing(hashOf(p.id))));
  if (papers) solid.push(onDesk(paperStack(papers, !!prop)));
  if (look.lamp) {
    solid.push(onDesk(kit.lamp.solid));
    lit.push(onDesk(kit.lamp.lit));
    thrown.push(onDesk(kit.glowLamp));
  }
  return { solid, lit, thrown };
}

/** Something of their own by the screen, so no two desks match. */
function deskThing(seed: number): BufferGeometry {
  const x = -0.5;
  const z = -0.22;
  const y = DESK_TOP;
  switch (seed % 4) {
    case 0: return merged([moved(tint(new CylinderGeometry(0.07, 0.055, 0.1, 10), "#d9d2c4"), x, y + 0.05, z), moved(tint(new SphereGeometry(0.1, 8, 6), OFFICE.plant), x, y + 0.16, z)]);
    case 1: return merged([moved(box(0.2, 0.26, 0.02, "#2a2730"), x, y + 0.13, z, 0.3), moved(tint(new PlaneGeometry(0.16, 0.21), "#b7c6dc"), x + 0.004, y + 0.13, z + 0.012, 0.3)]);
    case 2: return merged([0, 1, 2].map((i) => moved(box(0.26, 0.05, 0.2, ["#c4605a", "#3f8f8a", "#e9dfcc"][i]), x, y + 0.03 + i * 0.05, z, i * 0.2 - 0.2)));
    default: return merged([moved(tint(new CylinderGeometry(0.06, 0.05, 0.13, 10), "#3a3d4a"), x, y + 0.065, z), moved(box(0.012, 0.18, 0.012, "#c4605a"), x + 0.015, y + 0.15, z), moved(box(0.012, 0.16, 0.012, "#8fb8f2"), x - 0.02, y + 0.14, z + 0.01)]);
  }
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

function person(p: OfficePerson, d: Seat, y: number, kit: PeopleKit, marked: boolean): Object3D {
  const who = buildPerson(p, kit, marked);
  seatPerson(who, d, y);
  return who;
}

/** Frees what was built for one floor or for the top, and leaves the kit alone. */
export function disposeBuilt(group: Group): void {
  group.traverse((o) => { if (o instanceof Mesh && o.userData.owned) (o.geometry as BufferGeometry).dispose(); });
}

/** What does not change while people work: the shell, the room and its light. It takes
 *  far longer to build than the desks, so a floor keeps it when only someone's state moves. */
function buildRoom(f: OfficeFloor, spot: FloorSpot, kit: Kit): { group: Group; light: RoomLight; inRoom: MeshStandardMaterial } {
  const group = new Group();
  const shell = new Mesh(kit.shell, kit.matte);
  shell.receiveShadow = true;
  shell.castShadow = true;
  group.add(shell);

  const index = Math.round((spot.y - GROUND) / STOREY);
  const room = furnish({ id: f.id, index, lit: f.lit, board: !!f.board });
  const inRoom = f.lit ? kit.indoor : kit.night;
  const blockers = [...room.blockers, ...spot.desks.flatMap((d) => deskBlockers(d, chairSpot(d)))];
  const light: RoomLight = { ambient: roomAmbient(f.lit), lamps: room.lamps, blockers };
  const tiles = hexFloor(light, f.lit, room.accent, hashOf(f.id));
  group.add(mesh(merged([bake(merged(room.solid.map((g) => (g.index ? g.toNonIndexed() : g))), light), tiles]), inRoom, { cast: true, receive: true }));
  group.add(mesh(merged([...room.lit, wallGrid(light, f.lit)]), kit.glow));
  if (room.thrown.length) {
    const t = mesh(merged(room.thrown), kit.deskLight);
    t.renderOrder = 1;
    group.add(t);
  }
  if (f.lit) {
    const air = new Mesh(kit.haze, kit.hazeLight);
    air.renderOrder = 2;
    group.add(air);
  }
  return { group, light, inRoom };
}

interface BuiltRoom { key: string; group: Group; light: RoomLight; inRoom: MeshStandardMaterial }

const roomKey = (f: OfficeFloor, spot: FloorSpot) => JSON.stringify([f.id, spot.y, f.lit, !!f.board, spot.desks.map((d) => [d.x, d.z, d.rot])]);

/** A floor. `was` is the floor it replaces: its room moves over if nothing about it changed. */
export function buildFloor(f: OfficeFloor, spot: FloorSpot, kit: Kit, marked: ReadonlySet<string> = new Set(), was?: Group): Group {
  const group = new Group();
  group.position.y = spot.y;

  const key = roomKey(f, spot);
  const kept = was?.userData.room as BuiltRoom | undefined;
  const room: BuiltRoom = kept?.key === key ? kept : { key, ...buildRoom(f, spot, kit) };
  if (was && room === kept) {
    was.remove(room.group);
    was.userData.room = undefined;
  }
  group.userData.room = room;
  group.add(room.group);

  const people: PeopleKit = { matte: room.inRoom, bubble: kit.bubble, badge: kit.badge, shapes: kit.shapes };
  const solid: BufferGeometry[] = [];
  const lit: BufferGeometry[] = [];
  const thrown: BufferGeometry[] = [];
  const papersOn = paperDesk(f);
  for (const d of spot.desks) {
    const p = f.people.find((x) => x.id === d.personId);
    const desk = deskFor(d, p, d.personId === papersOn ? f.papers : 0, kit);
    solid.push(...desk.solid);
    lit.push(...desk.lit);
    thrown.push(...desk.thrown);
    if (p) group.add(person(p, d, 0, people, marked.has(p.id)));
  }
  if (f.board) {
    // The notes are what the board is for, so on a lit floor they do not take light.
    const z = -ROOM_D / 2 + WALL / 2 + 0.04;
    solid.push(moved(kit.boardFrame, BOARD_X, BOARD_Y, z));
    (f.lit ? lit : solid).push(moved(merged([moved(tint(new PlaneGeometry(2.44, 1.14), OFFICE.cork), 0, 0, 0.034), ...noteParts(f.board)]), BOARD_X, BOARD_Y, z));
  }
  if (solid.length) group.add(mesh(bake(merged(solid), room.light), room.inRoom, { cast: true, receive: true }));
  if (lit.length) group.add(mesh(merged(lit), kit.glow));
  if (thrown.length) {
    const t = mesh(merged(thrown), kit.deskLight);
    t.renderOrder = 1;
    group.add(t);
  }
  return group;
}

/** A string of bulbs hanging from a to b. */
function stringLights(a: [number, number, number], b: [number, number, number], bulbs: number): { lit: BufferGeometry[]; thrown: BufferGeometry[]; wire: BufferGeometry[] } {
  const lit: BufferGeometry[] = [];
  const thrown: BufferGeometry[] = [];
  const wire: BufferGeometry[] = [];
  const point = (k: number): [number, number, number] => [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k - Math.sin(k * Math.PI) * 0.5, a[2] + (b[2] - a[2]) * k];
  for (let i = 0; i <= bulbs; i += 1) {
    const k = i / bulbs;
    const p = point(k);
    lit.push(moved(tint(new SphereGeometry(0.07, 8, 6), OFFICE.glow), p[0], p[1] - 0.05, p[2]));
    thrown.push(flare(p[0], p[1] - 0.05, p[2], 0.62, "#ffd8a0", 0.3));
    if (i < bulbs) {
      const q = point((i + 1) / bulbs);
      const len = Math.hypot(q[0] - p[0], q[1] - p[1], q[2] - p[2]);
      const seg = new BoxGeometry(0.018, 0.018, len);
      seg.applyMatrix4(new Matrix4().lookAt(new Vector3(...q), new Vector3(...p), new Vector3(0, 1, 0)));
      wire.push(moved(tint(seg, "#1d1f28"), (p[0] + q[0]) / 2, (p[1] + q[1]) / 2, (p[2] + q[2]) / 2));
    }
  }
  return { lit, thrown, wire };
}

/** The top of the tower but the Founder's desk: all that depends on the height alone. */
function buildRoof(t: Tower, kit: Kit): Group {
  const group = new Group();
  const solid: BufferGeometry[] = [];
  const lit: BufferGeometry[] = [];
  const clear: BufferGeometry[] = [];
  const thrown: BufferGeometry[] = [];

  // Roof deck.
  const y = t.roofY;
  const roof = new Mesh(kit.slab, kit.matte);
  roof.position.y = y;
  roof.receiveShadow = true;
  roof.castShadow = true;
  group.add(roof);

  // Boards underfoot, stopped short of the penthouse.
  const planks = 15;
  const deckW = ROOM_W + LIFT_W - 0.4;
  for (let i = 0; i < planks; i += 1) {
    const z = -ROOM_D / 2 + 0.1 + (i + 0.5) * (ROOM_D / planks);
    solid.push(moved(box(deckW, 0.04, ROOM_D / planks - 0.03, i % 3 ? "#4a3a33" : "#544237"), SLAB_X, y + 0.02, z));
  }

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
  const house = { x: 0.7, z: -1.6, w: 6, d: 3.2, h: 2.7 };
  solid.push(moved(tint(roundedSlab(house.w, house.d, house.h, 0.18), OFFICE.shellOut), house.x, y + house.h, house.z));
  solid.push(moved(tint(roundedSlab(house.w + 0.8, house.d + 0.8, 0.42, 0.28), OFFICE.shellOut, OFFICE.slabTop), house.x, y + house.h + 0.38, house.z));
  const front = house.z + house.d / 2;
  const door = { x: house.x - 1.2, h: 2.1, w: 1.15 };
  solid.push(moved(box(door.w + 0.16, door.h + 0.1, 0.05, OFFICE.deskTop), door.x, y + (door.h + 0.1) / 2, front + 0.01));
  lit.push(moved(tint(new PlaneGeometry(door.w * 0.52, door.h - 0.04), OFFICE.glow), door.x + door.w * 0.22, y + door.h / 2, front + 0.04));
  solid.push(moved(box(door.w * 0.44, door.h - 0.04, 0.03, "#7a5534"), door.x - door.w * 0.26, y + door.h / 2, front + 0.05));
  const paneF = { w: 1.9, h: 1.2, x: house.x + 1.6, y: y + 1.5 };
  solid.push(moved(box(paneF.w + 0.16, paneF.h + 0.16, 0.05, OFFICE.deskTop), paneF.x, paneF.y, front + 0.01));
  lit.push(moved(tint(new PlaneGeometry(paneF.w, paneF.h), OFFICE.glow), paneF.x, paneF.y, front + 0.045));
  solid.push(moved(box(0.05, paneF.h, 0.02, OFFICE.deskTop), paneF.x, paneF.y, front + 0.055));
  const side = house.x + house.w / 2;
  const pane = { w: 1.5, h: 1.05, z: house.z + 0.1, y: y + 1.5 };
  solid.push(moved(box(0.05, pane.h + 0.16, pane.w + 0.16, OFFICE.deskTop), side + 0.01, pane.y, pane.z));
  lit.push(moved(tint(new PlaneGeometry(pane.w, pane.h).rotateY(Math.PI / 2), OFFICE.glow), side + 0.045, pane.y, pane.z));
  solid.push(moved(box(0.02, pane.h, 0.05, OFFICE.deskTop), side + 0.055, pane.y, pane.z));
  thrown.push(pool(door.x + 0.3, y + 0.05, front + 1, 3.4, 2.6, "#ffc98a", 0.5), pool(paneF.x, y + 0.05, front + 1.2, 4, 2.8, "#ffc98a", 0.4));

  // String lights over the deck, from the eaves to two posts at the rail.
  const post = (x: number, z: number) => solid.push(moved(tint(new CylinderGeometry(0.035, 0.045, 3.4, 8), OFFICE.deskBody), x, y + 1.7, z));
  const postL = [edge.l + 0.2, y + 3.35, edge.f - 0.2] as [number, number, number];
  const postR = [edge.r - 0.5, y + 3.35, edge.f - 0.2] as [number, number, number];
  post(postL[0], postL[2]);
  post(postR[0], postR[2]);
  const eaveL = [house.x - house.w / 2 - 0.3, y + house.h + 0.05, front + 0.34] as [number, number, number];
  const eaveR = [house.x + house.w / 2 + 0.3, y + house.h + 0.05, front + 0.34] as [number, number, number];
  for (const s of [stringLights(eaveL, postL, 8), stringLights(postL, postR, 16), stringLights(eaveR, postR, 7)]) {
    solid.push(...s.wire);
    lit.push(...s.lit);
    thrown.push(...s.thrown);
  }

  // Planters along the rail, and two chairs by the far end.
  const planter = (x: number, z: number, w: number, d: number) => {
    solid.push(moved(soft(w, 0.5, d, 0.05, "#3a3a44"), x, y + 0.27, z));
    const rand = seeded(Math.round(x * 13 + z * 7) + 99);
    for (let i = 0; i < w * 3; i += 1) solid.push(moved(tint(new SphereGeometry(0.2 + rand() * 0.12, 8, 6), rand() < 0.5 ? OFFICE.plant : OFFICE.plantDeep), x + (rand() - 0.5) * (w - 0.3), y + 0.62 + rand() * 0.12, z + (rand() - 0.5) * (d - 0.2)));
  };
  planter(4.7, edge.f - 0.42, 2.8, 0.56);
  planter(edge.l + 0.44, -1.2, 0.56, 3);

  // The sign: a thick hexagon on the penthouse roof, up against the sky, turned a little from the view.
  const plate = tint(new CylinderGeometry(SIGN.r, SIGN.r, SIGN.thick, 6), OFFICE.steel).rotateX(Math.PI / 2).rotateY(SIGN.turn);
  const roofTop = y + house.h + 0.38;
  const signY = roofTop + SIGN.post + SIGN.r;
  solid.push(moved(plate, SIGN.x, signY, SIGN.z));
  solid.push(moved(tint(new CylinderGeometry(0.07, 0.09, SIGN.post + 0.4, 10), OFFICE.steel), SIGN.x, roofTop + (SIGN.post + 0.4) / 2, SIGN.z));
  const logo = new Mesh(kit.logoPlate, kit.logo);
  logo.position.set(SIGN.x + Math.sin(SIGN.turn) * (SIGN.thick / 2 + 0.01), signY, SIGN.z + Math.cos(SIGN.turn) * (SIGN.thick / 2 + 0.01));
  logo.rotation.y = SIGN.turn;
  group.add(logo);

  // The lift: posts from the ground to just over the roof, a glass box, a lamp at each floor,
  // and the mast from there up to the top of the tower. The car goes on its own, as it moves.
  const { x, z, top } = t.lift;
  const half = LIFT_W / 2 - 0.06;
  const bottom = PLATE_TOP;
  const shaftTop = t.roofY + 1.3;
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
  for (const f of t.floors) {
    lit.push(moved(box(0.34, 0.1, 0.16, OFFICE.glow), x - half + 0.4, f.y + 3, z + half - 0.02));
    thrown.push(flare(x - half + 0.4, f.y + 3, z + half, 1.3, "#ffd8a0", 0.36));
  }
  const mast = top - shaftTop;
  solid.push(moved(tint(new CylinderGeometry(0.07, 0.1, mast, 8), OFFICE.steel), x + 0.25, shaftTop + mast / 2, z - 0.25));
  solid.push(moved(tint(new CylinderGeometry(0.045, 0.06, mast * 0.72, 8), OFFICE.steel), x - 0.22, shaftTop + mast * 0.36, z + 0.1));
  // One small warm light at the very top, for the aircraft.
  lit.push(moved(tint(new SphereGeometry(0.09, 8, 6), "#ff8a70"), x + 0.25, top + 0.05, z - 0.25));
  thrown.push(flare(x + 0.25, top + 0.05, z - 0.25, 1.1, "#ff7a60", 0.5));

  const car = mesh(liftCar(half), kit.glow);
  car.name = "liftCar";
  car.position.set(x, bottom + 0.05, z);
  car.userData.stops = [bottom + 0.05, ...t.floors.slice(1).map((f) => f.y)];
  group.add(car);

  group.add(mesh(merged(solid.map((g) => (g.index ? g.toNonIndexed() : g))), kit.matte, { cast: true, receive: true }));
  group.add(mesh(merged(lit), kit.glow));
  const glass = mesh(merged(clear), kit.glass);
  glass.renderOrder = 2;
  group.add(glass);
  const glows = mesh(merged(thrown), kit.deskLight);
  glows.renderOrder = 1;
  group.add(glows);
  return group;
}

/** The roof, and the Founder at their desk out front. `was` is the top it replaces. */
export function buildTop(m: OfficeModel, t: Tower, kit: Kit, marked: ReadonlySet<string> = new Set(), was?: Group): Group {
  const group = new Group();
  const key = JSON.stringify([t.roofY, t.floors.map((f) => f.y)]);
  const kept = was?.userData.roof as { key: string; group: Group } | undefined;
  const roof = kept?.key === key ? kept : { key, group: buildRoof(t, kit) };
  if (was && roof === kept) {
    was.remove(roof.group);
    was.userData.roof = undefined;
  }
  group.userData.roof = roof;
  group.add(roof.group);

  const y = t.roofY;
  const desk = deskFor(FOUNDER_DESK, m.founder ?? undefined, 0, kit);
  group.add(mesh(merged(desk.solid.map((g) => moved(g, 0, y, 0))), kit.matte, { cast: true, receive: true }));
  group.add(mesh(merged(desk.lit.map((g) => moved(g, 0, y, 0))), kit.glow));
  if (desk.thrown.length) {
    const glows = mesh(merged(desk.thrown.map((g) => moved(g, 0, y, 0))), kit.deskLight);
    glows.renderOrder = 1;
    group.add(glows);
  }
  if (m.founder) group.add(person(m.founder, FOUNDER_DESK, y, { matte: kit.indoor, bubble: kit.bubble, badge: kit.badge, shapes: kit.shapes }, marked.has(m.founder.id)));
  return group;
}

/** The car, lit from inside: it is the one thing on the outside of the tower that moves. */
function liftCar(half: number): BufferGeometry {
  const w = half * 1.5;
  const h = 2.5;
  const frame = "#23262f";
  const wall = (turn: number) => ramp(tint(new PlaneGeometry(w - 0.12, h - 0.26, 1, 4), "#000000"), "#332824", "#8a6448").rotateY(turn);
  return merged([
    moved(box(w, 0.13, w, frame), 0, 0.065, 0),
    moved(box(w, 0.16, w, frame), 0, h, 0),
    moved(wall(Math.PI / 2), w / 2, h / 2, 0),
    moved(wall(0), 0, h / 2, w / 2),
    moved(box(w * 0.8, 0.05, 0.02, OFFICE.glow), 0, h - 0.13, w / 2 + 0.012),
    moved(box(0.02, 0.05, w * 0.8, OFFICE.glow), w / 2 + 0.012, h - 0.13, 0),
    moved(box(0.03, h, 0.04, frame), w / 2 + 0.01, h / 2, 0),
    moved(box(0.04, h, 0.03, frame), 0, h / 2, w / 2 + 0.01),
    moved(box(0.05, h, 0.05, frame), w / 2, h / 2, w / 2),
  ]);
}
