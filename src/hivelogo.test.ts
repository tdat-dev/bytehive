import { describe, it, expect } from "vitest";
import { CELLS, cellPath, hiveLogo } from "./hivelogo";

describe("hivelogo", () => {
  it("has every comb cell once, the middle one first", () => {
    expect(CELLS).toHaveLength(13);
    expect(CELLS[0]).toEqual([0, 0]);
    expect(new Set(CELLS.map(String)).size).toBe(13);
  });

  it("puts a cell on the comb, odd rows half a cell to the right", () => {
    expect(cellPath(0, 0).startsWith("M12 9.4 ")).toBe(true);
    expect(cellPath(0, 1)).toMatch(/^M14\.25\d* 13\.3 /);
    expect(cellPath(0, -1)).toMatch(/^M14\.25\d* 5\.5 /);
  });

  it("numbers the cells in turn and keeps its defs apart", () => {
    const svg = hiveLogo("st");
    expect(svg.match(/class="hc"/g)).toHaveLength(13);
    expect(svg).toContain('style="--n:12"');
    expect(svg).toContain('id="stPrompt"');
    expect(svg).not.toContain('id="Prompt"');
  });
});
