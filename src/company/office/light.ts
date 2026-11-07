/* The office's light, worked out when a floor is built and left on its vertices: pools
 * under the lamps, soft corners, and the dark under anything that stands on the floor.
 * It costs nothing to draw, and it is still there on Low with shadows off. */
import type { BufferGeometry, BufferAttribute } from "three";
import { Color } from "three";
import type { Seat } from "./layout";

export type RGB = [number, number, number];

/** A light in a room. `reach` is where it has fallen to half. */
export interface Lamp { x: number; y: number; z: number; colour: RGB; power: number; reach: number }
/** Something on the floor the light does not get under. */
export interface Blocker { x: number; z: number; w: number; d: number; dark: number }
export interface RoomLight { ambient: RGB; lamps: Lamp[]; blockers: Blocker[] }

export const rgb = (hex: string): RGB => { const c = new Color(hex); return [c.r, c.g, c.b]; };

const smooth = (a: number, b: number, x: number) => { const k = Math.min(1, Math.max(0, (x - a) / (b - a))); return k * k * (3 - 2 * k); };

/** How much of the room's light gets to this point, in each colour. */
export function lightAt(room: RoomLight, x: number, y: number, z: number): RGB {
  const out: RGB = [...room.ambient];
  for (const l of room.lamps) {
    const d2 = (l.x - x) ** 2 + (l.y - y) ** 2 + (l.z - z) ** 2;
    const k = l.power / (1 + d2 / (l.reach * l.reach));
    out[0] += l.colour[0] * k;
    out[1] += l.colour[1] * k;
    out[2] += l.colour[2] * k;
  }
  return out;
}

/** 1 in the open, down to `1 - dark` under a blocker, with a soft edge round it. */
export function shadeAt(blockers: Blocker[], x: number, z: number): number {
  let lit = 1;
  for (const b of blockers) {
    const dx = Math.max(0, Math.abs(x - b.x) - b.w / 2);
    const dz = Math.max(0, Math.abs(z - b.z) - b.d / 2);
    lit *= 1 - b.dark * (1 - smooth(0, 0.62, Math.hypot(dx, dz)));
  }
  return lit;
}

/** Puts a room's light on what stands in it. What lies on the floor takes the blockers too. */
export function bake(g: BufferGeometry, room: RoomLight): BufferGeometry {
  const pos = g.getAttribute("position") as BufferAttribute;
  const colour = g.getAttribute("color") as BufferAttribute;
  for (let i = 0; i < pos.count; i += 1) {
    const x = pos.getX(i);
    const y = pos.getY(i);
    const z = pos.getZ(i);
    const [r, gg, b] = lightAt(room, x, y, z);
    const low = y < 0.5 ? 1 + (shadeAt(room.blockers, x, z) - 1) * (1 - y / 0.5) : 1;
    colour.setXYZ(i, colour.getX(i) * r * low, colour.getY(i) * gg * low, colour.getZ(i) * b * low);
  }
  return g;
}

/** Where a desk and its chair keep the light off the floor. */
export const deskBlockers = (d: Seat, chair: { x: number; z: number }): Blocker[] => [
  { x: d.x, z: d.z, w: 1.5, d: 0.74, dark: 0.58 },
  { x: chair.x, z: chair.z, w: 0.3, d: 0.3, dark: 0.38 },
];
