// src/company/office/fixtures.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { demoModel, demoSource, levelFrom } from "./fixtures";
import { BUILDING } from "./nav";

describe("the demo company", () => {
  const m = demoModel();
  const people = [m.founder!, ...m.floors.flatMap((f) => f.people)];
  it("shows every state, so one screenshot judges them all", () => {
    expect(new Set(people.map((p) => p.state))).toEqual(new Set(["needs", "working", "review", "idle", "stopped"]));
  });
  it("has a Lead, a person alone on a floor, a closed floor and an empty open one", () => {
    expect(m.floors.some((f) => f.lead)).toBe(true);
    expect(m.floors.some((f) => f.people.length === 1 && !f.lead)).toBe(true);
    expect(m.floors.some((f) => !f.open && f.people.length)).toBe(true);
    expect(m.floors.some((f) => f.open && !f.people.length)).toBe(true);
  });
  it("has every kind of row in Needs you", () => {
    expect(new Set(m.needs.map((r) => r.kind))).toEqual(new Set(["plan", "paused", "alert", "merge"]));
  });
  it("carries every prop and more than one CLI", () => {
    expect(new Set(people.map((p) => p.prop))).toEqual(new Set(["none", "laptop", "clipboard", "megaphone", "magnifier", "book", "pen", "pencil"]));
    expect(new Set(people.map((p) => p.cli)).size).toBeGreaterThan(1);
  });
  it("can be empty, or quiet", () => {
    expect(demoModel({ empty: true }).floors).toEqual([]);
    expect(demoModel({ quiet: true }).needs).toEqual([]);
  });
  it("does its actions in memory", async () => {
    const s = demoSource();
    await s.giveJob("Anything");
    expect(s.snapshot(0).model?.jobs.map((j) => j.title)).toContain("Anything");
  });
  it("reaches nothing real", () => {
    for (const f of ["fixtures.ts", "demo.ts"]) {
      const text = readFileSync(`src/company/office/${f}`, "utf8");
      expect(text).not.toMatch(/from "\.\.\/engine"|from "\.\/live"|\/ipc"|@tauri-apps/);
    }
  });
});

describe("the demo's variants", () => {
  it("grows to as many floors as asked, within the company's limit of people", () => {
    const m = demoModel({ floors: 8 });
    expect(m.floors).toHaveLength(8);
    expect(m.counts.people).toBe(20);
  });
  it("shrinks too, and leaves no row about someone who is gone", () => {
    const m = demoModel({ floors: 1 });
    expect(m.floors.map((f) => f.id)).toEqual(["bytehive"]);
    expect(m.needs.filter((r) => r.personId).every((r) => m.floors[0].people.some((p) => p.id === r.personId))).toBe(true);
  });
  it("puts a plane half way along its path, whenever it is asked", () => {
    const [flight] = demoModel({ flight: true, now: 10_000 }).flights;
    expect(flight.at).toBe(10_000 - 1200);
    expect(demoModel().flights).toEqual([]);
  });
});

describe("demoSource", () => {
  it("tells its listeners, and stops when told to", async () => {
    const s = demoSource();
    let heard = 0;
    const off = s.onChange(() => { heard += 1; });
    await s.giveJob("One");
    off();
    await s.giveJob("Two");
    expect(heard).toBe(1);
  });
  it("moves a plan on when it is approved, or rejected", async () => {
    const s = demoSource();
    await s.approve("J-0142");
    expect(s.snapshot(0).model?.jobs.find((j) => j.id === "J-0142")?.status).toBe("running");
    const t = demoSource();
    await t.reject("J-0142", "Too big");
    expect(t.snapshot(0).model?.jobs.some((j) => j.id === "J-0142")).toBe(false);
  });
  it("stops a Job", async () => {
    const s = demoSource();
    await s.stopJob("J-0141");
    expect(s.snapshot(0).model?.jobs.some((j) => j.id === "J-0141")).toBe(false);
  });
  it("shows anyone, and has a report for them from the board", async () => {
    const s = demoSource();
    expect(s.showPerson("linh")).toBe(true);
    expect((await s.report("linh"))?.cards.length).toBeGreaterThan(0);
    expect(await s.report("nobody")).toBeNull();
  });
  it("has a board of the size the wall shows", async () => {
    const m = demoSource().snapshot(0).model!;
    expect(m.floors.find((f) => f.id === "bytehive")?.board).toEqual({ todo: 2, doing: 2, done: 5 });
  });
});

describe("levelFrom", () => {
  const m = demoModel();
  it("starts at the building", () => {
    expect(levelFrom(null, m)).toEqual(BUILDING);
    expect(levelFrom("building", m)).toEqual(BUILDING);
  });
  it("reads a floor and a desk", () => {
    expect(levelFrom("floor:bytehive", m)).toEqual({ kind: "floor", floorId: "bytehive" });
    expect(levelFrom("desk:linh", m)).toEqual({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
  });
  it("seats the Founder on no floor", () => {
    expect(levelFrom("desk:founder", m)).toEqual({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" });
  });
  it("falls back to the building for what is not there", () => {
    expect(levelFrom("floor:gone", m)).toEqual(BUILDING);
    expect(levelFrom("desk:gone", m)).toEqual(BUILDING);
    expect(levelFrom("nonsense", m)).toEqual(BUILDING);
  });
});
