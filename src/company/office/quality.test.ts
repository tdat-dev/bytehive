import { afterEach, describe, expect, it, vi } from "vitest";
import { LOW_TIER, lookOf, readQuality, saveQuality, startTier, steppedDown } from "./quality";

const frames = (ms: number, n = 12) => Array<number>(n).fill(ms);

describe("startTier", () => {
  it("is everything on for High and for Auto", () => {
    expect(startTier("high", false)).toBe(0);
    expect(startTier("auto", false)).toBe(0);
  });
  it("is the bottom for Low", () => {
    expect(startTier("low", false)).toBe(LOW_TIER);
  });
  it("is Low on battery, unless the owner said High", () => {
    expect(startTier("auto", true)).toBe(LOW_TIER);
    expect(startTier("high", true)).toBe(0);
  });
});

describe("lookOf", () => {
  it("gives up shadows first, then glow, then ambient motion", () => {
    expect(lookOf(0)).toMatchObject({ shadows: true, glow: true, ambient: true });
    expect(lookOf(1)).toMatchObject({ shadows: false, glow: true, ambient: true });
    expect(lookOf(2)).toMatchObject({ shadows: false, glow: false, ambient: true });
    expect(lookOf(LOW_TIER)).toMatchObject({ shadows: false, glow: false, ambient: false });
  });
  it("holds the same look past the bottom", () => {
    expect(lookOf(LOW_TIER + 3)).toEqual(lookOf(LOW_TIER));
  });
});

describe("steppedDown", () => {
  it("stays put while a level change holds 30 frames a second", () => {
    expect(steppedDown(0, "auto", frames(30))).toBe(0);
  });
  it("takes one step down when it does not", () => {
    expect(steppedDown(0, "auto", frames(48))).toBe(1);
    expect(steppedDown(1, "auto", frames(48))).toBe(2);
  });
  it("is not moved by one slow frame", () => {
    expect(steppedDown(0, "auto", [...frames(30, 11), 90])).toBe(0);
  });
  it("steps down for frames over 100 ms even if the middle looks fine", () => {
    expect(steppedDown(0, "auto", [...frames(30, 8), 120, 130, 140])).toBe(1);
  });
  it("needs a few frames to go on", () => {
    expect(steppedDown(0, "auto", frames(80, 3))).toBe(0);
  });
  it("never goes under the bottom", () => {
    expect(steppedDown(LOW_TIER, "auto", frames(80))).toBe(LOW_TIER);
  });
  it("leaves a chosen setting alone", () => {
    expect(steppedDown(0, "high", frames(80))).toBe(0);
    expect(steppedDown(LOW_TIER, "low", frames(80))).toBe(LOW_TIER);
  });
});

describe("the setting", () => {
  afterEach(() => { localStorage.clear(); vi.restoreAllMocks(); });
  it("is Auto until the owner chooses", () => {
    expect(readQuality()).toBe("auto");
  });
  it("keeps what the owner chose", () => {
    saveQuality("low");
    expect(readQuality()).toBe("low");
  });
  it("reads anything else on disk as Auto", () => {
    localStorage.setItem("bytehive.officeQuality", "ultra");
    expect(readQuality()).toBe("auto");
  });
});
