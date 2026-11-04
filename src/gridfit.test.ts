import { describe, expect, it } from "vitest";
import { cellsAt, railPlan, CARD_W, GAP, READ_COLS } from "./gridfit";

describe("Grid layout: one terminal and the cards beside it", () => {
  it("gives a laptop's terminal enough columns beside one column of cards", () => {
    const p = railPlan(5, { width: 1200, height: 712 }, 15);
    expect(p.cols).toBe(1);
    expect(p.big.w).toBe(1200 - CARD_W - GAP);
    expect(p.rail).toEqual({ x: p.big.w + GAP, y: 0, w: CARD_W, h: 712 });
    expect(cellsAt(p.big, 15).cols).toBeGreaterThanOrEqual(READ_COLS);
    expect(p.readable).toBe(true);
  });

  it("grows the terminal's font on a 1080p screen", () => {
    const p = railPlan(5, { width: 1584, height: 928 }, 15);
    expect(p.font).toBeGreaterThan(15);
    expect(cellsAt(p.big, p.font).cols).toBeGreaterThanOrEqual(90);
  });

  it("puts the cards in two columns on a 27-inch screen, the terminal still wide", () => {
    const p = railPlan(7, { width: 2224, height: 1288 }, 15);
    expect(p.cols).toBe(2);
    expect(p.rail!.w).toBe(2 * CARD_W + 12);
    expect(p.font).toBe(19);
  });

  it("keeps one column for a few cards", () => {
    expect(railPlan(3, { width: 2224, height: 1288 }, 15).cols).toBe(1);
  });

  it("says when the terminal is too narrow beside the cards", () => {
    const p = railPlan(4, { width: 944, height: 700 }, 15);
    expect(p.readable).toBe(false);
  });

  it("gives the whole stage to the terminal with no cards", () => {
    const p = railPlan(0, { width: 1000, height: 600 }, 15);
    expect(p.big).toEqual({ x: 0, y: 0, w: 1000, h: 600 });
    expect(p.rail).toBeNull();
  });
});
