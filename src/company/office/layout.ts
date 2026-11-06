/* Where everything stands, in scene units, with no three.js: the tower's geometry and
 * the camera's shots are numbers, so they are tested here and the scene only draws them. */
import type { OfficeModel } from "./model";
import type { Level } from "./nav";

export const STOREY = 3.4;
export const SLAB = 0.42;
export const ROOM_W = 11;
export const ROOM_D = 7.4;
export const LIFT_W = 1.7;
export const GROUND = 0.5;
export const FOV = 24;
/** How long a message is in the air, for the paper plane. */
export const FLIGHT_MS = 2400;
const COLS = 3;
const DESK_W = 3;
const ROW_MAX = 2.6;
const MAX_FRAMED = 6;
const VIEW = { x: 0.62, y: 0.42, z: 1 };
const VIEW_LEN = Math.hypot(VIEW.x, VIEW.y, VIEW.z);

export interface Seat { x: number; z: number; rot: number }
export interface Desk extends Seat { personId: string }
export interface FloorSpot { id: string; y: number; desks: Desk[] }
export interface Tower { floors: FloorSpot[]; roofY: number; lift: { x: number; z: number; top: number }; width: number }
export interface Shot { px: number; py: number; pz: number; tx: number; ty: number; tz: number; fov: number }

/** Two rows of three in the usual floor. A Lead with more people, by hand, gets tighter rows. */
export function seatSpot(seat: number, count: number): Seat {
  const rows = Math.max(2, Math.ceil(count / COLS));
  const dz = Math.min(ROW_MAX, (ROOM_D - 1.6) / rows);
  const col = seat % COLS;
  const row = Math.floor(seat / COLS);
  return { x: (col - 1) * DESK_W, z: (row - (rows - 1) / 2) * dz, rot: row % 2 ? Math.PI : 0 };
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

/** A shot from the fixed viewing angle, far enough to hold a box of this size. */
function shot(tx: number, ty: number, tz: number, w: number, h: number, aspect: number): Shot {
  const half = Math.tan((FOV * Math.PI) / 360);
  const dist = Math.max(h / 2 / half, w / 2 / (half * aspect)) * 1.18;
  const k = dist / VIEW_LEN;
  return { px: tx + VIEW.x * k, py: ty + VIEW.y * k, pz: tz + VIEW.z * k, tx, ty, tz, fov: FOV };
}

export function shotFor(l: Level, t: Tower, aspect: number): Shot {
  if (l.kind === "desk" && !l.floorId) return shot(0, t.roofY + 1.2, 0, ROOM_W * 0.7, STOREY * 1.2, aspect);
  const f = l.kind === "building" ? undefined : t.floors.find((x) => x.id === l.floorId);
  if (f && l.kind === "floor") return shot(0, f.y + 1, 0, t.width, STOREY * 1.3, aspect);
  if (f && l.kind === "desk") {
    const d = f.desks.find((x) => x.personId === l.personId);
    return shot(d?.x ?? 0, f.y + 1, d?.z ?? 0, ROOM_W * 0.62, STOREY * 1.05, aspect);
  }
  const h = Math.min(t.roofY + STOREY, MAX_FRAMED * STOREY + STOREY);
  return shot(0, t.roofY / 2, 0, t.width + 2, h, aspect);
}

export const mixShots = (a: Shot, b: Shot, k: number): Shot => {
  const at = (x: number, y: number) => x + (y - x) * k;
  return { px: at(a.px, b.px), py: at(a.py, b.py), pz: at(a.pz, b.pz), tx: at(a.tx, b.tx), ty: at(a.ty, b.ty), tz: at(a.tz, b.tz), fov: at(a.fov, b.fov) };
};

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
