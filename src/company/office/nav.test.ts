import { describe, expect, it } from "vitest";
import type { NeedRow } from "../needs";
import type { OfficeModel } from "./model";
import { AMBIENT_MS, BUILDING, MOVE_MS, clampLevel, crumbs, escapeGoesUp, frameDelay, homeView, navUp, rowTarget, type Level } from "./nav";

const founder = { id: "founder", name: "Founder", floorId: null };
const ana = { id: "ana", name: "Ana", floorId: "a" };
const model = { founder, floors: [{ id: "a", name: "alpha", people: [ana] }] } as unknown as OfficeModel;
const floor: Level = { kind: "floor", floorId: "a" };
const desk: Level = { kind: "desk", personId: "ana", floorId: "a", tab: "terminal" };
const row = (over: Partial<NeedRow>): NeedRow => ({ key: "k", kind: "alert", jobId: null, personId: null, title: "t", detail: "", ...over });

describe("navUp", () => {
  it("goes one level up and stops at the building", () => {
    expect(navUp(desk)).toEqual(floor);
    expect(navUp(floor)).toEqual(BUILDING);
    expect(navUp(BUILDING)).toEqual(BUILDING);
  });
  it("takes the Founder's desk straight to the building", () => {
    expect(navUp({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" })).toEqual(BUILDING);
  });
});

describe("crumbs", () => {
  it("names the path, with the last one current", () => {
    expect(crumbs(desk, model).map((c) => c.label)).toEqual(["ByteHive", "alpha", "Ana"]);
    expect(crumbs(desk, model).map((c) => c.current)).toEqual([false, false, true]);
    expect(crumbs(BUILDING, model)).toEqual([{ label: "ByteHive", level: BUILDING, current: true }]);
  });
  it("has no floor for the Founder", () => {
    expect(crumbs({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" }, model).map((c) => c.label)).toEqual(["ByteHive", "Founder"]);
  });
});

describe("escapeGoesUp", () => {
  const calm = { level: floor, terminal: false, dialog: false, dock: false, typing: false };
  it("goes up with nothing in the way", () => { expect(escapeGoesUp(calm)).toBe(true); });
  it.each(["terminal", "dialog", "dock", "typing"] as const)("leaves Esc to %s", (k) => {
    expect(escapeGoesUp({ ...calm, [k]: true })).toBe(false);
  });
  it("has nowhere to go from the building", () => { expect(escapeGoesUp({ ...calm, level: BUILDING })).toBe(false); });
});

describe("rowTarget", () => {
  it("flies to the person a row names", () => {
    expect(rowTarget(row({ personId: "ana" }), model)).toEqual(desk);
    expect(rowTarget(row({ personId: "founder" }), model)).toEqual({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" });
  });
  it("opens Changes for a merge", () => {
    expect(rowTarget(row({ kind: "merge", personId: "ana" }), model)).toEqual({ ...desk, tab: "changes" });
  });
  it("stays at the building for a row about a Job or nobody", () => {
    expect(rowTarget(row({ kind: "plan" }), model)).toEqual(BUILDING);
    expect(rowTarget(row({ personId: "gone" }), model)).toEqual(BUILDING);
  });
});

describe("clampLevel", () => {
  it("keeps a level that still exists", () => {
    expect(clampLevel(desk, model)).toBe(desk);
    expect(clampLevel(floor, model)).toBe(floor);
  });
  it("falls back when a person or a floor has gone", () => {
    expect(clampLevel({ ...desk, personId: "gone" }, model)).toEqual(floor);
    expect(clampLevel({ kind: "floor", floorId: "z" }, model)).toEqual(BUILDING);
  });
});

describe("homeView", () => {
  const used = { used: true, webgl: true, elsewhere: false };
  it("is the office once the Company has been used", () => { expect(homeView(used)).toBe("office"); });
  it("is the app as it was for everyone else", () => { expect(homeView({ ...used, used: false })).toBe("inbox"); });
  it("is the Inbox with no WebGL, or with the Company in another window", () => {
    expect(homeView({ ...used, webgl: false })).toBe("inbox");
    expect(homeView({ ...used, elsewhere: true })).toBe("inbox");
  });
});

describe("frameDelay", () => {
  const still = { visible: true, moving: false, ambient: false, reduced: false };
  it("does not draw at all when nothing moves", () => { expect(frameDelay(still)).toBeNull(); });
  it("stops when hidden, whatever is moving", () => { expect(frameDelay({ ...still, visible: false, moving: true, ambient: true })).toBeNull(); });
  it("caps moving at 30 frames a second", () => { expect(frameDelay({ ...still, moving: true })).toBe(MOVE_MS); expect(MOVE_MS).toBeGreaterThanOrEqual(1000 / 30); });
  it("draws working people at a low rate", () => { expect(frameDelay({ ...still, ambient: true })).toBe(AMBIENT_MS); });
  it("drops ambient motion for reduced motion, not a move in progress", () => {
    expect(frameDelay({ ...still, ambient: true, reduced: true })).toBeNull();
    expect(frameDelay({ ...still, moving: true, reduced: true })).toBe(MOVE_MS);
  });
});
