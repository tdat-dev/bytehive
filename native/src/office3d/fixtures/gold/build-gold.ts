// Goldens for native/src/office3d build_parts.rs and surfaces.rs, from the office's own build.ts.
import { writeFileSync } from "node:fs";
import { Vector3 } from "three";
import * as B from "OFFICE/build";
import { rgb, type RoomLight } from "OFFICE/light";
const dump = (h: any) => { const g = h.index ? h.toNonIndexed() : h; return { pos: Array.from(g.getAttribute("position").array), normal: Array.from(g.getAttribute("normal").array), color: Array.from(g.getAttribute("color").array) }; };
const digest = (h: any) => {
  const g = h.index ? h.toNonIndexed() : h;
  const [p, n, c] = ["position", "normal", "color"].map((k) => g.getAttribute(k).array);
  const sum = (a: ArrayLike<number>, abs = false) => { const s = [0, 0, 0]; for (let i = 0; i < a.length; i += 1) s[i % 3] += abs ? Math.abs(a[i]) : a[i]; return s; };
  const r = (v: number) => Math.round(v * 1e6) / 1e6;
  return [p.length / 3, ...sum(p), ...sum(p, true), ...sum(n), ...sum(c), ...Array.from(p.slice(0, 3)), ...Array.from(p.slice(-3))].map(r);
};
const room: RoomLight = {
  ambient: [0.68, 0.62, 0.55],
  lamps: [{ x: -3.1, y: 2.6, z: 0.1, colour: rgb("#ffe0bd"), power: 1.05, reach: 2.2 }, { x: 3.1, y: 2.6, z: 0.1, colour: rgb("#ffe0bd"), power: 1.05, reach: 2.2 }],
  blockers: [{ x: 0.2, z: 0.1, w: 1.5, d: 0.74, dark: 0.58 }, { x: -1.4, z: 1.2, w: 2.6, d: 1, dark: 0.5 }],
};
const s = B.shellGeometry();
const lights = B.stringLights([-2, 3, -1], [2.5, 3.2, 1.5], 6);
const out = {
  slab: digest(s.slab), shell: digest(s.shell), desk: digest(B.deskGeometry()), chair: digest(B.chairGeometry()), chair_back: digest(B.chairBackGeometry()), board: digest(B.boardGeometry()),
  things: [0, 1, 2, 3].map((k) => digest(B.deskThing(k))),
  stickies: [B.stickies(null), B.stickies({ todo: 3, doing: 2, done: 1 }), B.stickies({ todo: 20, doing: 1, done: 7 }), B.stickies({ todo: 0, doing: 0, done: 40 })],
  notes: B.noteParts({ todo: 5, doing: 4, done: 2 }).map(digest),
  lights: { lit: lights.lit.map(digest), thrown: lights.thrown.map(digest), wire: lights.wire.map(digest) },
  lift: digest(B.liftCar(0.85)),
  wall_lit: digest(B.wallGrid(room, true)), wall_dark: digest(B.wallGrid(room, false)),
  floor_lit: digest(B.hexFloor(room, true, "#c4605a", 12345)), floor_dark: digest(B.hexFloor(room, false, "#3f8f8a", 987654)),
};
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", Object.keys(out).join(" "));
