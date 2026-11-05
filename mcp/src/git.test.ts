import { describe, it, expect } from "vitest";
import { evidenceDir } from "./git.js";

describe("evidenceDir", () => {
  it("reads the agent's run folder when ByteHive gave one", () => {
    expect(evidenceDir({ BYTEHIVE_RUN_DIR: "D:\\wt\\ana" }, "D:\\app")).toBe("D:\\wt\\ana");
  });

  it("falls back to the board's folder", () => {
    expect(evidenceDir({}, "D:\\app")).toBe("D:\\app");
    expect(evidenceDir({ BYTEHIVE_RUN_DIR: "  " }, "D:\\app")).toBe("D:\\app");
  });
});
