// src/company/needs.test.ts
import { describe, it, expect } from "vitest";
import { needsYou } from "./needs";
import type { Job } from "./types";

const job = (over: Partial<Job>): Job => ({ id: "J-1", title: "Add login", ask: "a", status: "running", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 1, ...over });

describe("needsYou", () => {
  it("is empty while work just runs", () => {
    expect(needsYou([job({})], [])).toEqual([]);
  });
  it("shows a plan with what it will cost you in people", () => {
    const rows = needsYou([job({ status: "awaiting-plan", plan: { summary: "Build it", cards: [], hires: [{ role: "qa", floor: "a" }] } })], []);
    expect(rows[0]).toMatchObject({ kind: "plan", jobId: "J-1", title: "Approve plan: Add login" });
    expect(rows[0].detail).toContain("Build it");
    expect(rows[0].detail).toContain("1 hire");
  });
  it("shows merge and paused jobs", () => {
    const rows = needsYou([job({ status: "awaiting-merge", summary: "Done" }), job({ id: "J-2", status: "paused" })], []);
    expect(rows.map((r) => r.kind)).toEqual(["paused", "merge"]);
  });
  it("puts plans first, then stops, then the rest", () => {
    const rows = needsYou([job({ status: "awaiting-merge" }), job({ id: "J-2", status: "awaiting-plan", plan: { summary: "s", cards: [], hires: [] } })], [
      { key: "k", job: "J-1", personId: "d1", text: "stuck", level: "warn" },
    ]);
    expect(rows.map((r) => r.kind)).toEqual(["plan", "alert", "merge"]);
  });
});
