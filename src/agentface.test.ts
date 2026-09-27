import { describe, expect, it } from "vitest";
import { faceGrid, faceParts, faceSvg } from "./agentface";
import { PERSONA_NAMES } from "./crew";

describe("an agent's pixel face", () => {
  it("is the same for the same name, whatever its case or spaces", () => {
    expect(faceParts("Ana")).toEqual(faceParts(" ana "));
    expect(faceSvg("Ana", "#f2b27a")).toBe(faceSvg("Ana", "#f2b27a"));
  });

  it("is a symmetric 8×8 face with eyes on the head", () => {
    for (const name of PERSONA_NAMES) {
      const g = faceGrid(faceParts(name));
      expect(g).toHaveLength(8);
      for (const row of g) {
        expect(row).toHaveLength(8);
        expect(row).toBe([...row].reverse().join(""));
      }
      expect(g.join("")).toContain("o"); // it has eyes
    }
  });

  it("tells the first agents of a project apart", () => {
    const first = PERSONA_NAMES.slice(0, 10).map((n) => faceGrid(faceParts(n)).join("/"));
    expect(new Set(first).size).toBe(10);
    const all = PERSONA_NAMES.map((n) => faceGrid(faceParts(n)).join("/"));
    expect(new Set(all).size).toBe(all.length);
  });

  it("draws the head in the agent's colour and keeps markup out of it", () => {
    const svg = faceSvg("Bob", '#6fd6a0"><script>');
    expect(svg).not.toContain("<script");
    expect(svg).toContain('fill="#6fd6a0script"');
    expect(faceSvg("Bob", "oklch(0.8 0.1 40)")).toContain('fill="oklch(0.8 0.1 40)"');
    expect(svg).toMatch(/^<svg class="face" viewBox="0 0 8 8"/);
  });
});
