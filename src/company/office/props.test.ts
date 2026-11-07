import { Color, type BufferGeometry } from "three";
import { describe, expect, it } from "vitest";
import { merged } from "./geometry";
import { risingAt, risingGeometry } from "./life";
import { bake, lightAt, shadeAt, type RoomLight } from "./light";
import { ROOM_D, ROOM_W } from "./layout";
import { OFFICE } from "./palette";
import { accentOf, furnish, roomAmbient } from "./props";

const NAMES = ["bytehive", "gcare", "vanta", "oldsite", "scratch", "project-6", "project-7", "project-8"];
const room = (id: string, index = 0, lit = true, board = false) => furnish({ id, index, lit, board });

/** A room's shape as one string: two rooms that match give the same one. */
const shapeOf = (g: BufferGeometry[]) => {
  const all = merged(g.map((x) => (x.index ? x.toNonIndexed() : x)));
  const p = all.getAttribute("position");
  let sum = 0;
  for (let i = 0; i < p.count; i += 1) sum += p.getX(i) * (i % 7) + p.getZ(i) * (i % 5);
  return `${p.count}:${sum.toFixed(2)}`;
};

describe("furnish", () => {
  it("gives every floor its own room", () => {
    const shapes = NAMES.map((n, i) => shapeOf(room(n, i).solid));
    expect(new Set(shapes).size).toBe(NAMES.length);
  });
  it("gives a floor the same room every time", () => {
    expect(shapeOf(room("gcare", 1).solid)).toBe(shapeOf(room("gcare", 1).solid));
  });
  it("never colours two floors in a row alike", () => {
    for (let i = 1; i < NAMES.length; i += 1) expect(accentOf("same", i)).not.toBe(accentOf("same", i - 1));
  });
  it("puts no honey in a room: honey says it is your turn", { timeout: 30_000 }, () => {
    const honey = new Color(OFFICE.honey);
    for (const [i, n] of NAMES.entries()) {
      for (const lit of [true, false]) {
        const f = room(n, i, lit);
        const all = merged([...f.solid, ...f.lit].map((x) => (x.index ? x.toNonIndexed() : x)));
        const c = all.getAttribute("color");
        let found = 0;
        for (let v = 0; v < c.count; v += 1) if (Math.abs(c.getX(v) - honey.r) + Math.abs(c.getY(v) - honey.g) + Math.abs(c.getZ(v) - honey.b) < 0.01) found += 1;
        expect(found).toBe(0);
      }
    }
  });
  it("lights a working room from three lamps, and leaves an empty one a single lamp", () => {
    expect(room("gcare", 1, true).lamps.length).toBeGreaterThanOrEqual(3);
    expect(room("gcare", 1, false).lamps.length).toBe(1);
  });
  it("keeps what stands on the floor inside the room", () => {
    for (const [i, n] of NAMES.entries()) {
      for (const b of room(n, i).blockers) {
        expect(Math.abs(b.x)).toBeLessThan(ROOM_W / 2 + 0.5);
        expect(Math.abs(b.z)).toBeLessThan(ROOM_D / 2 + 0.5);
      }
    }
  });
  it("keeps a working floor quieter than an empty one, so the people lead", () => {
    const count = (lit: boolean) => NAMES.reduce((n, name, i) => n + room(name, i, lit).blockers.length, 0);
    expect(count(true)).toBeLessThan(count(false) / 2);
  });
  it("leaves the wall to the board when there is one", () => {
    expect(shapeOf(room("bytehive", 0, false, true).solid)).not.toBe(shapeOf(room("bytehive", 0, false, false).solid));
  });
});

describe("the light of a room", () => {
  const lamp = { x: 0, y: 3, z: 0, colour: [1, 1, 1] as [number, number, number], power: 1, reach: 2 };
  const light: RoomLight = { ambient: roomAmbient(true), lamps: [lamp], blockers: [{ x: 4, z: 0, w: 2, d: 2, dark: 0.5 }] };
  it("is brightest under the lamp, and never under the room's ambient", () => {
    expect(lightAt(light, 0, 0, 0)[0]).toBeGreaterThan(lightAt(light, 5, 0, 0)[0]);
    expect(lightAt(light, 500, 0, 0)[0]).toBeCloseTo(roomAmbient(true)[0], 2);
  });
  it("is dark under something, and open a step away", () => {
    expect(shadeAt(light.blockers, 4, 0)).toBeCloseTo(0.5);
    expect(shadeAt(light.blockers, -4, 0)).toBe(1);
    expect(shadeAt(light.blockers, 5.2, 0)).toBeGreaterThan(shadeAt(light.blockers, 5, 0));
  });
  it("goes on what stands in the room, and blockers only near the floor", () => {
    const f = room("gcare", 1).solid[0].clone();
    const before = f.getAttribute("color").getX(0);
    bake(f, light);
    expect(f.getAttribute("color").getX(0)).not.toBe(before);
  });
  it("is warm with the lamps on, and cool left for the night", () => {
    const [r, , b] = roomAmbient(true);
    const [nr, , nb] = roomAmbient(false);
    expect(r).toBeGreaterThan(b);
    expect(nb).toBeGreaterThan(nr);
  });
});

describe("what rises from a desk", () => {
  it("starts at the desk, whichever way it is turned", () => {
    for (const rot of [0, Math.PI]) {
      for (const m of risingAt({ x: 3, z: 1, rot }, 10, 0.3)) {
        expect(Math.hypot(m.x - 3, m.z - 1)).toBeLessThan(0.8);
        expect(m.y).toBeGreaterThan(10.7);
      }
    }
  });
  it("is steam and bytes, each on its own beat", () => {
    const marks = risingAt({ x: 0, z: 0, rot: 0 }, 0, 0.3);
    expect(new Set(marks.map((m) => m.kind)).size).toBe(2);
    expect(new Set(marks.map((m) => m.seed)).size).toBeGreaterThan(3);
  });
  it("is two triangles a mark, in one geometry", () => {
    const marks = risingAt({ x: 0, z: 0, rot: 0 }, 0, 0.3);
    expect(risingGeometry(marks).getAttribute("position").count).toBe(marks.length * 6);
  });
});
