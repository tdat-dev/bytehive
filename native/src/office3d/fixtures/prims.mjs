// Corner-for-corner goldens from three.js 0.170 for the Rust ports in
// native/src/office3d/prims.rs. Usage: node prims.mjs <three.module.js> <out.json>
import { writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
const T = await import(pathToFileURL(process.argv[2]).href);
const ex = (rel) => pathToFileURL(process.argv[2].replace(/build[\/]three\.module\.js$/, rel)).href;
const { toCreasedNormals } = await import(ex("examples/jsm/utils/BufferGeometryUtils.js"));
// roundedSlab and flatTopped from the office's geometry.ts, word for word.
function flatTopped(g) {
  const s = toCreasedNormals(g, 0.9);
  const pos = g.getAttribute("position");
  const normal = s.getAttribute("normal");
  for (let i = 0; i < pos.count; i += 3) {
    const flat = Math.abs(pos.getY(i) - pos.getY(i + 1)) < 1e-5 && Math.abs(pos.getY(i) - pos.getY(i + 2)) < 1e-5;
    if (flat) for (let k = 0; k < 3; k += 1) normal.setXYZ(i + k, 0, pos.getY(i) < -1e-5 ? -1 : 1, 0);
  }
  return s;
}
function roundedSlab(w, d, h, r, bevel = 0.14) {
  const hw = w / 2 - bevel, hd = d / 2 - bevel;
  const q = Math.max(0.01, Math.min(r - bevel, hw, hd));
  const s = new T.Shape();
  s.absarc(hw - q, hd - q, q, 0, Math.PI / 2, false);
  s.absarc(-hw + q, hd - q, q, Math.PI / 2, Math.PI, false);
  s.absarc(-hw + q, -hd + q, q, Math.PI, Math.PI * 1.5, false);
  s.absarc(hw - q, -hd + q, q, Math.PI * 1.5, Math.PI * 2, false);
  const depth = Math.max(0.01, h - bevel * 2);
  const g = new T.ExtrudeGeometry(s, { depth, bevelEnabled: true, bevelSize: bevel, bevelThickness: bevel, bevelSegments: 3, curveSegments: 8 });
  g.rotateX(-Math.PI / 2);
  g.translate(0, -(depth + bevel), 0);
  return flatTopped(g);
}
const { RoundedBoxGeometry } = await import(pathToFileURL(process.argv[2].replace(/build[\/]three\.module\.js$/, "examples/jsm/geometries/RoundedBoxGeometry.js")).href);
const cases = {
  box: new T.BoxGeometry(1.2, 0.5, 0.8),
  plane: new T.PlaneGeometry(2, 1.5),
  plane_split: new T.PlaneGeometry(2.5, 1.6, 1, 6),
  circle: new T.CircleGeometry(0.5, 6),
  cylinder: new T.CylinderGeometry(0.3, 0.5, 1.2, 8, 2),
  cylinder_open: new T.CylinderGeometry(0.2, 0.2, 1, 6, 1, true),
  cone: new T.ConeGeometry(0.4, 0.9, 7),
  sphere: new T.SphereGeometry(0.2, 9, 7),
  sphere_part: new T.SphereGeometry(0.4, 20, 9, -0.35, Math.PI + 0.7, Math.PI / 2 - 0.3, Math.PI / 2.4),
  sphere_cap: new T.SphereGeometry(0.34, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2),
  torus: new T.TorusGeometry(0.34, 0.03, 8, 24),
  torus_arc: new T.TorusGeometry(0.2, 0.038, 6, 12, Math.PI * 1.1),
  capsule_body: new T.CapsuleGeometry(0.29, 0.24, 6, 14),
  capsule_leg: new T.CapsuleGeometry(0.125, 0.4, 4, 10),
  box_split: new T.BoxGeometry(1, 1, 1, 3, 3, 3),
  soft_desk: new RoundedBoxGeometry(2.5, 0.92, 0.68, 1, 0.05),
  soft_cushion: new RoundedBoxGeometry(0.3, 0.56, 0.96, 1, 0.13),
  slab_floor: roundedSlab(9.5, 7.2, 0.42, 0.42, 0.17),
  slab_wall: roundedSlab(6, 0.34, 2.6, 0.17),
  slab_lawn: roundedSlab(14, 11, 0.55, 1.6),
};
const r = (a) => Array.from(a, (v) => Math.round(v * 1e5) / 1e5);
const out = {};
for (const [k, g] of Object.entries(cases)) {
  const f = g.toNonIndexed();
  out[k] = { pos: r(f.getAttribute("position").array), normal: r(f.getAttribute("normal").array) };
}
writeFileSync(process.argv[3], JSON.stringify(out));
console.log(Object.entries(out).map(([k, v]) => `${k}:${v.pos.length / 3}`).join(" "));
