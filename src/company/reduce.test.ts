// src/company/reduce.test.ts
import { describe, it, expect } from "vitest";
import { applyRequest, approvePlan, joinJob, newJob, closeJob, pausedNote, pauseJob, rejectPlan, reportFailures, resumedNotes, resumeJob, type State } from "./reduce";
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
const waiting = () => applyRequest(newJob(state(), "x", 1).state, { type: "plan", ts: 2, from: FOUNDER_ID, job: "J-0001", plan }).state;
const running = () => approvePlan(waiting(), "J-0001", pick).state;
const closed = () => rejectPlan(waiting(), "J-0001", "").state;

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
  it("takes control characters out of a message: it is typed into a terminal", () => {
    const message = "done\u001b[201~\r\nrm\u0007 -rf\u0000\u007f\u0085\n\tnext";
    expect(applyRequest(state(), { type: "message", ts: 1, from: "d1", to: "Hai", message }).effects).toEqual([
      { kind: "deliver", personId: "l1", text: "[From Linh] done[201~\nrm -rf\n\tnext" },
    ]);
  });
  it("does not pass on messages from people on a paused job, and tells the sender", () => {
    const s = newJob(state(), "x", 1).state;
    s.jobs[0] = { ...s.jobs[0], status: "paused", people: ["d1"] };
    expect(applyRequest(s, { type: "message", ts: 1, from: "d1", to: "Hai", message: "done" }).effects).toEqual([
      { kind: "deliver", personId: "d1", text: expect.stringMatching(/^Held: your job J-0001 is paused.*Hai/) },
    ]);
  });
  it("still delivers a message from someone who is not on the paused job", () => {
    const s = newJob(state(), "x", 1).state;
    s.jobs[0] = { ...s.jobs[0], status: "paused", people: ["l1"] };
    expect(applyRequest(s, { type: "message", ts: 1, from: "d1", to: "Hai", message: "done" }).effects).toHaveLength(1);
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
    expect(applyRequest(running(), { type: "job-done", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" }).state.jobs[0]).toMatchObject({ status: "awaiting-merge", summary: "ok" });
  });
  it("ignores a request type it does not know, even from the Founder", () => {
    const s = running();
    const r = applyRequest(s, { type: "mystery", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" } as never);
    expect(r).toEqual({ state: s, effects: [] });
    expect(r.state.jobs[0].status).toBe("running");
  });
  it("refuses a second plan on a running job, so nobody is hired twice", () => {
    const s = running();
    const r = applyRequest(s, { type: "plan", ts: 3, from: FOUNDER_ID, job: "J-0001", plan });
    expect(r.state).toBe(s);
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("running") });
  });
  it("refuses a plan while another waits for approval", () => {
    const s = waiting();
    const r = applyRequest(s, { type: "plan", ts: 3, from: FOUNDER_ID, job: "J-0001", plan });
    expect(r.state).toBe(s);
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("awaiting-plan") });
  });
  it("does not bring a rejected job back with a plan", () => {
    const s = closed();
    const r = applyRequest(s, { type: "plan", ts: 3, from: FOUNDER_ID, job: "J-0001", plan });
    expect(r.state).toBe(s);
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("rejected") });
  });
  it("refuses job-done during planning, so a job can't skip approval", () => {
    const s = newJob(state(), "x", 1).state;
    const r = applyRequest(s, { type: "job-done", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" });
    expect(r.state).toBe(s);
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("planning") });
  });
  it("does not bring a rejected job back with job-done", () => {
    const s = closed();
    const r = applyRequest(s, { type: "job-done", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" });
    expect(r.state).toBe(s);
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("rejected") });
  });
  it("saves a proposed role and adds it to the list", () => {
    const r = applyRequest(state(), { type: "role", ts: 1, from: FOUNDER_ID, role: { ...role, id: "analyst" }, job: "J-0001" });
    expect(r.state.roles.map((x) => x.id)).toContain("analyst");
    expect(r.effects).toEqual([{ kind: "save-role", role: expect.objectContaining({ id: "analyst" }) }]);
  });
  it("gives a proposed role the plain rights whatever it asked for", () => {
    const r = applyRequest(state(), { type: "role", ts: 1, from: FOUNDER_ID, role: { ...role, id: "analyst", rank: "founder", cli: "powershell", denyTools: ["Read"] }, job: "J-0001" });
    expect(r.state.roles.find((x) => x.id === "analyst")).toMatchObject({ rank: "staff", cli: "claude", denyTools: [] });
    expect(r.effects[0]).toMatchObject({ kind: "save-role", role: { rank: "staff", cli: "claude", denyTools: [] } });
  });
  it("refuses to replace a role that exists", () => {
    const s = state();
    const r = applyRequest(s, { type: "role", ts: 1, from: FOUNDER_ID, role: { ...role, prompt: "new" }, job: "J-0001" });
    expect(r.state).toBe(s);
    expect(r.effects).toEqual([{ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("marketing") }]);
  });
  it.each(["a", "Analyst", "9lives", "..\\evil", "a b", `a${"b".repeat(31)}`])("refuses the role id %s", (id) => {
    const s = state();
    const r = applyRequest(s, { type: "role", ts: 1, from: FOUNDER_ID, role: { ...role, id }, job: "J-0001" });
    expect(r.state).toBe(s);
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID });
  });
  it("refuses a role with no id at all", () => {
    const r = applyRequest(state(), { type: "role", ts: 1, from: FOUNDER_ID, role: { ...role, id: undefined as unknown as string }, job: "J-0001" });
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID });
  });
});

describe("approvePlan and rejectPlan", () => {
  it("hires, starts the job and tells the Founder", () => {
    const r = approvePlan(waiting(), "J-0001", pick);
    expect(r.hired.map((p) => p.name)).toEqual(["Vy"]);
    expect(r.state.jobs[0].status).toBe("running");
    expect(r.state.jobs[0].people).toContain("p9");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("approved") });
  });
  it("counts someone already here who a card names as working the job", () => {
    const s = waiting();
    s.jobs[0].plan = { ...plan, cards: [{ floor: "a", title: "t", desc: "d", checklist: [], assignee: "Linh" }] };
    expect(approvePlan(s, "J-0001", pick).state.jobs[0].people).toEqual(["p9", "d1", "l1"]);
  });
  it("counts the Lead of every floor with a card: they run the floor, whoever does the card", () => {
    const s = waiting();
    s.jobs[0].plan = { ...plan, hires: [], cards: [{ floor: "a", title: "t", desc: "d", checklist: [] }] };
    expect(approvePlan(s, "J-0001", pick).state.jobs[0].people).toEqual(["l1"]);
    const named = waiting();
    named.jobs[0].plan = { ...plan, hires: [], cards: [{ floor: "a", title: "t", desc: "d", checklist: [], assignee: "Linh" }] };
    expect(approvePlan(named, "J-0001", pick).state.jobs[0].people).toEqual(["d1", "l1"]);
  });
  it("tells staff who get a new boss from a new Lead, and says who was moved", () => {
    const s = waiting();
    const alone: Person = { ...dev, id: "d2", name: "Mai", floor: "b", boss: FOUNDER_ID };
    s.company.people.push(alone);
    s.roles.push({ ...role, id: "lead", rank: "lead" });
    s.jobs[0].plan = { ...plan, hires: [{ role: "lead", floor: "b" }] };
    const r = approvePlan(s, "J-0001", pick);
    expect(r.rewired.map((p) => [p.name, p.boss])).toEqual([["Mai", "p9"]]);
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: "d2", text: expect.stringContaining("Vy") });
    expect(r.effects[1]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID });
  });
  it("keeps a plan's token cap inside the owner's limit", () => {
    const s = waiting();
    const limit = s.jobs[0].tokenCap;
    s.jobs[0].plan = { ...plan, tokenCap: limit * 10 };
    expect(approvePlan(s, "J-0001", pick).state.jobs[0].tokenCap).toBe(limit);
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
  it("leaves a job alone unless it is waiting for approval", () => {
    const s = running();
    expect(rejectPlan(s, "J-0001", "changed my mind")).toEqual({ state: s, effects: [] });
    expect(rejectPlan(s, "J-0001", "")).toEqual({ state: s, effects: [] });
  });
  it("sends a plan back when the second of two would give a floor two Leads", () => {
    const leadRole: RoleDef = { ...role, id: "lead", rank: "lead" };
    const twoLeads: JobPlan = { summary: "Lead for b", cards: [], hires: [{ role: "lead", floor: "b" }] };
    let s: State = { ...state(), roles: [role, leadRole] };
    s = newJob(s, "one", 1).state;
    s = newJob(s, "two", 2).state;
    s = applyRequest(s, { type: "plan", ts: 3, from: FOUNDER_ID, job: "J-0001", plan: twoLeads }).state;
    s = applyRequest(s, { type: "plan", ts: 4, from: FOUNDER_ID, job: "J-0002", plan: twoLeads }).state;
    s = approvePlan(s, "J-0001", pick).state;
    const r = approvePlan(s, "J-0002", pick);
    expect(r.hired).toEqual([]);
    expect(r.state.company.people.filter((p) => p.rank === "lead" && p.floor === "b").length).toBe(1);
    expect(r.state.jobs[1]).toMatchObject({ status: "planning", plan: null });
    expect(r.effects).toEqual([{ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("one Lead") }]);
  });
  it("sends a plan back when the company filled up while it waited", () => {
    const s = waiting();
    s.company.limits = { ...s.company.limits, peopleTotal: 3 };
    const r = approvePlan(s, "J-0001", pick);
    expect(r.hired).toEqual([]);
    expect(r.state.jobs[0].status).toBe("planning");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("limited to 3") });
  });
  it("sends a plan back when its role was removed, instead of crashing", () => {
    const r = approvePlan({ ...waiting(), roles: [] }, "J-0001", pick);
    expect(r.hired).toEqual([]);
    expect(r.state.jobs[0].status).toBe("planning");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("marketing") });
  });
});

describe("reportFailures", () => {
  const told = [{ kind: "deliver" as const, personId: FOUNDER_ID, text: "Plan approved." }];
  it("leaves the message as it is when nothing failed", () => {
    expect(reportFailures(told, [])).toBe(told);
  });
  it("adds what failed to the message", () => {
    expect(reportFailures(told, ["Vy did not start.", "No cards on b."])[0]).toMatchObject({
      text: expect.stringMatching(/^Plan approved\..*Vy did not start\. No cards on b\.$/),
    });
  });
  it("tells only the Founder", () => {
    const staff = { kind: "deliver" as const, personId: "d1", text: "New boss." };
    expect(reportFailures([staff, ...told], ["Vy did not start."])[0]).toBe(staff);
  });
});

describe("joinJob", () => {
  it("adds someone who picked up a card of a running job", () => {
    expect(joinJob(running(), "J-0001", "d1").jobs[0].people).toEqual(["p9", "d1"]);
  });
  it("gives the same state back when nothing changes", () => {
    const s = running();
    expect(joinJob(s, "J-0001", "p9")).toBe(s);
    expect(joinJob(s, "J-9999", "d1")).toBe(s);
    const done = applyRequest(s, { type: "job-done", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" }).state;
    expect(joinJob(done, "J-0001", "d1")).toBe(done);
  });
});

describe("what people hear when a job stops and starts", () => {
  it("tells the Founder why a job stopped, so they don't guess", () => {
    expect(pausedNote("J-0001")).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringMatching(/J-0001.*token cap/) });
  });
  it("does not wake someone stopped just to say it runs again", () => {
    const s = joinJob(running(), "J-0001", "d1");
    const told = resumedNotes(s, "J-0001", (id) => id === "d1");
    expect(told.map((e) => e.kind === "deliver" && e.personId)).toEqual([FOUNDER_ID, "d1"]);
  });
  it("tells the Founder and everyone on it that it runs again: what was held is gone", () => {
    const s = joinJob(running(), "J-0001", "d1");
    const told = resumedNotes(s, "J-0001");
    expect(told.map((e) => e.kind === "deliver" && e.personId)).toEqual([FOUNDER_ID, "p9", "d1"]);
    expect(told[0]).toMatchObject({ text: expect.stringMatching(/J-0001.*job_finish/) });
    expect(told[1]).toMatchObject({ text: expect.stringMatching(/J-0001.*again/) });
  });
});

describe("a paused job", () => {
  it("holds a message to someone on it, and tells the sender", () => {
    const s = pauseJob(joinJob(running(), "J-0001", "l1"), "J-0001");
    const r = applyRequest(s, { type: "message", ts: 2, from: FOUNDER_ID, to: "Hai", message: "go" });
    expect(r.effects).toEqual([{ kind: "deliver", personId: FOUNDER_ID, text: expect.stringMatching(/^Held: .*J-0001.*Hai/) }]);
  });
});

describe("pauseJob and resumeJob", () => {
  it("pauses a running job", () => {
    expect(pauseJob(running(), "J-0001").jobs[0].status).toBe("paused");
  });
  it("leaves a job that is not running alone", () => {
    const s = waiting();
    expect(pauseJob(s, "J-0001")).toBe(s);
    const r = running();
    expect(pauseJob(r, "J-9999")).toBe(r);
  });
  it("resumes a paused job with a higher cap", () => {
    const s = pauseJob(running(), "J-0001");
    const cap = s.jobs[0].tokenCap;
    expect(resumeJob(s, "J-0001", 1000).jobs[0]).toMatchObject({ status: "running", tokenCap: cap + 1000 });
  });
  it("acts only on a paused job", () => {
    const s = running();
    expect(resumeJob(s, "J-0001", 1000)).toBe(s);
    expect(resumeJob(s, "J-9999", 1000)).toBe(s);
  });
});

describe("closeJob", () => {
  const merging = () => applyRequest(running(), { type: "job-done", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" }).state;
  it("moves a job waiting for merge to done", () => {
    expect(closeJob(merging(), "J-0001").jobs[0].status).toBe("done");
  });
  it("refuses a job that is not waiting for merge, or does not exist", () => {
    const s = running();
    expect(closeJob(s, "J-0001")).toBe(s);
    expect(closeJob(s, "J-9999")).toBe(s);
    const done = closeJob(merging(), "J-0001");
    expect(closeJob(done, "J-0001")).toBe(done);
  });
});
