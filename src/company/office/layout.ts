/* Where everything stands, in scene units, with no three.js: the tower's geometry and
 * the camera's shots are numbers, so they are tested here and the scene only draws them. */
import type { OfficeModel } from "./model";
import type { Level } from "./nav";

export const STOREY = 4.4;
export const SLAB = 0.58;
export const ROOM_W = 11;
export const ROOM_D = 7.4;
export const LIFT_W = 1.7;
export const GROUND = 0.5;
/** A long lens, as in style-a.webp: the walls stay upright, and every floor is seen from the same height. */
export const FOV = 15;
/** How long a message is in the air, for the paper plane. */
export const FLIGHT_MS = 2400;
const COLS = 3;
const DESK_W = 3.45;
/** The two desks of a pair stand this far either side of its middle, backs together. */
const PAIR_Z = 0.44;
const PAIR_ROWS = 3.6;
/** Past this many floors the Building shot keeps the top of the tower and lets the bottom go. */
const MAX_FRAMED = 8;
/** The plot round the tower. */
export const PLOT = { w: 27, d: 23, x: 0.85, z: 0.6 };
/** Measured off style-a.webp: about 50 degrees round from the front, and low. */
export const VIEW = { x: 1, y: 0.2, z: 0.84 };
const VIEW_LEN = Math.hypot(VIEW.x, VIEW.y, VIEW.z);

export interface Seat { x: number; z: number; rot: number }
export interface Desk extends Seat { personId: string }
export interface FloorSpot { id: string; y: number; desks: Desk[] }
export interface Tower { floors: FloorSpot[]; roofY: number; lift: { x: number; z: number; top: number }; width: number }
export interface Shot { px: number; py: number; pz: number; tx: number; ty: number; tz: number; fov: number }

/** The Founder's desk, out on the roof in front of the penthouse, facing the camera. */
export const FOUNDER_DESK: Seat = { x: -1.6, z: 1.4, rot: Math.PI };
const CHAIR_BACK = 0.78;

/** Where the chair stands. At rot 0 it is on the camera's side of the desk, looking away:
 *  rows fill from the back, so most screens can be seen, beside the back of a head. */
export const chairSpot = (d: Seat): { x: number; z: number } => ({ x: d.x + Math.sin(d.rot) * CHAIR_BACK, z: d.z + Math.cos(d.rot) * CHAIR_BACK });

/** The middle pair fills first, so a small team sits where the eye lands. */
const PAIR_COLS = [0, -1, 1];

/** Desks stand in pairs, back to back: one person faces the viewer, the other their screen.
 *  Three pairs in the usual floor. A Lead with more people, by hand, gets a second row of pairs. */
export function seatSpot(seat: number, count: number): Seat {
  const pair = Math.floor(seat / 2);
  const rows = Math.max(1, Math.ceil(Math.ceil(count / 2) / COLS));
  const middle = (Math.floor(pair / COLS) - (rows - 1) / 2) * PAIR_ROWS;
  const front = seat % 2 === 1;
  return { x: PAIR_COLS[pair % COLS] * DESK_W, z: middle + (front ? PAIR_Z : -PAIR_Z), rot: front ? 0 : Math.PI };
}

export function towerLayout(m: OfficeModel): Tower {
  const floors = m.floors.map((f, i): FloorSpot => ({
    id: f.id,
    y: GROUND + i * STOREY,
    desks: f.people.map((p) => ({ personId: p.id, ...seatSpot(p.seat, f.people.length) })),
  }));
  const roofY = GROUND + m.floors.length * STOREY;
  return { floors, roofY, lift: { x: ROOM_W / 2 + LIFT_W / 2, z: -ROOM_D / 2 + LIFT_W / 2, top: roofY + STOREY }, width: ROOM_W + LIFT_W };
}

export interface Point { x: number; y: number; z: number }

/** Toward the camera, and the screen's right and up, in the world: the view never turns. */
const AIM: Point = { x: VIEW.x / VIEW_LEN, y: VIEW.y / VIEW_LEN, z: VIEW.z / VIEW_LEN };
const FLAT = Math.hypot(AIM.x, AIM.z);
const RIGHT: Point = { x: AIM.z / FLAT, y: 0, z: -AIM.x / FLAT };
const UP: Point = { x: (-AIM.x * AIM.y) / FLAT, y: FLAT, z: (-AIM.z * AIM.y) / FLAT };
const dot = (a: Point, b: Point) => a.x * b.x + a.y * b.y + a.z * b.z;

/** A shot from the fixed viewing angle, far enough to hold a box of this size. */
function shot(tx: number, ty: number, tz: number, w: number, h: number, aspect: number): Shot {
  const half = Math.tan((FOV * Math.PI) / 360);
  const dist = Math.max(h / 2 / half, w / 2 / (half * aspect)) * 1.18;
  const k = dist / VIEW_LEN;
  return { px: tx + VIEW.x * k, py: ty + VIEW.y * k, pz: tz + VIEW.z * k, tx, ty, tz, fov: FOV };
}

/** The shot that holds every point, measured as the lens sees them: a corner nearer the
 *  camera takes more of the frame. `room` is the share of the frame the points may fill. */
export function fitShot(points: Point[], aspect: number, room = 0.92): Shot {
  const us = points.map((p) => dot(p, RIGHT));
  const vs = points.map((p) => dot(p, UP));
  const ws = points.map((p) => dot(p, AIM));
  const mid = (xs: number[]) => (Math.min(...xs) + Math.max(...xs)) / 2;
  const [uc, vc, wc] = [mid(us), mid(vs), mid(ws)];
  const half = Math.tan((FOV * Math.PI) / 360) * room;
  const dist = Math.max(...points.map((_, i) => Math.max(Math.abs(us[i] - uc) / (half * aspect), Math.abs(vs[i] - vc) / half) + ws[i] - wc));
  const along = (axis: keyof Point) => RIGHT[axis] * uc + UP[axis] * vc + AIM[axis] * wc;
  const [tx, ty, tz] = [along("x"), along("y"), along("z")];
  return { px: tx + AIM.x * dist, py: ty + AIM.y * dist, pz: tz + AIM.z * dist, tx, ty, tz, fov: FOV };
}

/** What the Building shot must hold: the top of the mast, the roof's corners and the plot.
 *  On a tower too tall to read whole, the roof and the floors under it, down to `MAX_FRAMED`. */
export function towerPoints(t: Tower): Point[] {
  const tall = t.floors.length > MAX_FRAMED;
  const floor = tall ? t.roofY - MAX_FRAMED * STOREY : -0.4;
  const slab = { l: -ROOM_W / 2 - 0.3, r: ROOM_W / 2 + LIFT_W + 0.3, f: ROOM_D / 2 + 0.3, b: -ROOM_D / 2 - 0.3 };
  const plot = { l: slab.l - 2.4, r: slab.r + 2.4, f: slab.f + 2.4, b: slab.b - 2.4 };
  const corners = (b: { l: number; r: number; f: number; b: number }) => [{ x: b.l, z: b.f }, { x: b.r, z: b.f }, { x: b.l, z: b.b }, { x: b.r, z: b.b }];
  return [
    ...corners(slab).map((c) => ({ x: c.x, y: t.roofY, z: c.z })),
    ...corners(tall ? slab : plot).map((c) => ({ x: c.x, y: floor, z: c.z })),
    { x: t.lift.x, y: t.lift.top, z: t.lift.z },
  ];
}

export function shotFor(l: Level, t: Tower, aspect: number): Shot {
  if (l.kind === "desk" && !l.floorId) return shot(FOUNDER_DESK.x, t.roofY + 1.2, FOUNDER_DESK.z, ROOM_W * 0.7, STOREY * 1.05, aspect);
  const f = l.kind === "building" ? undefined : t.floors.find((x) => x.id === l.floorId);
  if (f && l.kind === "floor") return shot(LIFT_W / 2, f.y + 1.5, 0, t.width + 1.4, STOREY * 1.14, aspect);
  if (f && l.kind === "desk") {
    const d = f.desks.find((x) => x.personId === l.personId);
    return shot(d?.x ?? 0, f.y + 1.1, d?.z ?? 0, ROOM_W * 0.62, STOREY * 0.86, aspect);
  }
  return fitShot(towerPoints(t), aspect);
}

export const mixShots = (a: Shot, b: Shot, k: number): Shot => {
  const at = (x: number, y: number) => x + (y - x) * k;
  return { px: at(a.px, b.px), py: at(a.py, b.py), pz: at(a.pz, b.pz), tx: at(a.tx, b.tx), ty: at(a.ty, b.ty), tz: at(a.tz, b.tz), fov: at(a.fov, b.fov) };
};

const LEG_MS = 11_000;
const GLIDE_MS = 4_200;
const stopOf = (leg: number, n: number) => (Math.imul(leg + 1, 2654435761) >>> 8) % n;
const smoothed = (k: number) => k * k * (3 - 2 * k);

/** Where the lift's car is at this time: resting at a floor, or gliding to the next. All from the clock. */
export function liftAt(now: number, stops: number[]): number {
  if (stops.length < 2) return stops[0] ?? 0;
  const leg = Math.floor(now / LEG_MS);
  const from = stops[stopOf(leg, stops.length)];
  const to = stops[stopOf(leg + 1, stops.length)];
  const k = Math.min(1, (now - leg * LEG_MS) / GLIDE_MS);
  return k >= 1 ? to : from + (to - from) * smoothed(k);
}

const CRUISE = 1.6;

/** A paper plane's place at `k` from 0 to 1: out to the lift, along it, in to the floor. */
export function flightPoint(t: Tower, from: string | null, to: string | null, k: number): { x: number; y: number; z: number } {
  const yOf = (id: string | null) => (t.floors.find((f) => f.id === id)?.y ?? t.roofY) + CRUISE;
  const a = yOf(from);
  const b = yOf(to);
  const mid = 0.15;
  if (k < mid) return { x: (t.lift.x * k) / mid, y: a, z: (t.lift.z * k) / mid };
  if (k > 1 - mid) return { x: (t.lift.x * (1 - k)) / mid, y: b, z: (t.lift.z * (1 - k)) / mid };
  return { x: t.lift.x, y: a + ((b - a) * (k - mid)) / (1 - 2 * mid), z: t.lift.z };
}
