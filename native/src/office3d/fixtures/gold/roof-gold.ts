// Goldens for native/src/office3d roof.rs, from the office's own roofSteps.
import { writeFileSync } from "node:fs";
import { PlaneGeometry } from "three";
import * as B from "OFFICE/build";
import { GROUND, LIFT_W, ROOM_D, ROOM_W, STOREY } from "OFFICE/layout";
const digest = (h: any) => {
  const g = h.index ? h.toNonIndexed() : h;
  const [p, n, c] = ["position", "normal", "color"].map((k) => g.getAttribute(k).array);
  const sum = (a: ArrayLike<number>, abs = false) => { const s = [0, 0, 0]; for (let i = 0; i < a.length; i += 1) s[i % 3] += abs ? Math.abs(a[i]) : a[i]; return s; };
  const r = (v: number) => Math.round(v * 1e6) / 1e6;
  return [p.length / 3, ...sum(p), ...sum(p, true), ...sum(n), ...sum(c), ...Array.from(p.slice(0, 3)), ...Array.from(p.slice(-3))].map(r);
};
const out = [1, 3].map((n) => {
  const roofY = GROUND + n * STOREY;
  const t: any = { floors: Array.from({ length: n }, (_, i) => ({ id: "f" + i, y: GROUND + i * STOREY, desks: [] })), roofY, lift: { x: ROOM_W / 2 + LIFT_W / 2, z: -ROOM_D / 2 + LIFT_W / 2, top: roofY + STOREY }, width: ROOM_W + LIFT_W };
  const kit: any = { slab: B.shellGeometry().slab, matte: {}, glow: {}, glass: {}, deskLight: {}, logo: {}, logoPlate: new PlaneGeometry(1, 1) };
  const g = B.finished(B.roofSteps(t, kit));
  const [deck, logo, car, solid, lit, glass, glows] = g.children as any[];
  return {
    floors: n, deck: [deck.position.y, ...digest(deck.geometry)], solid: digest(solid.geometry), lit: digest(lit.geometry), clear: digest(glass.geometry), thrown: digest(glows.geometry),
    logo: [logo.position.x, logo.position.y, logo.position.z, logo.rotation.y], car: [car.position.x, car.position.y, car.position.z], stops: car.userData.stops, car_geo: digest(car.geometry),
  };
});
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", out.length);
