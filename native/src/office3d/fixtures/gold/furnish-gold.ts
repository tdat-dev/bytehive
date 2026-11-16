// Goldens for native/src/office3d furnish.rs: whole rooms from the office's own props.ts.
import { writeFileSync } from "node:fs";
import { furnished } from "OFFICE/props";
// Each geometry as a digest: corner count, sums of place, |place|, normal and
// colour, and its first and last corner. Small enough to keep in the repo,
// and any corner off moves a sum.
const digest = (h: any) => {
  const g = h.index ? h.toNonIndexed() : h;
  const [p, n, c] = ["position", "normal", "color"].map((k) => g.getAttribute(k).array);
  const sum = (a: ArrayLike<number>, abs = false) => { const s = [0, 0, 0]; for (let i = 0; i < a.length; i += 1) s[i % 3] += abs ? Math.abs(a[i]) : a[i]; return s; };
  const r = (v: number) => Math.round(v * 1e6) / 1e6;
  return [p.length / 3, ...sum(p), ...sum(p, true), ...sum(n), ...sum(c), ...Array.from(p.slice(0, 3)), ...Array.from(p.slice(-3))].map(r);
};
const flat = (list: any[]) => list.map(digest);
const rooms = [
  { id: "tea-shop", index: 0, lit: true, board: false },
  { id: "bakery-site", index: 1, lit: false, board: false },
  { id: "native-gpui", index: 2, lit: false, board: true },
  { id: "bytehive", index: 3, lit: true, board: true },
  { id: "zoldify", index: 4, lit: false, board: false },
  { id: "WhaleloSource", index: 5, lit: true, board: false },
];
const out = rooms.map((r) => {
  const f = furnished(r);
  return { room: r, accent: f.accent, solid: flat(f.solid), lit: flat(f.lit), thrown: flat(f.thrown), lamps: f.lamps, blockers: f.blockers };
});
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", out.map((o) => o.room.id + ":" + o.solid.length + "/" + o.lit.length + "/" + o.thrown.length).join(" "));
