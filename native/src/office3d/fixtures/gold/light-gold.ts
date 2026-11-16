// Goldens for native/src/office3d light.rs and glow.rs, from the office's own code.
import { writeFileSync } from "node:fs";
import { box, flare, pool, wash } from "OFFICE/geometry";
import { bake, lightAt, shadeAt, rgb, type RoomLight } from "OFFICE/light";
const room: RoomLight = {
  ambient: rgb("#3a3346"),
  lamps: [{ x: 1, y: 2.6, z: 0.5, colour: rgb("#ffd9a0"), power: 1.4, reach: 2.2 }, { x: -3, y: 2.4, z: -1, colour: rgb("#9fb4e6"), power: 0.8, reach: 3 }],
  blockers: [{ x: 0.2, z: 0.1, w: 1.5, d: 0.74, dark: 0.58 }, { x: 0.9, z: 0.6, w: 0.3, d: 0.3, dark: 0.38 }],
};
const dump = (h: any) => { const g = h.index ? h.toNonIndexed() : h; return { pos: Array.from(g.getAttribute("position").array), normal: Array.from(g.getAttribute("normal").array), color: Array.from(g.getAttribute("color").array) }; };
const out = {
  light_at: [lightAt(room, 0, 0.2, 0), lightAt(room, 2, 1, -1)],
  shade_at: [shadeAt(room.blockers, 0.3, 0.2), shadeAt(room.blockers, 1.2, 0.9), shadeAt(room.blockers, 3, 3)],
  baked: dump(bake(box(1.2, 0.8, 0.6, "#c8b49a").translate(0.4, 0.3, 0.2), room)),
  flare: dump(flare(1, 2, 3, 0.8, "#ffd9a0", 0.6)),
  pool: dump(pool(0.5, 0.01, -1, 2.2, 1.4, "#ffcf9a", 0.35)),
  wash: dump(wash(-2, 1.5, -3.3, 3, 2, Math.PI / 2, "#9fb4e6", 0.2)),
};
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", Object.keys(out).join(" "));
