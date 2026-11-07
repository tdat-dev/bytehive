import { describe, expect, it } from "vitest";
import type { OfficeModel } from "./model";
import { BUILDING } from "./nav";
import { FOUNDER_DESK, FOV, RIGHT, ROOM_D, ROOM_W, STOREY, chairSpot, flightPoint, liftAt, mixShots, seatSpot, shotFor, towerLayout, towerPoints, type Point, type Shot } from "./layout";

const floor = (id: string, people: string[]) => ({ id, name: id, people: people.map((p, seat) => ({ id: p, seat })) });
const model = (n: number) => ({ founder: { id: "founder" }, floors: Array.from({ length: n }, (_, i) => floor(`f${i}`, i ? ["x"] : ["a", "b"])) }) as unknown as OfficeModel;

describe("seatSpot", () => {
  it.each([1, 2, 6, 9])("keeps %i desks in the room, none on another", (count) => {
    const spots = Array.from({ length: count }, (_, i) => seatSpot(i, count));
    for (const s of spots) {
      expect(Math.abs(s.x)).toBeLessThan(ROOM_W / 2);
      expect(Math.abs(s.z)).toBeLessThan(ROOM_D / 2);
    }
    expect(new Set(spots.map((s) => `${s.x},${s.z}`)).size).toBe(count);
  });
  it("does not move anyone when someone joins a row that fits", () => {
    expect(seatSpot(0, 2)).toEqual(seatSpot(0, 3));
  });
  it.each([[2, 2.3], [4, 2.3], [6, 1.5], [8, 1.1]])("keeps %i heads apart across the screen", (count, gap) => {
    const across = Array.from({ length: count }, (_, i) => { const c = chairSpot(seatSpot(i, count)); return c.x * RIGHT.x + c.z * RIGHT.z; }).sort((a, b) => a - b);
    for (let i = 1; i < across.length; i += 1) expect(across[i] - across[i - 1]).toBeGreaterThan(gap);
  });
  it("sits everyone facing the viewer", () => {
    for (let i = 0; i < 8; i += 1) {
      const d = seatSpot(i, 8);
      expect(chairSpot(d).z).toBeLessThan(d.z);
    }
  });
  it("starts a small team at the middle of the line", () => {
    const across = (d: { x: number; z: number; rot: number }) => { const c = chairSpot(d); return Math.abs(c.x * RIGHT.x + c.z * RIGHT.z); };
    expect(across(seatSpot(0, 4))).toBeLessThan(across(seatSpot(2, 4)));
    expect(across(seatSpot(1, 4))).toBeLessThan(across(seatSpot(3, 4)));
  });
});

describe("towerLayout", () => {
  it("stacks floors from the ground in model order, the roof above them", () => {
    const t = towerLayout(model(3));
    expect(t.floors.map((f) => f.id)).toEqual(["f0", "f1", "f2"]);
    expect(t.floors[1].y - t.floors[0].y).toBe(STOREY);
    expect(t.roofY).toBeGreaterThan(t.floors[2].y);
  });
  it("still has a roof with no floors", () => {
    const t = towerLayout(model(0));
    expect(t.floors).toEqual([]);
    expect(t.roofY).toBeGreaterThan(0);
  });
  it("has a desk per person and the lift runs the full height", () => {
    const t = towerLayout(model(2));
    expect(t.floors[0].desks.map((d) => d.personId)).toEqual(["a", "b"]);
    expect(t.lift.top).toBeGreaterThanOrEqual(t.roofY);
  });
});

describe("shotFor", () => {
  const t = towerLayout(model(4));
  /** Where a point lands in the frame, from -1 to 1 on each axis, by the camera's own axes. */
  function inFrame(s: Shot, p: Point, aspect: number): { x: number; y: number } {
    const back = { x: s.px - s.tx, y: s.py - s.ty, z: s.pz - s.tz };
    const len = Math.hypot(back.x, back.y, back.z);
    const w = { x: back.x / len, y: back.y / len, z: back.z / len };
    const flat = Math.hypot(w.x, w.z);
    const right = { x: w.z / flat, y: 0, z: -w.x / flat };
    const up = { x: (-w.x * w.y) / flat, y: flat, z: (-w.z * w.y) / flat };
    const d = { x: p.x - s.px, y: p.y - s.py, z: p.z - s.pz };
    const dot = (a: Point, b: Point) => a.x * b.x + a.y * b.y + a.z * b.z;
    const half = Math.tan((FOV * Math.PI) / 360);
    return { x: dot(d, right) / (-dot(d, w) * half * aspect), y: dot(d, up) / (-dot(d, w) * half) };
  }

  it("frames the whole tower at the building, wider on a narrow screen", () => {
    const wide = shotFor(BUILDING, t, 2);
    const tall = shotFor(BUILDING, t, 0.7);
    const dist = (s: typeof wide) => Math.hypot(s.px - s.tx, s.py - s.ty, s.pz - s.tz);
    expect(dist(tall)).toBeGreaterThan(dist(wide));
    expect(wide.fov).toBe(FOV);
  });
  it.each([[1, 1.3], [5, 1.34], [5, 2], [8, 1.34], [8, 0.8], [8, 2.2], [12, 1.34]])("keeps the roof, the mast and every corner in frame with %i floors at %f", (n, aspect) => {
    const tower = towerLayout(model(n));
    const s = shotFor(BUILDING, tower, aspect);
    for (const p of towerPoints(tower)) {
      const at = inFrame(s, p, aspect);
      expect(Math.abs(at.x)).toBeLessThanOrEqual(1);
      expect(Math.abs(at.y)).toBeLessThanOrEqual(1);
    }
  });
  it("moves the Building over for what stands on the left, and still holds every corner", () => {
    const s = shotFor(BUILDING, t, 1.4, 0.3);
    expect(s.ox).toBeCloseTo(0.15);
    for (const p of towerPoints(t)) expect(Math.abs(inFrame(s, p, 1.4 * 0.7).x)).toBeLessThanOrEqual(1);
    expect(shotFor({ kind: "floor", floorId: "f1" }, t, 1.4, 0.3).ox).toBe(0);
  });
  it("gives up the bottom of a very tall tower, never the top", () => {
    const eight = towerLayout(model(8));
    const twelve = towerLayout(model(12));
    const dist = (s: ReturnType<typeof shotFor>) => Math.hypot(s.px - s.tx, s.py - s.ty, s.pz - s.tz);
    expect(dist(shotFor(BUILDING, twelve, 1.6))).toBeLessThanOrEqual(dist(shotFor(BUILDING, eight, 1.6)));
    expect(towerPoints(twelve).some((p) => p.y >= twelve.lift.top)).toBe(true);
    expect(Math.min(...towerPoints(twelve).map((p) => p.y))).toBeGreaterThan(twelve.floors[0].y);
  });
  it("comes closer for a floor, closer again for a desk", () => {
    const dist = (s: ReturnType<typeof shotFor>) => Math.hypot(s.px - s.tx, s.py - s.ty, s.pz - s.tz);
    const b = shotFor(BUILDING, t, 1.6);
    const f = shotFor({ kind: "floor", floorId: "f1" }, t, 1.6);
    const d = shotFor({ kind: "desk", personId: "x", floorId: "f1", tab: "terminal" }, t, 1.6);
    expect(dist(f)).toBeLessThan(dist(b));
    expect(dist(d)).toBeLessThan(dist(f));
    expect(f.ty).toBeGreaterThan(t.floors[1].y);
    expect(f.ty).toBeLessThan(t.floors[1].y + STOREY);
  });
  it("frames the roof for the Founder", () => {
    const s = shotFor({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" }, t, 1.6);
    expect(s.ty).toBeGreaterThanOrEqual(t.roofY);
  });
  it("looks at the Founder's desk, not the middle of the roof", () => {
    const s = shotFor({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" }, t, 1.6);
    expect([s.tx, s.tz]).toEqual([FOUNDER_DESK.x, FOUNDER_DESK.z]);
  });
});

describe("chairSpot", () => {
  it("stands on the camera's side of a desk in the back row, so its screen can be seen", () => {
    const c = chairSpot({ x: 2, z: -1, rot: 0 });
    expect(c.x).toBeCloseTo(2);
    expect(c.z).toBeGreaterThan(-1);
  });
  it("swaps sides for the row that faces the other way", () => {
    expect(chairSpot({ x: 2, z: 1, rot: Math.PI }).z).toBeLessThan(1);
  });
});

describe("mixShots", () => {
  it("runs from one shot to the other", () => {
    const a = shotFor(BUILDING, towerLayout(model(1)), 1.6);
    const b = { ...a, px: a.px + 10 };
    expect(mixShots(a, b, 0)).toEqual(a);
    expect(mixShots(a, b, 1).px).toBeCloseTo(b.px);
    expect(mixShots(a, b, 0.5).px).toBeCloseTo(a.px + 5);
  });
});

describe("liftAt", () => {
  const stops = [0, 4, 8, 12];
  it("rests at a floor most of the time, and moves between two", () => {
    const seen = Array.from({ length: 400 }, (_, i) => liftAt(i * 500, stops));
    const resting = seen.filter((y) => stops.includes(y));
    expect(resting.length).toBeGreaterThan(seen.length / 2);
    expect(seen.length - resting.length).toBeGreaterThan(0);
    expect(Math.min(...seen)).toBeGreaterThanOrEqual(0);
    expect(Math.max(...seen)).toBeLessThanOrEqual(12);
  });
  it("is the same at the same time, so nothing is kept between frames", () => {
    expect(liftAt(123456, stops)).toBe(liftAt(123456, stops));
  });
  it("never jumps", () => {
    for (let t = 0; t < 200000; t += 125) expect(Math.abs(liftAt(t + 125, stops) - liftAt(t, stops))).toBeLessThan(2);
  });
  it("stays at the one stop of a one floor tower", () => {
    expect(liftAt(5000, [3])).toBe(3);
  });
});

describe("flightPoint", () => {
  const t = towerLayout(model(3));
  it("leaves one floor, rises along the lift, arrives at the other", () => {
    expect(flightPoint(t, "f0", "f2", 0).y).toBeCloseTo(t.floors[0].y + 1.6);
    expect(flightPoint(t, "f0", "f2", 1).y).toBeCloseTo(t.floors[2].y + 1.6);
    expect(flightPoint(t, "f0", "f2", 0.5).x).toBeCloseTo(t.lift.x);
  });
  it("treats no floor as the roof", () => {
    expect(flightPoint(t, null, "f0", 0).y).toBeCloseTo(t.roofY + 1.6);
  });
});
