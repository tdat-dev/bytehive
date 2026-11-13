import { describe, expect, it } from "vitest";
import { isFramed } from "./frame";

describe("isFramed", () => {
  it("is false for a page that is its own top", () => {
    const w = {};
    expect(isFramed({ top: w, self: w })).toBe(false);
  });
  it("is true inside another page", () => {
    expect(isFramed({ top: {}, self: {} })).toBe(true);
  });
  it("is true when the top can't even be read", () => {
    const w = { get top(): unknown { throw new Error("cross-origin"); }, self: {} };
    expect(isFramed(w)).toBe(true);
  });
});
