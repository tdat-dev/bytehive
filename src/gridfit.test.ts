import { describe, expect, it } from "vitest";
import { cellsAt, gridPlan, gridTiles, READ_COLS, READ_ROWS } from "./gridfit";

/** Every card of a plan reaches a readable terminal at its font. */
const allReadable = (p: ReturnType<typeof gridPlan>) =>
  p.tiles.every((t) => { const c = cellsAt(t, p.font); return c.cols >= READ_COLS && c.rows >= READ_ROWS; });

describe("Grid fit", () => {
  it("lays two cards side by side, a short last row sharing the full width", () => {
    const [l, r] = gridTiles(2, 2, { width: 1012, height: 600 });
    expect([l.x, l.w, r.x, r.w, l.h]).toEqual([0, 498, 514, 498, 600]);
    const three = gridTiles(3, 2, { width: 1016, height: 616 });
    expect(three.map((t) => [t.x, t.y, t.w, t.h])).toEqual([[0, 0, 500, 300], [516, 0, 500, 300], [0, 316, 1016, 300]]);
  });

  it("shows fewer agents on a 27-inch screen instead of six unreadable ones, and grows their font", () => {
    const p = gridPlan(6, { width: 2200, height: 1280 }, 15);
    expect(p.count).toBe(4);
    expect(p.readable).toBe(true);
    expect(p.font).toBeGreaterThan(15);
    expect(allReadable(p)).toBe(true);
  });

  it("shows all of them when there is room", () => {
    const p = gridPlan(3, { width: 3400, height: 1300 }, 15);
    expect(p.count).toBe(3);
    expect(allReadable(p)).toBe(true);
  });

  it("never grows the font past what keeps a card readable", () => {
    const p = gridPlan(2, { width: 1900, height: 900 }, 15);
    expect(p.count).toBe(2);
    expect(allReadable(p)).toBe(true);
    expect(p.font).toBeLessThanOrEqual(19);
  });

  it("still shows two on a laptop where nothing fits, and says it is cramped", () => {
    const p = gridPlan(6, { width: 1180, height: 700 }, 15);
    expect(p.count).toBe(2);
    expect(p.readable).toBe(false);
    expect(p.font).toBe(15);
  });

  it("gives one agent the whole stage", () => {
    const p = gridPlan(1, { width: 1000, height: 600 }, 15);
    expect(p.tiles).toEqual([{ x: 0, y: 0, w: 1000, h: 600 }]);
  });
});
