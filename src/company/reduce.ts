// src/company/reduce.ts
/* Everything that changes the company, as plain functions: state in, state and
 * effects out. engine.ts runs the effects. The app re-checks every rule here,
 * because a request is only a line in a file. */
import { applyHires, canMessage, fromDraft, personById, personByName, planCap, validatePlan } from "./org";
import { FOUNDER_ID, type Company, type CompanyRequest, type Job, type Person, type RoleDef } from "./types";

export interface State { company: Company; jobs: Job[]; roles: RoleDef[] }

export type Effect =
  | { kind: "deliver"; personId: string; text: string }
  | { kind: "owner"; key: string; text: string }; // shown under Needs you: the owner is no terminal

const jobId = (n: number) => `J-${String(n).padStart(4, "0")}`;
const patch = (s: State, id: string, change: Partial<Job>): State => ({
  ...s,
  jobs: s.jobs.map((j) => (j.id === id ? { ...j, ...change } : j)),
});

const tellFounder = (text: string): Effect => ({ kind: "deliver", personId: FOUNDER_ID, text });

/** A message ends up typed into a terminal: an escape or a carriage return in it
 *  would be a keypress there, not text. Line breaks and tabs are still text. */
const typeable = (text: string) => text.replace(/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/g, "");

export function newJob(s: State, ask: string, now: number): { state: State; job: Job; effects: Effect[] } {
  const job: Job = {
    id: jobId(s.company.nextJob), title: ask.trim().split("\n")[0].slice(0, 80), ask, status: "planning", plan: null,
    tokenCap: s.company.limits.tokensPerJob, tokensUsed: 0, tokenBase: {}, people: [], createdAt: now,
  };
  const state = { ...s, company: { ...s.company, nextJob: s.company.nextJob + 1 }, jobs: [...s.jobs, job] };
  return { state, job, effects: [tellFounder(`JOB ${job.id}: ${ask}`)] };
}

/** The paused or stopped Job that keeps these two from talking. A pause ends, so it
 *  holds everyone on it. A stop doesn't and a message names no Job, so someone on a
 *  running Job as well stays reachable, or one stop would cut a Lead off for good. */
function holdingJob(s: State, from: string, to: string): Job | undefined {
  const onRunning = (id: string) => s.jobs.some((j) => j.status === "running" && j.people.includes(id));
  return s.jobs.find((j) => {
    const on = j.people.includes(from) || j.people.includes(to);
    return on && (j.status === "paused" || (j.status === "stopped" && !onRunning(from) && !onRunning(to)));
  });
}

export function applyRequest(s: State, r: CompanyRequest): { state: State; effects: Effect[] } {
  const none = { state: s, effects: [] as Effect[] };
  const from = personById(s.company, r.from);
  if (!from) return none;
  if (r.type === "message") {
    const to = r.to ? personByName(s.company, r.to) : undefined;
    if (!to || !canMessage(s.company, from.id, to.id)) return none;
    // Nothing reaches or leaves the people of a paused or stopped Job: the message is dropped, and the sender is told so.
    const held = holdingJob(s, from.id, to.id);
    if (held?.status === "paused") return { state: s, effects: [{ kind: "deliver", personId: from.id, text: `Held: your job ${held.id} is paused, so your message to ${to.name} was not sent. Wait for it to be resumed.` }] };
    if (held) return { state: s, effects: [{ kind: "deliver", personId: from.id, text: `Held: your job ${held.id} is stopped, so your message to ${to.name} was not sent. That job will not run again.` }] };
    return { state: s, effects: [{ kind: "deliver", personId: to.id, text: `[From ${from.name}] ${typeable(r.message)}` }] };
  }
  if (from.rank !== "founder") return none;
  const refuse = (text: string) => ({ state: s, effects: [tellFounder(text)] });
  const job = s.jobs.find((j) => j.id === r.job);
  if (!job) return none;
  if (r.type === "plan") {
    if (job.status !== "planning") return refuse(`Plan for ${r.job} refused: the job is ${job.status}, and a plan is only taken while it is planning.`);
    const errors = validatePlan(s.company, r.plan, s.roles);
    if (errors.length) return refuse(`Plan for ${r.job} refused: ${errors.join(" ")}`);
    return { state: patch(s, r.job, { status: "awaiting-plan", plan: r.plan }), effects: [] };
  }
  if (r.type !== "job-done") return none;
  // Nothing was planned, so nobody worked and there is nothing to merge. A plan the owner is reading still has to be answered.
  if (job.status === "planning") {
    return { state: patch(s, r.job, { status: "done", summary: r.summary }), effects: [{ kind: "owner", key: `no-work-${r.job}`, text: `${r.job} needed no work: ${r.summary}` }] };
  }
  if (job.status !== "running") return refuse(`${r.job} cannot be finished: it is ${job.status}, and only a running job, or a planning one that needs no work, can.`);
  return { state: patch(s, r.job, { status: "awaiting-merge", summary: r.summary }), effects: [] };
}

export function approvePlan(
  s: State,
  id: string,
  pick: { name: (taken: string[]) => string; id: () => string },
): { state: State; hired: Person[]; rewired: Person[]; saved: RoleDef[]; effects: Effect[] } {
  const job = s.jobs.find((j) => j.id === id);
  if (!job?.plan || job.status !== "awaiting-plan") return { state: s, hired: [], rewired: [], saved: [], effects: [] };
  // It passed on arrival, but the company may have changed while it waited.
  const errors = validatePlan(s.company, job.plan, s.roles);
  if (errors.length) {
    return {
      state: patch(s, id, { status: "planning", plan: null }),
      hired: [],
      rewired: [],
      saved: [],
      effects: [tellFounder(`Plan for ${id} no longer holds: ${errors.join(" ")} Make a new plan with job_plan.`)],
    };
  }
  // The owner has read them now: from here a drafted role is a role.
  const saved = (job.plan.newRoles ?? []).map(fromDraft);
  const roles = [...s.roles, ...saved];
  const { company, hired } = applyHires(s.company, job.plan, roles, pick);
  const names = hired.map((p) => `${p.name} (${p.role})`).join(", ");
  // Whoever a card names works this Job as much as a new hire does, and so does
  // the Lead of its floor, who hands it out and reviews it. A floor with no Lead
  // has nobody to hand it out: its people are briefed directly, and a stop on an
  // older Job would cut them off if this one left them out.
  const onCards = job.plan.cards.flatMap((c) => {
    const named = c.assignee ? personByName(company, c.assignee) : undefined;
    const floor = company.people.filter((p) => p.floor === c.floor);
    const lead = floor.find((p) => p.rank === "lead");
    return [named?.id, ...(lead ? [lead] : floor).map((p) => p.id)].filter((x): x is string => !!x);
  });
  const state = patch({ ...s, company, roles }, id, {
    status: "running",
    tokenCap: planCap(job.plan, job.tokenCap), // the owner's limit, never the Founder's
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
  return { state, hired, rewired, saved, effects: [...notices, tellFounder(text)] };
}

/** Write down what a person's pane runs on. Gives back the same state when nothing moved. */
export function notePerson(s: State, id: string, seen: Pick<Person, "session" | "worktree" | "branch">): State {
  const p = personById(s.company, id);
  if (!p || (p.session === seen.session && p.worktree === seen.worktree && p.branch === seen.branch)) return s;
  const people = s.company.people.map((x) => (x.id === id ? { ...x, session: seen.session, worktree: seen.worktree, branch: seen.branch } : x));
  return { ...s, company: { ...s.company, people } };
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

const GOING: Job["status"][] = ["planning", "awaiting-plan", "running", "paused"];

/** The owner ends a Job that is still going, planned or not. There is no way back: a new Job is. */
export function haltJob(s: State, id: string): State {
  const status = s.jobs.find((j) => j.id === id)?.status;
  return status && GOING.includes(status) ? patch(s, id, { status: "stopped" }) : s;
}

export const stoppedNote = (id: string): Effect =>
  tellFounder(`${id} was stopped by the owner. It takes no more messages and cannot be finished. Leave it, and do not brief anyone on it.`);

/** A pause is the owner's to lift, and the Founder would otherwise only see its effects. */
export const pausedNote = (id: string): Effect =>
  tellFounder(`${id} reached its token cap and is paused until the owner adds tokens. Its messages are held and it cannot be finished. Wait, you will be told.`);

/** What was held while a Job was paused is gone, and nobody knows it runs again.
 *  Someone stopped is left out: a delivery would start them just to hear it. */
export function resumedNotes(s: State, id: string, isUp: (personId: string) => boolean = () => true): Effect[] {
  const job = s.jobs.find((j) => j.id === id);
  if (!job) return [];
  return [
    tellFounder(`${id} is running again with more tokens. If its work is finished, call job_finish now.`),
    ...job.people.filter(isUp).map((personId): Effect => ({ kind: "deliver", personId, text: `${id} is running again. Anything you sent while it was paused was not delivered: send it again if it still matters.` })),
  ];
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
