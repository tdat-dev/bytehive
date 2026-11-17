// Goldens for native/src/office3d setting.rs, town.rs and sky.rs, from the office's own setting.ts.
import { writeFileSync } from "node:fs";
import * as S from "OFFICE/setting";
const digest = (h: any) => {
  const g = h.index ? h.toNonIndexed() : h;
  const [p, n, c] = ["position", "normal", "color"].map((k) => g.getAttribute(k).array);
  const sum = (a: ArrayLike<number>, abs = false) => { const s = [0, 0, 0]; for (let i = 0; i < a.length; i += 1) s[i % 3] += abs ? Math.abs(a[i]) : a[i]; return s; };
  const r = (v: number) => Math.round(v * 1e6) / 1e6;
  return [p.length / 3, ...sum(p), ...sum(p, true), ...sum(n), ...sum(c), ...Array.from(p.slice(0, 3)), ...Array.from(p.slice(-3))].map(r);
};
const s = S.createSetting({ outdoor: {} as any, thrown: {} as any });
for (const _ of s.rest()) { /* all at once */ }
const [far, floor, light, plot, lamps, globes] = s.group.children as any[];
const stars = S.starField();
const at = Array.from(stars.getAttribute("position").array as Float32Array);
const col = Array.from(stars.getAttribute("color").array as Float32Array);
const out = {
  facing: far.rotation.y,
  field: digest(floor.children[0].geometry), plot: digest(plot.geometry), glows: digest(lamps.geometry), globes: digest(globes.geometry), spill: digest(light.children[0].geometry), town: digest(far.children[0].geometry),
  moon: digest(S.moonGeometry()), bench: digest(S.bench()), lamp: digest(S.streetLamp()),
  stars: { first: [...at.slice(0, 3), ...col.slice(0, 3)], last: [...at.slice(-3), ...col.slice(-3)], sum: [0, 1, 2].map((k) => at.reduce((a, v, i) => (i % 3 === k ? a + v : a), 0)).concat([0, 1, 2].map((k) => col.reduce((a, v, i) => (i % 3 === k ? a + v : a), 0))) },
};
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", Object.keys(out).join(" "), far.children.length);
