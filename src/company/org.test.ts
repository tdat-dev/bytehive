import { describe, it, expect } from "vitest";
import { applyHires, canMessage, emptyCompany, personByName, reportsOf, resolveTargets, validatePlan } from "./org";
import { DEFAULT_LIMITS, FOUNDER_ID, type Company, type JobPlan, type Person, type RoleDef } from "./types";

const role = (id: string, rank: "lead" | "staff"): RoleDef => ({
  id, title: id, department: "x", rank, cli: "claude", denyTools: [], look: { shirt: "#fff", hair: "#000", prop: "none" }, prompt: "p", skills: [],
});
const roles = [role("lead", "lead"), role("developer", "staff"), role("marketing", "staff")];
const person = (id: string, rank: Person["rank"], floor: string | null, boss: string | null): Person => ({
  id, name: id.toUpperCase(), role: "developer", rank, floor, boss, cli: "claude", look: role("x", "staff").look,
});
const company = (people: Person[]): Company => ({
  ...emptyCompany(),
  floors: [{ id: "a", dir: "D:\\a", name: "a" }, { id: "b", dir: "D:\\b", name: "b" }],
  people: [...emptyCompany().people, ...people],
});
const plan = (over: Partial<JobPlan>): JobPlan => ({ summary: "s", cards: [], hires: [], ...over });
let n = 0;
const pick = { name: () => `N${(n += 1)}`, id: () => `id${n}` };

describe("canMessage", () => {
  const c = company([person("l1", "lead", "a", FOUNDER_ID), person("d1", "staff", "a", "l1"), person("d2", "staff", "a", "l1"), person("l2", "lead", "b", FOUNDER_ID)]);
  it("lets a person reach their boss and their reports", () => {
    expect(canMessage(c, "d1", "l1")).toBe(true);
    expect(canMessage(c, "l1", "d1")).toBe(true);
    expect(canMessage(c, FOUNDER_ID, "l1")).toBe(true);
  });
  it("stops peers, skips and other floors", () => {
    expect(canMessage(c, "d1", "d2")).toBe(false);
    expect(canMessage(c, "d1", FOUNDER_ID)).toBe(false);
    expect(canMessage(c, "l1", "l2")).toBe(false);
  });
});

describe("resolveTargets", () => {
  const c = company([person("l1", "lead", "a", FOUNDER_ID), person("d1", "staff", "a", "l1")]);
  it("sends a message with no target to the boss, or the reports of a person with none above", () => {
    expect(resolveTargets(c, "d1", null)).toEqual({ people: [personByName(c, "L1")] });
    expect(resolveTargets(c, FOUNDER_ID, null)).toEqual({ people: reportsOf(c, FOUNDER_ID) });
  });
  it("explains a refusal in words the agent can act on", () => {
    const r = resolveTargets(c, "d1", "Founder");
    expect(r).toEqual({ error: expect.stringContaining("L1") });
  });
  it("reports an unknown name", () => {
    expect(resolveTargets(c, "d1", "Ghost")).toEqual({ error: expect.stringContaining("Ghost") });
  });
});

describe("validatePlan", () => {
  it("accepts a one person hire on an empty floor", () => {
    expect(validatePlan(company([]), plan({ hires: [{ role: "marketing", floor: "a" }] }), roles)).toEqual([]);
  });
  it("needs a Lead once a floor reaches two people", () => {
    const errors = validatePlan(company([]), plan({ hires: [{ role: "developer", floor: "a" }, { role: "developer", floor: "a" }] }), roles);
    expect(errors.join()).toContain("Lead");
  });
  it("rejects an unknown floor, an unknown role, and too many hires", () => {
    const five = Array.from({ length: 6 }, () => ({ role: "developer", floor: "a" }));
    const errors = validatePlan(company([]), plan({ hires: [...five, { role: "lead", floor: "a" }, { role: "ghost", floor: "z" }] }), roles);
    expect(errors.join("|")).toMatch(/floor/);
    expect(errors.join("|")).toMatch(/role/);
    expect(errors.join("|")).toContain(String(DEFAULT_LIMITS.peoplePerJob));
  });
  it("rejects a card on an unknown floor", () => {
    expect(validatePlan(company([]), plan({ cards: [{ floor: "z", title: "t", desc: "", checklist: [] }] }), roles).join()).toMatch(/floor/);
  });
});

describe("applyHires", () => {
  it("reports a lone hire to the Founder", () => {
    const { hired } = applyHires(company([]), plan({ hires: [{ role: "marketing", floor: "a" }] }), roles, pick);
    expect(hired[0].boss).toBe(FOUNDER_ID);
    expect(hired[0].rank).toBe("staff");
  });
  it("puts staff under the floor's Lead and moves a lone worker under a new Lead", () => {
    const c = company([person("d1", "staff", "a", FOUNDER_ID)]);
    const { company: next, hired } = applyHires(c, plan({ hires: [{ role: "lead", floor: "a" }, { role: "developer", floor: "a" }] }), roles, pick);
    const lead = hired.find((p) => p.rank === "lead")!;
    expect(lead.boss).toBe(FOUNDER_ID);
    expect(next.people.find((p) => p.id === "d1")!.boss).toBe(lead.id);
    expect(hired.find((p) => p.rank === "staff")!.boss).toBe(lead.id);
  });
});
