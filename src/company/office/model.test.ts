import { describe, expect, it } from "vitest";
import { emptyCompany } from "../org";
import type { Company, Job, Person, RoleDef } from "../types";
import type { Alert } from "../watch";
import { companyUsed, departmentShirt, hairFor, lookOf, officeModel, officeState, sceneKey, type OfficePane } from "./model";

const status = (state: string, extra: object = {}) => ({ status: { state, ask: null, ...extra } }) as OfficePane["task"];
const person = (id: string, over: Partial<Person> = {}): Person => ({
  id, name: id.toUpperCase(), role: "developer", rank: "staff", floor: "a", boss: "founder", cli: "claude",
  look: { shirt: "#111111", hair: "#222222", prop: "laptop" }, ...over,
});
const company = (people: Person[] = []): Company => ({
  ...emptyCompany(),
  floors: [{ id: "a", dir: "D:\\a", name: "alpha" }, { id: "b", dir: "D:\\b", name: "beta" }],
  people: [...emptyCompany().people, ...people],
});
const pane = (personId: string, task: OfficePane["task"]): OfficePane => ({ personId, wsId: "w", paneId: `pane-${personId}`, task });
const job = (over: Partial<Job>): Job => ({ id: "J-1", title: "Add login", ask: "a", status: "running", plan: null, tokenCap: 100, tokensUsed: 5, tokenBase: {}, people: [], createdAt: 1, ...over });
const role = (id: string, department: string): RoleDef => ({ id, title: id.toUpperCase(), department, rank: "staff", cli: "claude", denyTools: [], look: { shirt: "#333333", hair: "#444444", prop: "pen" }, prompt: "", skills: [] });

describe("officeState", () => {
  it.each([["needs", "needs"], ["working", "working"], ["review", "review"], ["idle", "idle"], ["stopped", "stopped"], ["merged", "stopped"]])("%s draws as %s", (from, to) => {
    expect(officeState(status(from))).toBe(to);
  });
  it("is an empty chair with no pane", () => {
    expect(officeState(null)).toBe("stopped");
    expect(officeState(undefined)).toBe("stopped");
  });
  it("is idle when a person is out of usage: the limit is not a question", () => {
    expect(officeState(status("idle", { limit: { what: "x" } }))).toBe("idle");
  });
});

describe("companyUsed", () => {
  it("takes a Job, not a Founder or a floor", () => {
    expect(companyUsed([])).toBe(false);
    expect(companyUsed([job({ status: "rejected" })])).toBe(true);
  });
});

describe("printing", () => {
  const company = { ...emptyCompany(), floors: [{ id: "f", dir: "D:\\f", name: "f" }], people: [...emptyCompany().people, { id: "linh", name: "Linh", role: "developer", rank: "staff", floor: "f", boss: "founder", cli: "claude", look: { shirt: "#123456", hair: "#123456", prop: "none" } }] } as unknown as Company;
  const pane = (printing: boolean): OfficePane => ({ personId: "linh", wsId: "w", paneId: "p", task: { title: "x", status: { state: "working", ask: null } } as OfficePane["task"], printing });
  const linh = (printing: boolean) => officeModel(company, [], [], [pane(printing)]).floors[0].people[0];
  it("comes from the pane, not from the state", () => {
    expect(linh(true).state).toBe("working");
    expect(linh(true).printing).toBe(true);
    expect(linh(false).state).toBe("working");
    expect(linh(false).printing).toBe(false);
  });
  it("changes what the scene must draw", () => {
    expect(sceneKey(officeModel(company, [], [], [pane(true)]))).not.toBe(sceneKey(officeModel(company, [], [], [pane(false)])));
  });
});

describe("looks", () => {
  it("dresses a department alike", () => {
    expect(departmentShirt("Engineering")).toBe(departmentShirt("Engineering"));
    expect(departmentShirt("Engineering")).not.toBe(departmentShirt("Marketing"));
  });
  it("gives a new department a steady colour from the spare list", () => {
    expect(departmentShirt("Sales")).toBe(departmentShirt("Sales"));
    expect(departmentShirt("Sales")).toMatch(/^#[0-9a-f]{6}$/);
  });
  it("gives people different hair by id, the same every time", () => {
    const hairs = new Set(["a1", "b2", "c3", "d4", "e5", "f6"].map(hairFor));
    expect(hairs.size).toBeGreaterThan(1);
    expect(hairFor("a1")).toBe(hairFor("a1"));
  });
  it("uses the department until the look was edited", () => {
    const r = role("qa", "Engineering");
    expect(lookOf(person("x"), r).shirt).toBe(departmentShirt("Engineering"));
    expect(lookOf(person("x", { look: { shirt: "#123456", hair: "#654321", prop: "book", edited: true } }), r)).toEqual({ shirt: "#123456", hair: "#654321", prop: "book" });
  });
  it("keeps the role's prop and the Founder's white shirt, since honey says it is your turn", () => {
    expect(lookOf(person("x"), role("qa", "Engineering")).prop).toBe("laptop");
    expect(lookOf(person("f", { rank: "founder" }), role("founder", "Management")).shirt).toBe("#eef0f4");
  });
  it("falls back to the stored shirt with no role", () => {
    expect(lookOf(person("x"), undefined).shirt).toBe("#111111");
  });
  it("gives a department named like an object key a real colour", () => {
    for (const d of ["constructor", "toString", "__proto__"]) expect(departmentShirt(d)).toMatch(/^#[0-9a-f]{6}$/);
  });
  it("never passes on a colour that is not one", () => {
    const bad = { shirt: "red; background:url(x)", hair: "", prop: "book", edited: true };
    const r = role("qa", "Engineering");
    expect(lookOf(person("x", { look: bad }), r)).toEqual({ shirt: departmentShirt("Engineering"), hair: hairFor("x"), prop: "book" });
    expect(lookOf(person("x", { look: { ...bad, edited: false } }), undefined).shirt).toMatch(/^#[0-9a-f]{6}$/);
    expect(lookOf(person("f", { rank: "founder", look: bad }), r).shirt).toBe("#eef0f4");
  });
  it("keeps the short and the long way to write a colour", () => {
    const look = { shirt: "#ABC", hair: "#aabbcc", prop: "pen", edited: true };
    expect(lookOf(person("x", { look }), undefined)).toEqual({ shirt: "#ABC", hair: "#aabbcc", prop: "pen" });
  });
});

describe("officeModel", () => {
  it("puts the Founder on top, whatever project its pane sits in", () => {
    const m = officeModel(company(), [], [], [pane("founder", status("working"))], { openDirs: ["D:\\a"] });
    expect(m.founder?.state).toBe("working");
    expect(m.floors.flatMap((f) => f.people)).toEqual([]);
  });
  it("does not light a floor because the Founder's pane is there", () => {
    const m = officeModel(company(), [], [], [pane("founder", status("working"))], { openDirs: ["D:\\a"] });
    expect(m.floors.find((f) => f.id === "a")).toMatchObject({ lit: false, open: true });
  });
  it("shows floors with people, and open projects with nobody, and no others", () => {
    const m = officeModel(company([person("x", { floor: "b" })]), [], [], [], { openDirs: ["d:/a/"] });
    expect(m.floors.map((f) => f.id)).toEqual(["a", "b"]);
    expect(officeModel(company(), [], [], []).floors).toEqual([]);
  });
  it("keeps the company's floor order when states change", () => {
    const people = [person("x", { floor: "a" }), person("y", { floor: "b" })];
    const one = officeModel(company(people), [], [], [pane("y", status("needs"))]);
    const two = officeModel(company(people), [], [], [pane("x", status("needs"))]);
    expect(one.floors.map((f) => f.id)).toEqual(two.floors.map((f) => f.id));
  });
  it("seats the Lead first, then people in the order they joined", () => {
    const m = officeModel(company([person("s1"), person("l", { rank: "lead" }), person("s2")]), [], [], []);
    expect(m.floors[0].people.map((p) => [p.id, p.seat])).toEqual([["l", 0], ["s1", 1], ["s2", 2]]);
    expect(m.floors[0].lead).toBe("l");
  });
  it("darkens a closed project and keeps its people", () => {
    const m = officeModel(company([person("x")]), [], [], []);
    expect(m.floors[0]).toMatchObject({ open: false, lit: false });
    expect(m.floors[0].people[0]).toMatchObject({ id: "x", state: "stopped", paneId: null });
  });
  it("lights an open floor with someone at work", () => {
    const m = officeModel(company([person("x")]), [], [], [pane("x", status("idle"))], { openDirs: ["D:\\a"] });
    expect(m.floors[0]).toMatchObject({ open: true, lit: true });
  });
  it("counts papers for the Lead's desk, up to three", () => {
    const people = ["1", "2", "3", "4"].map((n) => person(n));
    const m = officeModel(company(people), [], [], people.map((p) => pane(p.id, status("review"))));
    expect(m.floors[0].papers).toBe(3);
  });
  it("puts what a person asks in Needs you, before merges, after plans", () => {
    const task = { status: { state: "needs", ask: { kind: "run", detail: "rm x", prompt: "", options: [] } } } as unknown as OfficePane["task"];
    const jobs = [job({ id: "J-1", status: "awaiting-plan", plan: { summary: "s", cards: [], hires: [] } }), job({ id: "J-2", status: "awaiting-merge", people: ["x"] })];
    const m = officeModel(company([person("x")]), jobs, [], [pane("x", task)]);
    expect(m.needs.map((r) => r.key)).toEqual(["plan-J-1", "ask-x", "merge-J-2"]);
    expect(m.needs[1]).toMatchObject({ kind: "alert", personId: "x", title: "X wants to run rm x", standing: false });
  });
  it("counts a floor's needs once per person", () => {
    const alert: Alert = { key: "k", job: null, personId: "x", text: "stuck", level: "warn" };
    const m = officeModel(company([person("x")]), [], [alert], [pane("x", status("needs"))]);
    expect(m.floors[0].needs).toBe(1);
    expect(m.counts.needs).toBe(m.needs.length);
  });
  it("lists the Jobs still going, in words", () => {
    const m = officeModel(company(), [job({ status: "running" }), job({ id: "J-2", status: "done" })], [], []);
    expect(m.jobs).toEqual([{ id: "J-1", title: "Add login", status: "running", label: "Running", tokensUsed: 5, tokenCap: 100, cards: null }]);
  });
  it("says how far a Job got from its cards on the boards", () => {
    const m = officeModel(company(), [job({ status: "paused" })], [], [], { jobCards: { "J-1": { done: 2, total: 6 } } });
    expect(m.jobs[0].cards).toEqual({ done: 2, total: 6 });
  });
  it("uses role titles, boards and flights when given them", () => {
    const m = officeModel(company([person("x", { role: "qa" })]), [], [], [], {
      roles: [role("qa", "Engineering")], boards: { a: { todo: 1, doing: 2, done: 3 } }, flights: [{ id: "f", from: "x", to: "founder", at: 1 }],
    });
    expect(m.floors[0].people[0]).toMatchObject({ title: "QA", department: "Engineering" });
    expect(m.floors[0].board).toEqual({ todo: 1, doing: 2, done: 3 });
    expect(m.flights).toHaveLength(1);
  });
  it("makes a readable title from the role id with no role file", () => {
    expect(officeModel(company([person("x")]), [], [], []).floors[0].people[0].title).toBe("Developer");
  });
  it("has no Founder with an empty list of people", () => {
    expect(officeModel({ ...company(), people: [] }, [], [], []).founder).toBeNull();
  });
});

describe("sceneKey", () => {
  const m = (state: string) => officeModel(company([person("x")]), [], [], [pane("x", status(state))], { openDirs: ["D:\\a"] });
  it("is the same for the same picture", () => { expect(sceneKey(m("idle"))).toBe(sceneKey(m("idle"))); });
  it("changes when a person's state does", () => { expect(sceneKey(m("idle"))).not.toBe(sceneKey(m("working"))); });
  it("changes when a floor's name or needs do, which anchors and tags follow", () => {
    const a = m("idle");
    const floor = (over: object) => ({ ...a, floors: a.floors.map((f, i) => (i ? f : { ...f, ...over })) });
    expect(sceneKey(floor({ name: "renamed" }))).not.toBe(sceneKey(a));
    expect(sceneKey(floor({ needs: 3 }))).not.toBe(sceneKey(a));
  });
  it("ignores rail text, which the scene does not draw", () => {
    const a = m("idle");
    expect(sceneKey({ ...a, needs: [] })).toBe(sceneKey({ ...a, needs: [{ key: "k", kind: "alert", jobId: null, personId: null, title: "t", detail: "" }] }));
  });
});
