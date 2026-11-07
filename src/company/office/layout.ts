/* Where everything stands, in scene units, with no three.js: the tower's geometry and
 * the camera's shots are numbers, so they are tested here and the scene only draws them. */
import type { OfficeModel } from "./model";
import type { Level } from "./nav";

export const STOREY = 4;
export const SLAB = 0.56;
export const ROOM_W = 10.4;
export const ROOM_D = 6.8;
export const LIFT_W = 1.7;
export const GROUND = 0.5;
/** A long lens, as in style-a.webp: the walls stay upright, and every floor is seen from the same height. */
export const FOV = 15;
/** How long a message is in the air, for the paper plane. */
export const FLIGHT_MS = 2400;
/** People stand in one line across the screen, in two rows of the room: the near row on the
 *  left, the far one on the right. Heads never line up, so nobody hides anyone. */
const SEATS = { near: 1.75, far: -1.6, reach: 3.95, widest: 2.4, most: 10 };
/** Past this many floors the Building shot keeps the top of the tower and lets the bottom go. */
const MAX_FRAMED = 8;
/** The plot round the tower. */
export const PLOT = { w: 21, d: 17, x: 0.85, z: 0.4 };
/** Measured off style-a.webp: about 50 degrees round from the front, and low. */
export const VIEW = { x: 1, y: 0.2, z: 0.84 };
const VIEW_LEN = Math.hypot(VIEW.x, VIEW.y, VIEW.z);

/** Toward the camera, and the screen's right and up, in the world: the view never turns. */
const AIM = { x: VIEW.x / VIEW_LEN, y: VIEW.y / VIEW_LEN, z: VIEW.z / VIEW_LEN };
const FLAT = Math.hypot(AIM.x, AIM.z);
export const RIGHT = { x: AIM.z / FLAT, y: 0, z: -AIM.x / FLAT };
const UP = { x: (-AIM.x * AIM.y) / FLAT, y: FLAT, z: (-AIM.z * AIM.y) / FLAT };

export interface Seat { x: number; z: number; rot: number }
export interface Desk extends Seat { personId: string }
export interface FloorSpot { id: string; y: number; desks: Desk[] }
export interface Tower { floors: FloorSpot[]; roofY: number; lift: { x: number; z: number; top: number }; width: number }
/** `ox`: how far the picture slides right, as a share of the frame, to clear what stands on its left. */
export interface Shot { px: number; py: number; pz: number; tx: number; ty: number; tz: number; fov: number; ox: number }

/** The Founder's desk, out on the roof in front of the penthouse, facing the camera. */
export const FOUNDER_DESK: Seat = { x: -1.6, z: 1.4, rot: Math.PI };
const CHAIR_BACK = 0.78;

/** Where the chair stands. At rot 0 it is on the camera's side of the desk, looking away:
 *  rows fill from the back, so most screens can be seen, beside the back of a head. */
export const chairSpot = (d: Seat): { x: number; z: number } => ({ x: d.x + Math.sin(d.rot) * CHAIR_BACK, z: d.z + Math.cos(d.rot) * CHAIR_BACK });

/** How many places a floor of this many is laid out for: a floor keeps its plan while it fills. */
const placesFor = (count: number) => (count <= 4 ? 4 : count <= 6 ? 6 : count <= 8 ? 8 : SEATS.most);

/** From the middle of the line outward, so a small team sits where the eye lands. */
function middleOut(places: number, seat: number): number {
  const half = Math.floor(places / 2);
  return seat % 2 ? half + (seat - 1) / 2 : half - 1 - seat / 2;
}

/** Everyone faces the viewer from behind their desk. Past ten, by hand, a third row in the middle. */
export function seatSpot(seat: number, count: number): Seat {
  const places = placesFor(count);
  if (seat >= SEATS.most) return { x: (((seat - SEATS.most) % 5) - 2) * 2, z: CHAIR_BACK + Math.floor((seat - SEATS.most) / 5) * 0.2, rot: Math.PI };
  const i = middleOut(places, seat);
  const gap = Math.min(SEATS.widest, (2 * SEATS.reach) / (places - 1));
  const across = (i - (places - 1) / 2) * gap;
  const chairZ = i < places / 2 ? SEATS.near : SEATS.far;
  return { x: (across - RIGHT.z * chairZ) / RIGHT.x, z: chairZ + CHAIR_BACK, rot: Math.PI };
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

const dot = (a: Point, b: Point) => a.x * b.x + a.y * b.y + a.z * b.z;

/** A shot from the fixed viewing angle, far enough to hold a box of this size. */
function shot(tx: number, ty: number, tz: number, w: number, h: number, aspect: number): Shot {
  const half = Math.tan((FOV * Math.PI) / 360);
  const dist = Math.max(h / 2 / half, w / 2 / (half * aspect)) * 1.18;
  const k = dist / VIEW_LEN;
  return { px: tx + VIEW.x * k, py: ty + VIEW.y * k, pz: tz + VIEW.z * k, tx, ty, tz, fov: FOV, ox: 0 };
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
  return { px: tx + AIM.x * dist, py: ty + AIM.y * dist, pz: tz + AIM.z * dist, tx, ty, tz, fov: FOV, ox: 0 };
}

/** What the Building shot must hold: the top of the mast, the roof's corners and the plot.
 *  On a tower too tall to read whole, the roof and the floors under it, down to `MAX_FRAMED`. */
export function towerPoints(t: Tower): Point[] {
  const tall = t.floors.length > MAX_FRAMED;
  const floor = tall ? t.roofY - MAX_FRAMED * STOREY : -0.4;
  const slab = { l: -ROOM_W / 2 - 0.3, r: ROOM_W / 2 + LIFT_W + 0.3, f: ROOM_D / 2 + 0.3, b: -ROOM_D / 2 - 0.3 };
  const plot = { l: PLOT.x - PLOT.w / 2, r: PLOT.x + PLOT.w / 2, f: PLOT.z + PLOT.d / 2, b: PLOT.z - PLOT.d / 2 };
  const corners = (b: { l: number; r: number; f: number; b: number }) => [{ x: b.l, z: b.f }, { x: b.r, z: b.f }, { x: b.l, z: b.b }, { x: b.r, z: b.b }];
  return [
    ...corners(slab).map((c) => ({ x: c.x, y: t.roofY, z: c.z })),
    ...corners(tall ? slab : plot).map((c) => ({ x: c.x, y: floor, z: c.z })),
    { x: t.lift.x, y: t.lift.top, z: t.lift.z },
  ];
}

/** A floor's own corners, for the Floor shot: that room, and only as much of its neighbours as the frame leaves over. */
function floorPoints(f: FloorSpot): Point[] {
  const side = [{ x: -ROOM_W / 2, z: ROOM_D / 2 + 0.2 }, { x: ROOM_W / 2 + LIFT_W, z: ROOM_D / 2 + 0.2 }, { x: ROOM_W / 2 + LIFT_W, z: -ROOM_D / 2 }, { x: -ROOM_W / 2, z: -ROOM_D / 2 }];
  return [...side.map((c) => ({ ...c, y: f.y - 0.1 })), ...side.map((c) => ({ ...c, y: f.y + STOREY - SLAB }))];
}

/** `clear`: the share of the frame's width on the left that the Building shot leaves free for the job box. */
export function shotFor(l: Level, t: Tower, aspect: number, clear = 0): Shot {
  if (l.kind === "desk" && !l.floorId) return shot(FOUNDER_DESK.x, t.roofY + 1.2, FOUNDER_DESK.z, ROOM_W * 0.7, STOREY * 1.05, aspect);
  const f = l.kind === "building" ? undefined : t.floors.find((x) => x.id === l.floorId);
  if (f && l.kind === "floor") return fitShot(floorPoints(f), aspect, 1);
  if (f && l.kind === "desk") {
    const d = f.desks.find((x) => x.personId === l.personId);
    return shot(d?.x ?? 0, f.y + 1.1, d?.z ?? 0, ROOM_W * 0.62, STOREY * 0.86, aspect);
  }
  return { ...fitShot(towerPoints(t), aspect * (1 - clear)), ox: clear / 2 };
}

export const mixShots = (a: Shot, b: Shot, k: number): Shot => {
  const at = (x: number, y: number) => x + (y - x) * k;
  return { px: at(a.px, b.px), py: at(a.py, b.py), pz: at(a.pz, b.pz), tx: at(a.tx, b.tx), ty: at(a.ty, b.ty), tz: at(a.tz, b.tz), fov: at(a.fov, b.fov), ox: at(a.ox, b.ox) };
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
