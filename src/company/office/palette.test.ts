import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { APP, OFFICE, TEXT_PAIRS, contrast } from "./palette";

const tokens = readFileSync("src/styles/tokens.css", "utf8");
const token = (name: string) => new RegExp(`--${name}:\\s*([^;]+);`).exec(tokens)?.[1];
const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
/** One colour at `alpha` over another, as the screen shows it. */
const over = (top: string, alpha: number, under: string) =>
  `#${rgb(top).map((c, i) => Math.round(c * alpha + rgb(under)[i] * (1 - alpha)).toString(16).padStart(2, "0")).join("")}`;

describe("contrast", () => {
  it("matches the known extremes", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrast("#777777", "#777777")).toBe(1);
  });
  it.each(TEXT_PAIRS)("%s passes", (_name, fg, bg, min) => {
    expect(contrast(fg, bg)).toBeGreaterThanOrEqual(min);
  });
});

describe("the app's colours", () => {
  const names: Array<[keyof typeof APP, string]> = [
    ["ground", "ground"], ["panel", "panel"], ["raised", "raised"], ["over", "over"],
    ["ink", "ink"], ["ink2", "ink-2"], ["ink3", "ink-3"], ["inkOff", "ink-off"],
    ["honey", "honey"], ["honeyInk", "honey-ink"], ["work", "work"], ["review", "review"], ["stop", "stop"], ["danger", "danger"],
  ];
  it.each(names)("%s is the value in tokens.css", (key, css) => {
    expect(token(css)).toBe(APP[key]);
  });

  it("uses the office's honey, not one of its own", () => {
    expect(APP.honey.toLowerCase()).toBe(OFFICE.honey);
  });

  // ux-overhaul.md 5.1: text on each surface
  const surfaces = [APP.ground, APP.panel, APP.raised, APP.over];
  const TABLE: Array<[string, string, number[]]> = [
    ["ink", APP.ink, [15.7, 14.7, 13.5, 11.9]],
    ["ink-2", APP.ink2, [9.9, 9.3, 8.5, 7.5]],
    ["ink-3", APP.ink3, [6.3, 5.9, 5.4, 4.8]],
    ["honey", APP.honey, [10.6, 10.0, 9.1, 8.1]],
    ["work", APP.work, [10.6, 10.0, 9.1, 8.1]],
    ["review", APP.review, [9.3, 8.7, 8.0, 7.0]],
    ["stop", APP.stop, [8.2, 7.7, 7.1, 6.3]],
    ["danger", APP.danger, [7.6, 7.1, 6.5, 5.7]],
  ];
  it.each(TABLE)("%s on each surface matches the table", (_name, fg, want) => {
    surfaces.forEach((bg, i) => {
      expect(Math.abs(contrast(fg, bg) - want[i])).toBeLessThanOrEqual(0.06);
      expect(contrast(fg, bg)).toBeGreaterThanOrEqual(4.5);
    });
  });

  it("keeps honey readable and a needs row at the floor", () => {
    const soft = over(APP.honey, 0.14, APP.panel);
    expect(soft).toBe("#342e25");
    expect(contrast(APP.honeyInk, APP.honey)).toBeCloseTo(10.4, 0);
    expect(contrast(APP.ink, soft)).toBeCloseTo(11.2, 0);
    expect(contrast(APP.honey, soft)).toBeCloseTo(7.6, 0);
    // the floor, which is why meta text in a needs row is ink-2
    expect(contrast(APP.ink3, soft)).toBeGreaterThanOrEqual(4.45);
    expect(contrast(APP.ink2, soft)).toBeGreaterThanOrEqual(4.5);
  });

  it("gives control borders 3:1 and keeps ink-off to disabled", () => {
    expect(contrast(over(APP.ink, 0.38, APP.panel), APP.panel)).toBeGreaterThanOrEqual(3);
    expect(contrast(over(APP.ink, 0.38, APP.raised), APP.raised)).toBeGreaterThanOrEqual(3);
    expect(contrast(APP.inkOff, APP.panel)).toBeLessThan(4.5);
  });

  it("never rings focus in honey", () => {
    expect(contrast(APP.ground, OFFICE.wallLit)).toBeGreaterThanOrEqual(3);
    expect(tokens).toMatch(/--ring:0 0 0 2px var\(--ink\),0 0 0 4px var\(--ground\)/);
  });
});
