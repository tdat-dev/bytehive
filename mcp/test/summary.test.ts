import { describe, expect, it } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { slugPane, summaryPath, writeSummary } from "../src/summary.js";

describe("slugPane", () => {
  it("lowercases and dashes non-alphanumerics", () => {
    expect(slugPane("Claude Code 0k5qww")).toBe("claude-code-0k5qww");
    expect(slugPane("A__B!!")).toBe("a-b");
  });

  it("falls back to agent when nothing slug-worthy remains", () => {
    expect(slugPane("")).toBe("agent");
    expect(slugPane("---")).toBe("agent");
  });
});

describe("writeSummary", () => {
  const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), "bytehive-summary-"));

  it("writes Doing/Blocked/Next to the slugged file, creating .bytehive", () => {
    const dir = tmp();
    const r = writeSummary(dir, "Claude Code 1", { doing: "login form", blocked: "no", next: "tests" });
    expect(r.path).toBe(summaryPath(dir, "Claude Code 1"));
    expect(fs.readFileSync(r.path, "utf8")).toBe("Doing: login form\nBlocked: no\nNext: tests\n");
  });

  it("overwrites on the next report (latest wins)", () => {
    const dir = tmp();
    writeSummary(dir, "Ana", { doing: "a", blocked: "no", next: "b" });
    writeSummary(dir, "Ana", { doing: "c", blocked: "d", next: "e" });
    expect(fs.readFileSync(summaryPath(dir, "Ana"), "utf8")).toContain("Doing: c");
  });

  it("rejects empty fields and an empty agent", () => {
    const dir = tmp();
    expect(() => writeSummary(dir, "Ana", { doing: "", blocked: "no", next: "x" })).toThrow(/non-empty/);
    expect(() => writeSummary(dir, "  ", { doing: "a", blocked: "no", next: "x" })).toThrow(/agent/);
  });
});
