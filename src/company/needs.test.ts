// src/company/needs.test.ts
import { describe, it, expect } from "vitest";
import { needsYou } from "./needs";
import type { Job, RoleDef } from "./types";

const job = (over: Partial<Job>): Job => ({ id: "J-1", title: "Add login", ask: "a", status: "running", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 1, ...over });

describe("needsYou", () => {
  it("says which alerts stand until you dismiss them", () => {
    const rows = needsYou([], [
      { key: "a", job: null, personId: null, text: "file", level: "warn", standing: true },
      { key: "b", job: "J-1", personId: null, text: "stuck", level: "warn" },
    ]);
    expect(rows.map((r) => r.standing)).toEqual([true, false]);
  });
  it("is empty while work just runs", () => {
    expect(needsYou([job({})], [])).toEqual([]);
  });
  it("shows a plan with what it will cost you in people", () => {
    const rows = needsYou([job({ status: "awaiting-plan", plan: { summary: "Build it", cards: [], hires: [{ role: "qa", floor: "a" }] } })], []);
    expect(rows[0]).toMatchObject({ kind: "plan", jobId: "J-1", title: "Approve plan: Add login" });
    expect(rows[0].detail).toContain("Build it");
    expect(rows[0].detail).toContain("1 hire");
  });
  it("shows what you are approving: hires, cards per floor, the cap and every new role in full", () => {
    const plan = {
      summary: "Build it",
      cards: [
        { floor: "a", title: "Login page", desc: "", checklist: [] },
        { floor: "b", title: "Copy", desc: "", checklist: [] },
        { floor: "a", title: "Tests", desc: "", checklist: [] },
      ],
      hires: [{ role: "qa", floor: "a" }, { role: "analyst", floor: "z" }],
      tokenCap: 900,
      newRoles: [{ id: "analyst", title: "Analyst", department: "Data", prompt: "Line one.\nLine two." }],
    };
    const [row] = needsYou([job({ status: "awaiting-plan", plan, tokenCap: 5000 })], [], {
      floors: [{ id: "a", dir: "D:\\a", name: "gcare" }, { id: "b", dir: "D:\\b", name: "site" }],
      roles: [{ id: "qa", title: "QA" } as RoleDef],
    });
    expect(row.plan).toEqual({
      hires: ["QA on gcare", "Analyst on z"],
      floors: [{ floor: "gcare", cards: ["Login page", "Tests"] }, { floor: "site", cards: ["Copy"] }],
      tokenCap: 900,
      newRoles: plan.newRoles,
    });
  });
  it("shows the cap that will apply, not what the plan asked for", () => {
    const plan = { summary: "s", cards: [], hires: [], tokenCap: 9000 };
    expect(needsYou([job({ status: "awaiting-plan", plan, tokenCap: 5000 })], [])[0].plan?.tokenCap).toBe(5000);
  });
  it("shows merge and paused jobs", () => {
    const rows = needsYou([job({ status: "awaiting-merge", summary: "Done" }), job({ id: "J-2", status: "paused" })], []);
    expect(rows.map((r) => r.kind)).toEqual(["paused", "merge"]);
  });
  it("lets a merge row open the first person of the job", () => {
    const rows = needsYou([job({ status: "awaiting-merge", people: ["p1", "p2"] }), job({ id: "J-2", status: "awaiting-merge" })], []);
    expect(rows.map((r) => r.personId)).toEqual(["p1", null]);
  });
  it("puts plans first, then stops, then the rest", () => {
    const rows = needsYou([job({ status: "awaiting-merge" }), job({ id: "J-2", status: "awaiting-plan", plan: { summary: "s", cards: [], hires: [] } })], [
      { key: "k", job: "J-1", personId: "d1", text: "stuck", level: "warn" },
    ]);
    expect(rows.map((r) => r.kind)).toEqual(["plan", "alert", "merge"]);
  });
});
