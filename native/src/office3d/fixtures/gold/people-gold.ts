// Goldens for native/src/office3d people_*.rs, from the office's own people.ts.
import { writeFileSync } from "node:fs";
import * as P from "D:/bytehive-worktrees/office/src/company/office/people";
import { hashOf } from "D:/bytehive-worktrees/office/src/company/office/geometry";
const digest = (h: any) => {
  const g = h.index ? h.toNonIndexed() : h;
  const [p, n, c] = ["position", "normal", "color"].map((k) => g.getAttribute(k).array);
  const sum = (a: ArrayLike<number>, abs = false) => { const s = [0, 0, 0]; for (let i = 0; i < a.length; i += 1) s[i % 3] += abs ? Math.abs(a[i]) : a[i]; return s; };
  const r = (v: number) => Math.round(v * 1e6) / 1e6;
  return [p.length / 3, ...sum(p), ...sum(p, true), ...sum(n), ...sum(c), ...Array.from(p.slice(0, 3)), ...Array.from(p.slice(-3))].map(r);
};
const ranks = ["staff", "lead", "founder"];
const props = ["clipboard", "megaphone", "magnifier", "pen", "pencil", "laptop", "none"];
const shirts = ["#4f7fc0", "#c4605a", "#eef0f4", "#5d8a4e"];
const looks = Array.from({ length: 12 }, (_, i) => ({ id: "p" + i + "-x", rank: ranks[i % 3], shirt: shirts[i % 4], hair: ["#2a2e3a", "#7a4a2a", "#d9b26a"][i % 3], prop: props[i % props.length], cli: "claude" }));
const people = looks.map((p: any) => ({ ...p, hairKind: hashOf(p.id) % 6, torso: digest(P.torsoGeometry(p)), legs: digest(P.legsGeometry()), armL: digest(P.armGeometry(p, "L")), armR: digest(P.armGeometry(p, "R")) }));
const states = ["needs", "working", "review", "idle", "stopped"];
const times = [0, 333, 1200, 7100, 9000, 15400];
const poses = states.flatMap((s) => times.flatMap((t) => [0, 1, 2].map((seat) => ({ s, t, seat, pose: P.pose(s as any, t, seat) }))));
const kit: any = { matte: {}, bubble: {}, badge: () => ({}) };
const desks = [{ x: 1.2, z: -0.4, rot: 0 }, { x: -2.1, z: 1.0, rot: Math.PI }, { x: 0.3, z: 0.7, rot: 0.3 }];
const rigs: any[] = [];
for (const s of states) for (const [di, d] of desks.entries()) for (const marked of [false, true]) {
  const p: any = { ...looks[di], state: s, seat: di };
  const who = P.buildPerson(p, kit, marked);
  P.seatPerson(who, d as any, 4.5);
  P.animate(who, 1300, 0.2);
  who.updateMatrixWorld(true);
  const parts = who.userData.parts;
  const torso = parts.trunk.children.find((c: any) => c.name === "torso");
  const m = (o: any) => (o ? Array.from(o.matrixWorld.elements) : null);
  rigs.push({ s, d, marked, visible: who.visible, trunk: m(torso), legs: m(parts.legs), armL: m(parts.armL), armR: m(parts.armR), bubble: parts.bubble && parts.bubble.visible ? m(parts.bubble) : null });
}
const spots = states.flatMap((s) => desks.map((d) => ({ s, d, spot: P.spotOf(d as any, s as any) })));
const hops = [-5, 0, 100, 230, 459, 460, 900].map((ms) => [ms, P.hopAt(ms)]);
const plane = P.planeGeometry();
plane.computeVertexNormals();
const lamp = P.lampGeometry();
const tower: any = { floors: [{ id: "a", y: 0.5, desks: [] }, { id: "b", y: 4.5, desks: [] }], roofY: 8.5, lift: { x: 6.05, z: -2.55, top: 12.5 }, width: 12.1 };
const floorOf = (id: string) => ({ ann: "a", bo: "b", fo: null } as any)[id];
const flights = [["ann", "bo", 1000], ["bo", "fo", 2000], ["ann", "ann", 0], ["ann", "gone", 0]].map(([from, to, now]) => ({ from, to, now, reduced: false, at: P.planeAt(tower, { id: "f", from, to, at: 0 } as any, floorOf, now as number, false) }));
flights.push({ from: "fo", to: "ann", now: 500, reduced: true, at: P.planeAt(tower, { id: "f", from: "fo", to: "ann", at: 0 } as any, floorOf, 500, true) });
const out = {
  people, poses, rigs, spots, hops, bubble: digest(P.bubbleGeometry()),
  laptop: digest(P.deskProp("laptop")), book: digest(P.deskProp("book")), stack3: digest(P.paperStack(3)), stack2: digest(P.paperStack(2, true)),
  lampSolid: digest(lamp.solid), lampLit: digest(lamp.lit), glowWork: digest(P.deskGlow("work")), glowLamp: digest(P.deskGlow("lamp")),
  screens: ["work", "dim", "off"].map((s) => P.screenLight(s as any).toArray()), plane: digest(plane), flights,
};
writeFileSync(process.argv[2], JSON.stringify(out));
console.log("ok", people.map((p) => p.hairKind).join(","));
