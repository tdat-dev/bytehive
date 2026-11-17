// Goldens for native/src/office3d lens.rs and stage.rs, from scene.ts and three's camera.
import { writeFileSync } from "node:fs";
import { PerspectiveCamera, Vector3 } from "three";
import { cubicBezier, easeLevel, pickFrom } from "D:/bytehive-worktrees/office/src/company/office/scene";
const ks = [-0.1, 0, 0.05, 0.2, 0.37, 0.5, 0.81, 0.99, 1, 1.2];
const curves = [[0.2, 0, 0, 1], [0.4, 0, 0.2, 1], [0.25, 0.1, 0.25, 1]].map((c) => ({ c, at: ks.map((k) => (cubicBezier as any)(...c)(k)) }));
const ease = ks.map((k) => easeLevel(k));
const shots = [
  { px: 60, py: 22, pz: 52, tx: 1, ty: 8, tz: -1, fov: 15, ox: 0 },
  { px: 40, py: 14, pz: 30, tx: 2, ty: 5.5, tz: 0.3, fov: 15, ox: 0.18 },
  { px: 10, py: 6, pz: 9, tx: -1.6, ty: 9.7, tz: 1.4, fov: 22, ox: -0.05 },
];
const points = [[0, 0, 0], [3, 4.5, -2], [-5, 12, 3], [6.05, 16, -2.55], [100, 0, 0]];
const lenses = shots.flatMap((s) => [[1280, 800], [640, 900]].map(([w, h]) => {
  const c = new PerspectiveCamera(15, w / h, 1, 900);
  c.position.set(s.px, s.py, s.pz);
  c.fov = s.fov;
  if (s.ox) c.setViewOffset(w, h, -s.ox * w, 0, w, h); else c.clearViewOffset();
  c.updateProjectionMatrix();
  c.lookAt(s.tx, s.ty, s.tz);
  c.updateMatrixWorld();
  return { s, w, h, proj: Array.from(c.projectionMatrix.elements), view: Array.from(c.matrixWorldInverse.elements), points: points.map((p) => new Vector3(...p).project(c).toArray()) };
}));
const H = (o: any) => ({ distance: 1, ...o });
const picks = [
  [],
  [H({ floorId: "a", distance: 10 })],
  [H({ floorId: "a", distance: 10 }), H({ personId: "ann", floorId: "a", distance: 12 }), H({ floorId: "b", distance: 11 })],
  [H({ floorId: "b", distance: 9 }), H({ personId: "ann", floorId: "a", distance: 12 })],
  [H({ floorId: "a", distance: 10 }), H({ personId: "bo", floorId: "b", over: true, distance: 30 })],
  [H({ personId: "founder", distance: 5 }), H({ floorId: "a", distance: 10 })],
].map((hits) => ({ hits, got: pickFrom(hits as any) }));
writeFileSync(process.argv[2], JSON.stringify({ curves, ease, lenses, picks }));
console.log("ok");
