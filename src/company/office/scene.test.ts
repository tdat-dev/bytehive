// src/company/office/scene.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { plantSpot, stickies } from "./build";
import { OFFICE } from "./palette";
import { cssPoint, easeInOutCubic, goodRenderer, pickFrom } from "./scene";

describe("goodRenderer", () => {
  it.each([
    ["ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0)", true],
    ["ANGLE (NVIDIA, NVIDIA GeForce RTX 2070 Direct3D11)", true],
    ["Google SwiftShader", false],
    ["ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)))", false],
    ["ANGLE (Microsoft, Microsoft Basic Render Driver Direct3D11)", false],
    ["llvmpipe (LLVM 15.0.7)", false],
  ])("%s is %s", (name, good) => { expect(goodRenderer(name)).toBe(good); });
});

describe("the scene can move to a Worker", () => {
  it.each(["scene.ts", "build.ts"])("%s reads nothing from the page", (file) => {
    const code = readFileSync(`src/company/office/${file}`, "utf8").replace(/\/\*[\s\S]*?\*\/|\/\/.*$/gm, "");
    expect(code).not.toMatch(/\b(document|window|getComputedStyle)\b/);
  });
});

describe("easeInOutCubic", () => {
  it("starts and ends in place, and passes the middle in the middle", () => {
    expect(easeInOutCubic(0)).toBe(0);
    expect(easeInOutCubic(1)).toBe(1);
    expect(easeInOutCubic(0.5)).toBeCloseTo(0.5);
  });
  it("is slow at both ends", () => {
    expect(easeInOutCubic(0.1)).toBeLessThan(0.1);
    expect(easeInOutCubic(0.9)).toBeGreaterThan(0.9);
  });
});

describe("cssPoint", () => {
  it("turns the middle of the view into the middle of the canvas", () => {
    expect(cssPoint({ x: 0, y: 0, z: 0.5 }, 800, 600)).toEqual({ x: 400, y: 300, visible: true });
  });
  it("counts y down from the top, as CSS does", () => {
    expect(cssPoint({ x: -1, y: 1, z: 0.5 }, 800, 600)).toEqual({ x: 0, y: 0, visible: true });
  });
  it("is not visible off the canvas, or behind the camera", () => {
    expect(cssPoint({ x: 1.2, y: 0, z: 0.5 }, 800, 600).visible).toBe(false);
    expect(cssPoint({ x: 0, y: -1.2, z: 0.5 }, 800, 600).visible).toBe(false);
    expect(cssPoint({ x: 0, y: 0, z: 1.2 }, 800, 600).visible).toBe(false);
  });
});

describe("pickFrom", () => {
  const floor = (floorId: string, distance: number) => ({ floorId, distance });
  const person = (personId: string, floorId: string | undefined, distance: number) => ({ personId, floorId, distance });
  it("is nothing when the ray hits nothing", () => {
    expect(pickFrom([])).toBeNull();
  });
  it("is the floor the ray enters first", () => {
    expect(pickFrom([floor("b", 12), floor("a", 10)])).toEqual({ floorId: "a" });
  });
  it("prefers a person on that floor", () => {
    expect(pickFrom([floor("a", 10), person("x", "a", 13)])).toEqual({ personId: "x", floorId: "a" });
  });
  it("takes the nearer of two people", () => {
    expect(pickFrom([floor("a", 10), person("far", "a", 15), person("near", "a", 13)])).toMatchObject({ personId: "near" });
  });
  it("never reaches through a slab to someone on the floor below", () => {
    expect(pickFrom([floor("top", 10), floor("below", 14), person("hidden", "below", 15)])).toEqual({ floorId: "top" });
  });
  it("finds the Founder, who has no floor", () => {
    expect(pickFrom([person("founder", undefined, 9), floor("a", 12)])).toEqual({ personId: "founder" });
  });
});

describe("stickies", () => {
  const total = (o: Record<string, number>) => Object.values(o).reduce((a, b) => a + b, 0);
  const tally = (list: string[]) => ({
    todo: list.filter((c) => c === OFFICE.noteTodo).length,
    doing: list.filter((c) => c === OFFICE.noteDoing).length,
    done: list.filter((c) => c === OFFICE.noteDone).length,
  });
  it("is one note a card while they fit", () => {
    expect(tally(stickies({ todo: 2, doing: 2, done: 5 }))).toEqual({ todo: 2, doing: 2, done: 5 });
  });
  it("has nothing for no board, or an empty one", () => {
    expect(stickies(null)).toEqual([]);
    expect(stickies({ todo: 0, doing: 0, done: 0 })).toEqual([]);
  });
  it("never holds more than twelve, in proportion", () => {
    const t = tally(stickies({ todo: 10, doing: 20, done: 90 }));
    expect(total(t)).toBe(12);
    expect(t.done).toBeGreaterThan(t.doing);
    expect(t.doing).toBeGreaterThan(0);
  });
  it("keeps a list with one card on the wall, however long the others are", () => {
    expect(tally(stickies({ todo: 1, doing: 1, done: 500 }))).toEqual({ todo: 1, doing: 1, done: 10 });
  });
});

describe("plantSpot", () => {
  it("stands in a corner of the room", () => {
    const s = plantSpot([], false)!;
    expect(Math.abs(s.x)).toBeGreaterThan(4);
    expect(s.z).toBeLessThan(-2.5);
  });
  it("keeps clear of a desk", () => {
    const first = plantSpot([], false)!;
    const next = plantSpot([{ x: first.x, z: first.z, rot: 0 }], false)!;
    expect(next).not.toEqual(first);
  });
  it("keeps clear of the wall board", () => {
    expect(plantSpot([], true)).not.toEqual(plantSpot([], false));
  });
  it("gives up with no corner free", () => {
    const desks = [-5, 5].flatMap((x) => [-3, 3].map((z) => ({ x, z, rot: 0 })));
    expect(plantSpot(desks, false)).toBeNull();
  });
});
