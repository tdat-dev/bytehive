// src/company/reduce.ts
/* Everything that changes the company, as plain functions: state in, state and
 * effects out. engine.ts runs the effects. The app re-checks every rule here,
 * because a request is only a line in a file. */
import { applyHires, canMessage, personById, personByName, validatePlan, validRoleId } from "./org";
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

const tellFounder = (text: string): Effect => ({ kind: "deliver", personId: FOUNDER_ID, text });

export function newJob(s: State, ask: string, now: number): { state: State; job: Job; effects: Effect[] } {
  const job: Job = {
    id: jobId(s.company.nextJob), title: ask.trim().split("\n")[0].slice(0, 80), ask, status: "planning", plan: null,
    tokenCap: s.company.limits.tokensPerJob, tokensUsed: 0, tokenBase: {}, people: [], createdAt: now,
  };
  const state = { ...s, company: { ...s.company, nextJob: s.company.nextJob + 1 }, jobs: [...s.jobs, job] };
  return { state, job, effects: [tellFounder(`JOB ${job.id}: ${ask}`)] };
}

export function applyRequest(s: State, r: CompanyRequest): { state: State; effects: Effect[] } {
  const none = { state: s, effects: [] as Effect[] };
  const from = personById(s.company, r.from);
  if (!from) return none;
  if (r.type === "message") {
    const to = r.to ? personByName(s.company, r.to) : undefined;
    if (!to || !canMessage(s.company, from.id, to.id)) return none;
    // Nothing reaches or leaves a paused Job's people: the message is dropped, and the sender is told so.
    const held = s.jobs.find((j) => j.status === "paused" && (j.people.includes(from.id) || j.people.includes(to.id)));
    if (held) return { state: s, effects: [{ kind: "deliver", personId: from.id, text: `Held: your job ${held.id} is paused, so your message to ${to.name} was not sent. Wait for it to be resumed.` }] };
    return { state: s, effects: [{ kind: "deliver", personId: to.id, text: `[From ${from.name}] ${r.message}` }] };
  }
  if (from.rank !== "founder") return none;
  const refuse = (text: string) => ({ state: s, effects: [tellFounder(text)] });
  if (r.type === "role") {
    if (!validRoleId(r.role.id)) return refuse(`Role ${r.role.id} refused: an id is 2 to 31 lowercase letters, digits or dashes, starting with a letter.`);
    if (s.roles.some((x) => x.id === r.role.id)) return refuse(`Role ${r.role.id} refused: that role exists already. Use it, or propose another id.`);
    const role: RoleDef = { ...r.role, rank: "staff", cli: "claude", denyTools: [] };
    return { state: { ...s, roles: [...s.roles, role] }, effects: [{ kind: "save-role", role }] };
  }
  const job = s.jobs.find((j) => j.id === r.job);
  if (!job) return none;
  if (r.type === "plan") {
    if (job.status !== "planning") return refuse(`Plan for ${r.job} refused: the job is ${job.status}, and a plan is only taken while it is planning.`);
    const errors = validatePlan(s.company, r.plan, s.roles);
    if (errors.length) return refuse(`Plan for ${r.job} refused: ${errors.join(" ")}`);
    return { state: patch(s, r.job, { status: "awaiting-plan", plan: r.plan }), effects: [] };
  }
  if (job.status !== "running") return refuse(`${r.job} cannot be finished: it is ${job.status}, and only a running job can.`);
  return { state: patch(s, r.job, { status: "awaiting-merge", summary: r.summary }), effects: [] };
}

export function approvePlan(
  s: State,
  id: string,
  pick: { name: (taken: string[]) => string; id: () => string },
): { state: State; hired: Person[]; rewired: Person[]; effects: Effect[] } {
  const job = s.jobs.find((j) => j.id === id);
  if (!job?.plan || job.status !== "awaiting-plan") return { state: s, hired: [], rewired: [], effects: [] };
  // It passed on arrival, but the company may have changed while it waited.
  const errors = validatePlan(s.company, job.plan, s.roles);
  if (errors.length) {
    return {
      state: patch(s, id, { status: "planning", plan: null }),
      hired: [],
      rewired: [],
      effects: [tellFounder(`Plan for ${id} no longer holds: ${errors.join(" ")} Make a new plan with job_plan.`)],
    };
  }
  const { company, hired } = applyHires(s.company, job.plan, s.roles, pick);
  const names = hired.map((p) => `${p.name} (${p.role})`).join(", ");
  // Whoever a card names works this Job as much as a new hire does; a card with
  // nobody named is the floor Lead's to hand out.
  const onCards = job.plan.cards.flatMap((c) => {
    const who = c.assignee ? personByName(company, c.assignee) : company.people.find((p) => p.floor === c.floor && p.rank === "lead");
    return who ? [who.id] : [];
  });
  const state = patch({ ...s, company }, id, {
    status: "running",
    tokenCap: Math.min(job.plan.tokenCap ?? job.tokenCap, job.tokenCap), // the owner's limit, never the Founder's
    people: [...new Set([...job.people, ...hired.map((p) => p.id), ...onCards])],
  });
  // A new Lead takes over the floor. Whoever was there keeps a session that says
  // otherwise, and would refuse the Lead's orders until they hear it.
  const rewired = company.people.filter((p) => personById(s.company, p.id) && personById(s.company, p.id)?.boss !== p.boss);
  const notices = rewired.map((p): Effect => ({
    kind: "deliver",
    personId: p.id,
    text: `[From Founder] You have a new boss: ${p.boss ? personById(company, p.boss)?.name : "nobody"}. Take your cards from them and report to them from now on.`,
  }));
  const text = `Plan for ${id} approved.${names ? ` Hired: ${names}.` : ""} The cards are on the boards. Brief the Lead of each floor, or the person working alone there.`;
  return { state, hired, rewired, effects: [...notices, tellFounder(text)] };
}

/** Add someone who picked up a card of a running Job. */
export function joinJob(s: State, id: string, personId: string): State {
  const job = s.jobs.find((j) => j.id === id);
  if (job?.status !== "running" || job.people.includes(personId)) return s;
  return patch(s, id, { people: [...job.people, personId] });
}

/** Stop a running Job, usually because it reached its token cap. */
export function pauseJob(s: State, id: string): State {
  if (s.jobs.find((j) => j.id === id)?.status !== "running") return s;
  return patch(s, id, { status: "paused" });
}

/** Carry on a paused Job, with `extra` more tokens on its cap. */
export function resumeJob(s: State, id: string, extra: number): State {
  const job = s.jobs.find((j) => j.id === id);
  if (job?.status !== "paused") return s;
  return patch(s, id, { status: "running", tokenCap: job.tokenCap + extra });
}

/** The owner has reviewed and merged what a Job made: it is done. Only a Job
 *  that is waiting for that can be closed. */
export function closeJob(s: State, id: string): State {
  if (s.jobs.find((j) => j.id === id)?.status !== "awaiting-merge") return s;
  return patch(s, id, { status: "done" });
}

/** Add what went wrong while carrying out a plan to what the Founder is told. */
export function reportFailures(effects: Effect[], failed: string[]): Effect[] {
  if (!failed.length) return effects;
  const note = ` But this went wrong: ${failed.join(" ")}`;
  return effects.map((e) => (e.kind === "deliver" && e.personId === FOUNDER_ID ? { ...e, text: e.text + note } : e));
}

export function rejectPlan(s: State, id: string, note: string): { state: State; effects: Effect[] } {
  if (s.jobs.find((j) => j.id === id)?.status !== "awaiting-plan") return { state: s, effects: [] };
  const why = note.trim();
  if (!why) return { state: patch(s, id, { status: "rejected", plan: null }), effects: [] };
  return {
    state: patch(s, id, { status: "planning", plan: null }),
    effects: [tellFounder(`Plan for ${id} sent back: ${why} Make a new plan with job_plan.`)],
  };
}
