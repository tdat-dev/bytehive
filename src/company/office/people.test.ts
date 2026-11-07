// src/company/office/people.test.ts
import { readFileSync } from "node:fs";
import { Color, Mesh, MeshBasicMaterial, MeshStandardMaterial, Vector3, type BufferAttribute, type Object3D } from "three";
import { describe, expect, it } from "vitest";
import { FLIGHT_MS, chairSpot, towerLayout, type Tower } from "./layout";
import type { Flight, OfficeFloor, OfficePerson, OfficeState } from "./model";
import { demoModel } from "./fixtures";
import { applyPose, buildPerson, deskGlow, deskProp, paperDesk, paperStack, planeAt, planeGeometry, pose, screenLight, seatPerson, spotOf } from "./people";

describe("pose", () => {
  it("has a bubble for needs and only for needs", () => {
    for (const s of ["working", "review", "idle", "stopped"] as const) expect(pose(s, 0, 0).bubble).toBe(false);
    expect(pose("needs", 0, 0).bubble).toBe(true);
  });
  it("stands up with the lamp on when it needs you", () => {
    expect(pose("needs", 0, 0)).toMatchObject({ sit: 0, lamp: true });
  });
  it("types at a lit screen when working, moving with time", () => {
    const a = pose("working", 0, 0);
    const b = pose("working", 125, 0);
    expect(a.screen).toBe("work");
    expect(a.armL).not.toBe(b.armL);
  });
  it("does not type in step with the next desk", () => {
    expect(pose("working", 300, 0).armL).not.toBe(pose("working", 300, 1).armL);
  });
  it("leans back when idle, and holds still", () => {
    expect(pose("idle", 0, 0).lean).toBeGreaterThan(0);
    expect(pose("idle", 0, 0)).toEqual(pose("idle", 5000, 0));
  });
  it("leaves the chair empty and the screen off when stopped", () => {
    expect(pose("stopped", 0, 0)).toMatchObject({ present: false, screen: "off", bubble: false, lamp: false });
  });
});

const person = (over: Partial<OfficePerson> = {}): OfficePerson => ({
  id: "linh", name: "Linh", rank: "staff", title: "Developer", department: "Engineering", cli: "claude", shirt: "#5fd08a", hair: "#2a2e3a",
  prop: "none", state: "working", doing: null, floorId: "bytehive", seat: 1, paneId: null, wsId: null, ...over,
});

const kit = { matte: new MeshStandardMaterial(), bubble: new MeshBasicMaterial(), badge: () => new MeshBasicMaterial() };
const meshes = (o: Object3D) => {
  const out: Mesh[] = [];
  o.traverse((c) => { if (c instanceof Mesh) out.push(c); });
  return out;
};
const named = (o: Object3D, name: string) => o.getObjectByName(name);
const vertexColours = (m: Mesh) => {
  const c = m.geometry.getAttribute("color");
  const seen = new Set<string>();
  for (let i = 0; i < c.count; i += 1) seen.add([c.getX(i), c.getY(i), c.getZ(i)].map((v) => v.toFixed(3)).join());
  return seen.size;
};

describe("buildPerson", () => {
  it("builds a bubble for someone who needs you, and for nobody else", () => {
    for (const state of ["working", "review", "idle", "stopped"] as OfficeState[]) expect(named(buildPerson(person({ state }), kit), "bubble")).toBeUndefined();
    expect(named(buildPerson(person({ state: "needs" }), kit), "bubble")).toBeDefined();
  });
  it("stays a handful of draw calls, so twenty people fit the budget", () => {
    expect(meshes(buildPerson(person(), kit)).length).toBeLessThanOrEqual(5);
  });
  it("marks every mesh as its own to free", () => {
    for (const m of meshes(buildPerson(person({ state: "needs" }), kit))) expect(m.userData.owned).toBe(true);
  });
  it("gives a Lead a band on the left arm, and only the left", () => {
    const staff = buildPerson(person(), kit);
    const lead = buildPerson(person({ rank: "lead" }), kit);
    expect(vertexColours(named(lead, "armL") as Mesh)).toBeGreaterThan(vertexColours(named(staff, "armL") as Mesh));
    expect(vertexColours(named(lead, "armR") as Mesh)).toBe(vertexColours(named(staff, "armR") as Mesh));
  });
  it.each([["clipboard", "armL"], ["megaphone", "armR"], ["magnifier", "armR"], ["pen", "armR"], ["pencil", "armR"]])("puts a %s in the %s hand", (prop, arm) => {
    const bare = named(buildPerson(person(), kit), arm) as Mesh;
    const holding = named(buildPerson(person({ prop }), kit), arm) as Mesh;
    expect(holding.geometry.getAttribute("position").count).toBeGreaterThan(bare.geometry.getAttribute("position").count);
  });
  it.each(["laptop", "book", "none", "something-new"])("keeps a %s out of the hands", (prop) => {
    for (const arm of ["armL", "armR"]) {
      const bare = named(buildPerson(person(), kit), arm) as Mesh;
      const other = named(buildPerson(person({ prop }), kit), arm) as Mesh;
      expect(other.geometry.getAttribute("position").count).toBe(bare.geometry.getAttribute("position").count);
    }
  });
});

describe("applyPose", () => {
  const worldOf = (o: Object3D) => { o.updateWorldMatrix(true, false); return o.getWorldPosition(new Vector3()); };

  it("hides someone who is not there", () => {
    const p = buildPerson(person({ state: "stopped" }), kit);
    applyPose(p, pose("stopped", 0, 0));
    expect(p.visible).toBe(false);
  });
  it("swings the arms as the pose says", () => {
    const p = buildPerson(person(), kit);
    applyPose(p, { ...pose("working", 0, 0), armL: 0, armR: 0 });
    const rest = named(p, "armL")!.rotation.x;
    applyPose(p, { ...pose("working", 0, 0), armL: 0.1, armR: 0 });
    expect(named(p, "armL")!.rotation.x).toBeCloseTo(rest + 0.1);
  });
  it("tips the top half back with the lean, and leaves the legs", () => {
    const p = buildPerson(person({ state: "idle" }), kit);
    applyPose(p, pose("idle", 0, 0));
    expect(named(p, "trunk")!.rotation.x).toBeCloseTo(pose("idle", 0, 0).lean);
    expect(named(p, "legs")!.parent).toBe(named(p, "body"));
  });
  it("takes the hands off the desk to lean back", () => {
    const p = buildPerson(person(), kit);
    applyPose(p, { ...pose("working", 0, 0), armL: 0, armR: 0 });
    const typing = named(p, "armL")!.rotation.x;
    applyPose(p, pose("idle", 0, 0));
    expect(named(p, "armL")!.rotation.x).toBeLessThan(typing);
  });
  it("stands higher than it sits, out of the chair", () => {
    const p = buildPerson(person(), kit);
    seatPerson(p, { x: 0, z: 0, rot: 0 }, 0);
    applyPose(p, pose("working", 0, 0));
    const sat = worldOf(named(p, "body")!).clone();
    applyPose(p, pose("needs", 0, 0));
    const stood = worldOf(named(p, "body")!);
    expect(stood.y).toBeGreaterThan(sat.y);
    expect(stood.distanceTo(sat)).toBeGreaterThan(0.5);
  });
  it("faces the desk sitting down, whichever way the desk is turned", () => {
    for (const rot of [0, Math.PI]) {
      const p = buildPerson(person(), kit);
      seatPerson(p, { x: 0, z: 0, rot }, 0);
      applyPose(p, pose("working", 0, 0));
      expect(named(p, "body")!.rotation.y).toBeCloseTo(rot);
    }
  });
  it("turns to the viewer on its feet, whichever way the desk is turned", () => {
    const facing = [0, Math.PI].map((rot) => {
      const p = buildPerson(person({ state: "needs" }), kit);
      seatPerson(p, { x: 0, z: 0, rot }, 0);
      applyPose(p, pose("needs", 0, 0));
      p.updateWorldMatrix(true, true);
      return new Vector3(0, 0, -1).transformDirection(named(p, "body")!.matrixWorld);
    });
    for (const f of facing) {
      expect(f.x).toBeGreaterThan(0.5);
      expect(f.z).toBeGreaterThan(0.5);
    }
  });
});

describe("spotOf", () => {
  const desk = { x: 3, z: -1, rot: 0 };
  it("is the chair for someone sitting", () => {
    expect(spotOf(desk, "working")).toMatchObject(chairSpot(desk));
  });
  it("is where the body went for someone standing, so a click on them lands", () => {
    const p = buildPerson(person({ state: "needs" }), kit);
    seatPerson(p, desk, 0);
    applyPose(p, pose("needs", 0, 0));
    const at = named(p, "body")!.getWorldPosition(new Vector3());
    const s = spotOf(desk, "needs");
    expect(s.x).toBeCloseTo(at.x);
    expect(s.z).toBeCloseTo(at.z);
  });
  it("knows where the bubble is, over the head", () => {
    const p = buildPerson(person({ state: "needs" }), kit);
    seatPerson(p, desk, 0);
    const s = spotOf(desk, "needs");
    expect(named(p, "bubble")!.getWorldPosition(new Vector3()).y).toBeCloseTo(s.bubble);
    expect(s.bubble).toBeGreaterThan(s.head);
  });
  it.each([0, Math.PI])("stands clear of the desk and the chair, with the desk turned %s", (rot) => {
    const d = { x: 0, z: 0, rot };
    const s = spotOf(d, "needs");
    const c = chairSpot(d);
    expect(Math.abs(s.x) > 1.05 || Math.abs(s.z) > 0.7).toBe(true);
    expect(Math.hypot(s.x - c.x, s.z - c.z)).toBeGreaterThan(0.6);
  });
  it("steps toward the viewer from the back row, past the line of the slab above", () => {
    const d = { x: 0, z: 0, rot: 0 };
    expect(spotOf(d, "needs").z).toBeGreaterThan(chairSpot(d).z + 0.5);
  });
  it("holds a standing head higher", () => {
    expect(spotOf(desk, "needs").head).toBeGreaterThan(spotOf(desk, "working").head);
  });
});

describe("deskProp", () => {
  it.each(["laptop", "book"])("sets a %s on the desk", (prop) => { expect(deskProp(prop)).not.toBeNull(); });
  it.each(["clipboard", "megaphone", "magnifier", "pen", "pencil", "none", "something-new"])("sets no %s on the desk", (prop) => { expect(deskProp(prop)).toBeNull(); });
});

describe("the review papers", () => {
  const floor = (over: Partial<OfficeFloor>): OfficeFloor => ({ id: "f", name: "f", dir: "", open: true, lit: true, people: [], lead: null, needs: 0, papers: 0, board: null, ...over });
  it("go on the Lead's desk", () => {
    expect(paperDesk(floor({ lead: "hai", papers: 1, people: [person({ id: "hai" }), person({ id: "minh", state: "review" })] }))).toBe("hai");
  });
  it("stay on the person's own desk with no Lead", () => {
    expect(paperDesk(floor({ papers: 1, people: [person({ id: "linh" }), person({ id: "minh", state: "review" })] }))).toBe("minh");
  });
  it("go nowhere when nothing is in review", () => {
    expect(paperDesk(floor({ lead: "hai", people: [person({ id: "hai" })] }))).toBeNull();
  });
  it("keep off a prop on the desk", () => {
    const x = (g: ReturnType<typeof paperStack>) => { g.computeBoundingBox(); return g.boundingBox!; };
    const laptop = x(deskProp("laptop")!);
    expect(x(paperStack(1)).max.x).toBeGreaterThan(laptop.min.x);
    expect(x(paperStack(1, true)).max.x).toBeLessThan(laptop.min.x);
  });
  it("stack as many sheets as there are papers", () => {
    const heightOf = (n: number) => { const g = paperStack(n); g.computeBoundingBox(); return g.boundingBox!.max.y - g.boundingBox!.min.y; };
    expect(heightOf(3)).toBeGreaterThan(heightOf(1));
  });
});

describe("the light a desk throws", () => {
  const colourOf = (g: ReturnType<typeof deskGlow>) => new Color().fromBufferAttribute(g.getAttribute("color") as BufferAttribute, 0);
  it("is the screen's blue at work, and the lamp's amber", () => {
    expect(colourOf(deskGlow("work")).b).toBeGreaterThan(colourOf(deskGlow("work")).r);
    expect(colourOf(deskGlow("lamp")).r).toBeGreaterThan(colourOf(deskGlow("lamp")).b);
  });
  it("lies on the desk top, and keeps the uv its picture needs", () => {
    for (const kind of ["work", "lamp"] as const) {
      const g = deskGlow(kind);
      g.computeBoundingBox();
      expect(g.boundingBox!.min.y).toBeGreaterThan(0.78);
      expect(g.boundingBox!.max.y).toBeLessThan(0.8);
      expect(g.boundingBox!.max.x).toBeLessThanOrEqual(0.75);
      expect(g.getAttribute("uv")).toBeDefined();
    }
  });
});

describe("screenLight", () => {
  const glow = (s: "work" | "dim" | "off") => { const c = screenLight(s); return c.r + c.g + c.b; };
  it("is brightest at work, low when dim, and dark when off", () => {
    expect(glow("work")).toBeGreaterThan(glow("dim"));
    expect(glow("dim")).toBeGreaterThan(glow("off"));
  });
});

describe("planeGeometry", () => {
  it("is three triangles, in paper", () => {
    const g = planeGeometry();
    expect(g.getAttribute("position").count).toBe(9);
    expect(g.index).toBeNull();
    expect(g.getAttribute("color")).toBeDefined();
  });
  it("has its nose on +z, the way lookAt turns a mesh", () => {
    const g = planeGeometry();
    g.computeBoundingBox();
    expect(g.boundingBox!.max.z).toBeGreaterThan(-g.boundingBox!.min.z - 1e-6);
  });
});

describe("planeAt", () => {
  const m = demoModel();
  const t: Tower = towerLayout(m);
  const floorOf = (id: string) => (id === "founder" ? null : id === "ghost" ? undefined : m.floors.find((f) => f.people.some((p) => p.id === id))?.id);
  const flight = (from: string, to: string): Flight => ({ id: "x", from, to, at: 1000 });
  const up = flight("tuan", "founder");

  it("is out on the way in the middle of a flight", () => {
    const p = planeAt(t, up, floorOf, 1000 + FLIGHT_MS / 2, false)!;
    expect(p.x).toBeCloseTo(t.lift.x);
    expect(p.dy).toBeGreaterThan(0);
  });
  it("noses the way it goes", () => {
    const out = planeAt(t, up, floorOf, 1000 + FLIGHT_MS * 0.05, false)!;
    expect(out.dx).toBeGreaterThan(0);
    const down = planeAt(t, flight("founder", "tuan"), floorOf, 1000 + FLIGHT_MS / 2, false)!;
    expect(down.dy).toBeLessThan(0);
  });
  it("has a nose at both ends of the flight", () => {
    for (const k of [0, 0.999]) {
      const p = planeAt(t, up, floorOf, 1000 + FLIGHT_MS * k, false)!;
      expect(Math.hypot(p.dx, p.dy, p.dz)).toBeGreaterThan(0);
    }
  });
  it("is gone once it lands, and before it leaves", () => {
    expect(planeAt(t, up, floorOf, 1000 + FLIGHT_MS, false)).toBeNull();
    expect(planeAt(t, up, floorOf, 999, false)).toBeNull();
  });
  it("sits still at its destination for reduced motion, until the flight's time is up", () => {
    const early = planeAt(t, up, floorOf, 1001, true)!;
    const late = planeAt(t, up, floorOf, 1000 + FLIGHT_MS - 1, true)!;
    expect(early).toEqual(late);
    expect(early.y).toBeCloseTo(t.roofY + 1.6);
    expect(early.x).toBeCloseTo(0);
    expect(planeAt(t, up, floorOf, 1000 + FLIGHT_MS, true)).toBeNull();
  });
  it("has no plane on the same floor", () => {
    expect(planeAt(t, flight("linh", "an"), floorOf, 2000, false)).toBeNull();
  });
  it("has no plane for someone who has left", () => {
    expect(planeAt(t, flight("ghost", "founder"), floorOf, 2000, false)).toBeNull();
    expect(planeAt(t, flight("tuan", "ghost"), floorOf, 2000, false)).toBeNull();
  });
});

describe("the scene can move to a Worker", () => {
  it("people.ts reads nothing from the page", () => {
    const code = readFileSync("src/company/office/people.ts", "utf8").replace(/\/\*[\s\S]*?\*\/|\/\/.*$/gm, "");
    expect(code).not.toMatch(/\b(document|window|getComputedStyle)\b/);
  });
  it("never imports the engine", () => {
    for (const file of ["people.ts", "scene.ts", "build.ts", "textures.ts"]) {
      expect(readFileSync(`src/company/office/${file}`, "utf8")).not.toMatch(/from "\.\.\/(engine|org|jobs)/);
    }
  });
});
