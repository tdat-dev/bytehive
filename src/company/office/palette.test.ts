import { describe, expect, it } from "vitest";
import { TEXT_PAIRS, contrast } from "./palette";

describe("contrast", () => {
  it("matches the known extremes", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrast("#777777", "#777777")).toBe(1);
  });
  it.each(TEXT_PAIRS)("%s passes", (_name, fg, bg, min) => {
    expect(contrast(fg, bg)).toBeGreaterThanOrEqual(min);
  });
});
