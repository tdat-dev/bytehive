// src/company/reduce.ts
/* Everything that changes the company, as plain functions: state in, state and
 * effects out. engine.ts runs the effects. The app re-checks every rule here,
 * because a request is only a line in a file. */
import { applyHires, canMessage, personById, personByName, validatePlan } from "./org";
import { FOUNDER_ID, type Company, type CompanyRequest, type Job, type Person, type RoleDef } from "./types";

export interface State { company: Company; jobs: Job[]; roles: RoleDef[] }

export type Effect =
  | { kind: "deliver"; personId: string; text: string }
  | { kind: "save-role"; role: RoleDef };

const jobId = (n: number) => `J-${String(n).padStart(4, "0")}`;
const patch = (s: State, id: string, change: Partial<Job>): State => ({
  ...s,
  jobs: s.jobs.map((j) => (j.id === id ? { ...j, ...change } : j)),
});

export function newJob(s: State, ask: string, now: number): { state: State; job: Job; effects: Effect[] } {
  const job: Job = {
    id: jobId(s.company.nextJob), title: ask.trim().split("\n")[0].slice(0, 80), ask, status: "planning", plan: null,
    tokenCap: s.company.limits.tokensPerJob, tokensUsed: 0, tokenBase: {}, people: [], createdAt: now,
  };
  const state = { ...s, company: { ...s.company, nextJob: s.company.nextJob + 1 }, jobs: [...s.jobs, job] };
  return { state, job, effects: [{ kind: "deliver", personId: FOUNDER_ID, text: `JOB ${job.id}: ${ask}` }] };
}

export function applyRequest(s: State, r: CompanyRequest): { state: State; effects: Effect[] } {
  const none = { state: s, effects: [] as Effect[] };
  const from = personById(s.company, r.from);
  if (!from) return none;
  if (r.type === "message") {
    const to = r.to ? personByName(s.company, r.to) : undefined;
    if (!to || !canMessage(s.company, from.id, to.id)) return none;
    return { state: s, effects: [{ kind: "deliver", personId: to.id, text: `[From ${from.name}] ${r.message}` }] };
  }
  if (from.rank !== "founder") return none;
  if (r.type === "role") {
    return { state: { ...s, roles: [...s.roles.filter((x) => x.id !== r.role.id), r.role] }, effects: [{ kind: "save-role", role: r.role }] };
  }
  if (!s.jobs.some((j) => j.id === r.job)) return none;
  if (r.type === "plan") {
    const errors = validatePlan(s.company, r.plan, s.roles);
    if (errors.length) return { state: s, effects: [{ kind: "deliver", personId: FOUNDER_ID, text: `Plan for ${r.job} refused: ${errors.join(" ")}` }] };
    return { state: patch(s, r.job, { status: "awaiting-plan", plan: r.plan }), effects: [] };
  }
  return { state: patch(s, r.job, { status: "awaiting-merge", summary: r.summary }), effects: [] };
}

export function approvePlan(
  s: State,
  id: string,
  pick: { name: (taken: string[]) => string; id: () => string },
): { state: State; hired: Person[]; effects: Effect[] } {
  const job = s.jobs.find((j) => j.id === id);
  if (!job?.plan || job.status !== "awaiting-plan") return { state: s, hired: [], effects: [] };
  const { company, hired } = applyHires(s.company, job.plan, s.roles, pick);
  const names = hired.map((p) => `${p.name} (${p.role})`).join(", ");
  const state = patch({ ...s, company }, id, {
    status: "running",
    tokenCap: job.plan.tokenCap ?? job.tokenCap,
    people: [...new Set([...job.people, ...hired.map((p) => p.id)])],
  });
  const text = `Plan for ${id} approved.${names ? ` Hired: ${names}.` : ""} The cards are on the boards. Brief the Lead of each floor, or the person working alone there.`;
  return { state, hired, effects: [{ kind: "deliver", personId: FOUNDER_ID, text }] };
}

export function rejectPlan(s: State, id: string, note: string): { state: State; effects: Effect[] } {
  const why = note.trim();
  if (!why) return { state: patch(s, id, { status: "rejected", plan: null }), effects: [] };
  return {
    state: patch(s, id, { status: "planning", plan: null }),
    effects: [{ kind: "deliver", personId: FOUNDER_ID, text: `Plan for ${id} sent back: ${why} Make a new plan with job_plan.` }],
  };
}
