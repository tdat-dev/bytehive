// src/company/reduce.test.ts
import { describe, it, expect } from "vitest";
import { applyRequest, approvePlan, newJob, rejectPlan, type State } from "./reduce";
import { emptyCompany } from "./org";
import { FOUNDER_ID, type JobPlan, type Person, type RoleDef } from "./types";

const look = { shirt: "#fff", hair: "#000", prop: "none" };
const role: RoleDef = { id: "marketing", title: "M", department: "M", rank: "staff", cli: "claude", denyTools: [], look, prompt: "p", skills: [] };
const lead: Person = { id: "l1", name: "Hai", role: "lead", rank: "lead", floor: "a", boss: FOUNDER_ID, cli: "claude", look };
const dev: Person = { id: "d1", name: "Linh", role: "developer", rank: "staff", floor: "a", boss: "l1", cli: "claude", look };
const state = (): State => ({
  company: { ...emptyCompany(), floors: [{ id: "a", dir: "D:\\a", name: "a" }, { id: "b", dir: "D:\\b", name: "b" }], people: [...emptyCompany().people, lead, dev] },
  jobs: [],
  roles: [role],
});
const plan: JobPlan = { summary: "Add marketing", cards: [], hires: [{ role: "marketing", floor: "b" }] };
const pick = { name: () => "Vy", id: () => "p9" };

describe("newJob", () => {
  it("numbers jobs and wakes the Founder", () => {
    const { state: s, job, effects } = newJob(state(), "add marketing to b", 100);
    expect(job.id).toBe("J-0001");
    expect(s.company.nextJob).toBe(2);
    expect(job.status).toBe("planning");
    expect(job.tokenCap).toBe(5_000_000);
    expect(effects).toEqual([{ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("JOB J-0001") }]);
  });
});

describe("applyRequest", () => {
  it("delivers a message between boss and report with who sent it", () => {
    const { effects } = applyRequest(state(), { type: "message", ts: 1, from: "d1", to: "Hai", message: "done" });
    expect(effects).toEqual([{ kind: "deliver", personId: "l1", text: "[From Linh] done" }]);
  });
  it("drops a forged message that breaks the hierarchy", () => {
    expect(applyRequest(state(), { type: "message", ts: 1, from: "d1", to: "Founder", message: "hi" }).effects).toEqual([]);
  });
  it("puts a valid plan up for approval", () => {
    const s = newJob(state(), "x", 1).state;
    const r = applyRequest(s, { type: "plan", ts: 2, from: FOUNDER_ID, job: "J-0001", plan });
    expect(r.state.jobs[0].status).toBe("awaiting-plan");
    expect(r.state.jobs[0].plan).toEqual(plan);
  });
  it("refuses a plan from anyone but the Founder", () => {
    const s = newJob(state(), "x", 1).state;
    expect(applyRequest(s, { type: "plan", ts: 2, from: "l1", job: "J-0001", plan }).state.jobs[0].status).toBe("planning");
  });
  it("tells the Founder when a plan fails on arrival", () => {
    const s = newJob(state(), "x", 1).state;
    const bad = { ...plan, hires: [{ role: "ghost", floor: "b" }] };
    const r = applyRequest(s, { type: "plan", ts: 2, from: FOUNDER_ID, job: "J-0001", plan: bad });
    expect(r.state.jobs[0].status).toBe("planning");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("ghost") });
  });
  it("marks a finished job for merge", () => {
    const s = newJob(state(), "x", 1).state;
    expect(applyRequest(s, { type: "job-done", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" }).state.jobs[0]).toMatchObject({ status: "awaiting-merge", summary: "ok" });
  });
  it("saves a proposed role and adds it to the list", () => {
    const r = applyRequest(state(), { type: "role", ts: 1, from: FOUNDER_ID, role: { ...role, id: "analyst" }, job: "J-0001" });
    expect(r.state.roles.map((x) => x.id)).toContain("analyst");
    expect(r.effects).toEqual([{ kind: "save-role", role: expect.objectContaining({ id: "analyst" }) }]);
  });
});

describe("approvePlan and rejectPlan", () => {
  const waiting = () => applyRequest(newJob(state(), "x", 1).state, { type: "plan", ts: 2, from: FOUNDER_ID, job: "J-0001", plan }).state;
  it("hires, starts the job and tells the Founder", () => {
    const r = approvePlan(waiting(), "J-0001", pick);
    expect(r.hired.map((p) => p.name)).toEqual(["Vy"]);
    expect(r.state.jobs[0].status).toBe("running");
    expect(r.state.jobs[0].people).toContain("p9");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("approved") });
  });
  it("uses the plan's token cap when it has one", () => {
    const s = waiting();
    s.jobs[0].plan = { ...plan, tokenCap: 900 };
    expect(approvePlan(s, "J-0001", pick).state.jobs[0].tokenCap).toBe(900);
  });
  it("sends a rejection with its reason back to the Founder and replans", () => {
    const r = rejectPlan(waiting(), "J-0001", "too many people");
    expect(r.state.jobs[0].status).toBe("planning");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("too many people") });
  });
  it("closes the job when rejected with no reason", () => {
    expect(rejectPlan(waiting(), "J-0001", "").state.jobs[0].status).toBe("rejected");
  });
});
