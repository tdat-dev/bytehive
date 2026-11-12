// Corner-for-corner goldens from three.js 0.170 for the Rust ports in
// native/src/office3d/prims.rs. Usage: node prims.mjs <three.module.js> <out.json>
import { writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
const T = await import(pathToFileURL(process.argv[2]).href);
const cases = {
  box: new T.BoxGeometry(1.2, 0.5, 0.8),
  plane: new T.PlaneGeometry(2, 1.5),
  circle: new T.CircleGeometry(0.5, 6),
  cylinder: new T.CylinderGeometry(0.3, 0.5, 1.2, 8, 2),
  cylinder_open: new T.CylinderGeometry(0.2, 0.2, 1, 6, 1, true),
  cone: new T.ConeGeometry(0.4, 0.9, 7),
  sphere: new T.SphereGeometry(0.2, 9, 7),
  sphere_part: new T.SphereGeometry(0.4, 20, 9, -0.35, Math.PI + 0.7, Math.PI / 2 - 0.3, Math.PI / 2.4),
  sphere_cap: new T.SphereGeometry(0.34, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2),
  torus: new T.TorusGeometry(0.34, 0.03, 8, 24),
  torus_arc: new T.TorusGeometry(0.2, 0.038, 6, 12, Math.PI * 1.1),
};
const r = (a) => Array.from(a, (v) => Math.round(v * 1e5) / 1e5);
const out = {};
for (const [k, g] of Object.entries(cases)) {
  const f = g.toNonIndexed();
  out[k] = { pos: r(f.getAttribute("position").array), normal: r(f.getAttribute("normal").array) };
}
writeFileSync(process.argv[3], JSON.stringify(out));
console.log(Object.entries(out).map(([k, v]) => `${k}:${v.pos.length / 3}`).join(" "));
