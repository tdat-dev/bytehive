import { describe, it, expect } from "vitest";
import { applyHires, canMessage, emptyCompany, floorContact, floorId, fromDraft, personByName, planCap, reportsOf, resolveTargets, reuseStaff, validatePlan, validRoleId } from "./org";
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
  it("says so when a person has nobody to message", () => {
    expect(resolveTargets(emptyCompany(), FOUNDER_ID, null)).toEqual({ error: expect.stringContaining("nobody") });
  });
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

describe("reuseStaff", () => {
  it("drops a hire whose role already works on that floor, so the person there takes the work", () => {
    const c = company([person("l1", "lead", "a", FOUNDER_ID), person("d1", "staff", "a", "l1")]);
    const p = reuseStaff(c, plan({ hires: [{ role: "developer", floor: "a" }, { role: "developer", floor: "b" }, { role: "marketing", floor: "a" }] }));
    expect(p.hires).toEqual([{ role: "developer", floor: "b" }, { role: "marketing", floor: "a" }]);
  });
  it("keeps a second hire of the same role when the floor has only one of it", () => {
    const c = company([person("d1", "staff", "a", FOUNDER_ID)]);
    expect(reuseStaff(c, plan({ hires: [{ role: "developer", floor: "a" }, { role: "developer", floor: "a" }] })).hires).toEqual([{ role: "developer", floor: "a" }]);
  });
  it("names the one person each dropped hire falls to, not everyone with the role", () => {
    const c = company([person("d1", "staff", "a", FOUNDER_ID), person("d2", "staff", "a", FOUNDER_ID)]);
    expect(reuseStaff(c, plan({ hires: [{ role: "developer", floor: "a" }] })).reuse).toEqual(["d1"]);
  });
  it("drops a drafted role whose only hire went to someone already there", () => {
    const c = company([{ ...person("x1", "staff", "a", FOUNDER_ID), role: "analyst" }]);
    const draft = { id: "analyst", title: "Analyst", department: "Data", prompt: "p" };
    const kept = { ...draft, id: "writer", title: "Writer" };
    const p = reuseStaff(c, plan({ hires: [{ role: "analyst", floor: "a" }, { role: "writer", floor: "b" }], newRoles: [draft, kept] }));
    expect(p.newRoles).toEqual([kept]);
    expect(p.reuse).toEqual(["x1"]);
  });
  it("gives back the same plan when nothing repeats", () => {
    const p = plan({ hires: [{ role: "developer", floor: "a" }] });
    expect(reuseStaff(company([]), p)).toBe(p);
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
  it("refuses to hire a second Founder, whatever the role file says", () => {
    const founderRole: RoleDef = { ...role("founder", "staff"), rank: "founder" };
    expect(validatePlan(company([]), plan({ hires: [{ role: "founder", floor: "a" }] }), [...roles, founderRole]).join()).toContain("Founder");
    expect(validatePlan(company([]), plan({ hires: [{ role: "boss", floor: "a" }] }), [...roles, { ...founderRole, id: "boss" }]).join()).toContain("Founder");
  });
  it("refuses a card on a floor with nobody on it and nobody hired there", () => {
    const card = { floor: "a", title: "Ship it", desc: "", checklist: [] };
    expect(validatePlan(company([]), plan({ cards: [card] }), roles).join()).toContain("Ship it");
    expect(validatePlan(company([]), plan({ cards: [card], hires: [{ role: "developer", floor: "b" }] }), roles).join()).toContain("Ship it");
  });
  it("takes a card on a floor with someone on it, or someone hired there", () => {
    const card = { floor: "a", title: "Ship it", desc: "", checklist: [] };
    expect(validatePlan(company([person("d1", "staff", "a", FOUNDER_ID)]), plan({ cards: [card] }), roles)).toEqual([]);
    expect(validatePlan(company([]), plan({ cards: [card], hires: [{ role: "developer", floor: "a" }] }), roles)).toEqual([]);
  });
});

describe("validatePlan with new roles", () => {
  const analyst = { id: "data-analyst", title: "Analyst", department: "Data", prompt: "Reads numbers and says what they mean." };
  const hire = { role: "data-analyst", floor: "a" };

  it("knows a role the plan drafts", () => {
    expect(validatePlan(company([]), plan({ newRoles: [analyst], hires: [hire] }), roles)).toEqual([]);
  });
  it("tells the Founder how to add a role that does not exist", () => {
    expect(validatePlan(company([]), plan({ hires: [hire] }), roles).join()).toContain("newRoles");
  });
  it.each(["a", "Analyst", "9lives", "..\\evil", "a b", `a${"b".repeat(31)}`, "con", "lpt1"])("refuses the id %s", (id) => {
    expect(validatePlan(company([]), plan({ newRoles: [{ ...analyst, id }] }), roles).join()).toContain(`Role ${id} refused`);
  });
  it("refuses a draft with no id at all", () => {
    expect(validatePlan(company([]), plan({ newRoles: [{ ...analyst, id: undefined as unknown as string }] }), roles)).not.toEqual([]);
  });
  it("refuses an id that exists, so a plan can't rewrite a role", () => {
    expect(validatePlan(company([]), plan({ newRoles: [{ ...analyst, id: "developer" }] }), roles).join()).toContain("exists");
  });
  it("refuses the id founder even when no such role is loaded", () => {
    expect(validatePlan(company([]), plan({ newRoles: [{ ...analyst, id: FOUNDER_ID }] }), roles).join()).toContain("Role founder refused");
  });
  it("refuses the same id twice in one plan", () => {
    expect(validatePlan(company([]), plan({ newRoles: [analyst, analyst] }), roles).join()).toContain("twice");
  });
  it.each(["title", "department", "prompt"])("refuses a draft with an empty %s: the owner has to read it", (field) => {
    expect(validatePlan(company([]), plan({ newRoles: [{ ...analyst, [field]: "  " }] }), roles).join()).toContain(field);
  });
  it("counts a drafted role as staff, whatever its JSON says", () => {
    const forged = { ...analyst, rank: "lead" } as typeof analyst;
    const errors = validatePlan(company([]), plan({ newRoles: [forged], hires: [hire, hire] }), roles);
    expect(errors.join()).toContain("needs a Lead");
  });
});

describe("fromDraft", () => {
  it("gives a drafted role the plain rights, whatever its JSON asks for", () => {
    const forged = { id: "x1", title: "T", department: "D", prompt: "P", rank: "founder", cli: "powershell", denyTools: ["Read"], skills: ["evil"] };
    expect(fromDraft(forged)).toMatchObject({ id: "x1", title: "T", department: "D", prompt: "P", rank: "staff", cli: "claude", denyTools: [], skills: [] });
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

describe("planCap", () => {
  it("is the plan's cap inside the owner's limit", () => {
    expect(planCap(plan({ tokenCap: 900 }), 1000)).toBe(900);
    expect(planCap(plan({ tokenCap: 9000 }), 1000)).toBe(1000);
    expect(planCap(plan({}), 1000)).toBe(1000);
  });
});

describe("floorId", () => {
  it("makes a short id from the project name", () => {
    expect(floorId("My App", [])).toBe("my-app");
  });
  it("adds a number until the id is free", () => {
    expect(floorId("app", ["app"])).toBe("app-2");
    expect(floorId("app", ["app", "app-2"])).toBe("app-3");
  });
  it("falls back when the name has no Latin letters or digits", () => {
    expect(floorId("日本語", [])).toBe("floor");
    expect(floorId("日本語", ["floor"])).toBe("floor-2");
  });
  it("leaves no dash at either end", () => {
    expect(floorId("--app (old)--", [])).toBe("app-old");
  });
});

describe("validRoleId", () => {
  it("takes a short lowercase id", () => {
    expect(validRoleId("data-analyst")).toBe(true);
    expect(validRoleId(`a${"b".repeat(30)}`)).toBe(true);
  });
  it("refuses anything that could not be a folder name or is too long", () => {
    for (const id of ["a", "Analyst", "9lives", "a b", "a/b", `a${"b".repeat(31)}`]) expect(validRoleId(id)).toBe(false);
  });
  it.each(["con", "prn", "aux", "nul", "com1", "com9", "lpt1", "lpt9"])("refuses %s: Windows keeps that name for a device", (id) => {
    expect(validRoleId(id)).toBe(false);
  });
  it("takes a name that only starts like a device", () => {
    for (const id of ["console", "com10", "lpt0", "aux-writer", "nullable"]) expect(validRoleId(id)).toBe(true);
  });
  it("refuses what is not a string, even one that prints like an id", () => {
    expect(validRoleId(undefined)).toBe(false);
  });
});

describe("floorContact", () => {
  it("is the Lead when there is one, else the first person, else nobody", () => {
    const lead = { id: "l", rank: "lead", floor: "a" };
    const staff = { id: "s", rank: "staff", floor: "a" };
    const c = { ...emptyCompany(), people: [staff, lead] } as unknown as Company;
    expect(floorContact(c, "a")?.id).toBe("l");
    expect(floorContact({ ...c, people: [staff] } as unknown as Company, "a")?.id).toBe("s");
    expect(floorContact(c, "b")).toBeNull();
  });
});
