// Goldens for native/src/office3d desk.rs and floor.rs, from the office's own buildFloor and buildTop.
import { writeFileSync } from "node:fs";
import { PlaneGeometry } from "three";
import * as B from "D:/bytehive-worktrees/office/src/company/office/build";
import * as P from "D:/bytehive-worktrees/office/src/company/office/people";
import { flare } from "D:/bytehive-worktrees/office/src/company/office/geometry";
import { GROUND, LIFT_W, ROOM_D, ROOM_W, STOREY, seatSpot } from "D:/bytehive-worktrees/office/src/company/office/layout";
const digest = (h: any) => {
  const g = h.index ? h.toNonIndexed() : h;
  const [p, n, c] = ["position", "normal", "color"].map((k) => g.getAttribute(k).array);
  const sum = (a: ArrayLike<number>, abs = false) => { const s = [0, 0, 0]; for (let i = 0; i < a.length; i += 1) s[i % 3] += abs ? Math.abs(a[i]) : a[i]; return s; };
  const r = (v: number) => Math.round(v * 1e6) / 1e6;
  return [p.length / 3, ...sum(p), ...sum(p, true), ...sum(n), ...sum(c), ...Array.from(p.slice(0, 3)), ...Array.from(p.slice(-3))].map(r);
};
const mat = (n: string) => ({ n });
const { shell, slab } = B.shellGeometry();
const kit: any = {
  matte: mat("matte"), indoor: mat("indoor"), night: mat("night"), glow: mat("glow"), wall: mat("wall"), glass: mat("glass"), deskLight: mat("deskLight"), hazeLight: mat("hazeLight"), bubble: mat("bubble"), logo: mat("logo"), badge: () => mat("badge"),
  haze: flare(LIFT_W / 2, (STOREY - 0.56) / 2, 0.8, 17, "#ffb070", 0.1), logoPlate: new PlaneGeometry(1, 1), shapes: new Map(), ready: new Map(),
  shell, slab, desk: B.deskGeometry(), chair: B.chairGeometry(), chairBack: B.chairBackGeometry(), boardFrame: B.boardGeometry(), lamp: P.lampGeometry(), glowWork: P.deskGlow("work"), glowLamp: P.deskGlow("lamp"),
  *parts() { /* ready */ },
};
const person = (id: string, rank: string, state: string, seat: number, prop: string) => ({ id, name: id, rank, title: "", department: "", cli: "claude", shirt: "#4f7fc0", hair: "#2a2e3a", prop, state, doing: null, floorId: null, seat, paneId: null, wsId: null });
const floors = [
  { id: "web-app", lit: true, board: { todo: 3, doing: 2, done: 5 }, lead: "lee", papers: 2, index: 0, people: [person("lee", "lead", "review", 0, "clipboard"), person("ann", "staff", "working", 1, "laptop"), person("bo", "staff", "needs", 2, "book"), person("cy", "staff", "idle", 3, "pen")] },
  { id: "docs", lit: false, board: null, lead: null, papers: 1, index: 1, people: [person("dee", "staff", "stopped", 0, "none"), person("eve", "staff", "review", 1, "laptop")] },
  { id: "api", lit: true, board: { todo: 0, doing: 1, done: 0 }, lead: "fay", papers: 3, index: 2, people: [person("fay", "staff", "working", 0, "megaphone"), person("gus", "staff", "idle", 1, "book"), person("hal", "staff", "needs", 2, "none"), person("ivy", "staff", "working", 3, "pencil"), person("jo", "staff", "review", 4, "laptop")] },
];
const out: any = { floors: [], tops: [] };
for (const f of floors) {
  const y = GROUND + f.index * STOREY;
  const ids = [...f.people.map((p) => p.id), "empty"];
  const desks = ids.map((id, i) => ({ personId: id, ...seatSpot(i, ids.length) }));
  const spot: any = { id: f.id, y, desks };
  const model: any = { id: f.id, name: f.id, dir: "", open: true, lit: f.lit, people: f.people, lead: f.lead, needs: 0, papers: f.papers, board: f.board };
  const g = B.buildFloor(model, spot, kit);
  const room = g.children[0] as any;
  const byMat = (group: any) => Object.fromEntries(group.children.filter((c: any) => c.isMesh).map((c: any) => [c.material.n, digest(c.geometry)]));
  const people = g.children.filter((c: any) => c.userData.person).map((c: any) => ({ id: c.userData.person.id, x: c.position.x, y: c.position.y, z: c.position.z, rot: c.userData.rot }));
  out.floors.push({ ...f, y, desks, room: byMat(room), floor: byMat(g), sat: people });
}
for (const n of [1, 3]) {
  const roofY = GROUND + n * STOREY;
  const t: any = { floors: Array.from({ length: n }, (_, i) => ({ id: "f" + i, y: GROUND + i * STOREY, desks: [] })), roofY, lift: { x: ROOM_W / 2 + LIFT_W / 2, z: -ROOM_D / 2 + LIFT_W / 2, top: roofY + STOREY }, width: ROOM_W + LIFT_W };
  for (const state of [null, "working", "needs"]) {
    const founder = state ? { ...person("founder", "founder", state, 0, "pen"), shirt: "#eef0f4" } : null;
    const g = B.buildTop({ founder } as any, t, kit);
    const parts = g.children.slice(1).filter((c: any) => c.isMesh).map((c: any) => [c.material.n, digest(c.geometry)]);
    const who = g.children.find((c: any) => c.userData.person) as any;
    out.tops.push({ n, state, parts: Object.fromEntries(parts), founder: who ? [who.position.x, who.position.y, who.position.z] : null });
  }
}
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", out.floors.map((f: any) => Object.keys(f.room).join("/") + " | " + Object.keys(f.floor).join("/")).join(" ; "));
