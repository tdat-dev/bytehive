// src/company/needs.ts
/* What the owner has to act on, in the order worth acting on it. */
import { planCap } from "./org";
import type { Alert } from "./watch";
import type { Floor, Job, JobPlan, NewRole, RoleDef } from "./types";

/** What approving a plan comes to: hired people run with skip permissions, so the
 *  owner reads all of it here, new roles word for word. */
export interface PlanDetail {
  hires: string[];
  floors: Array<{ floor: string; cards: string[] }>;
  tokenCap: number;
  newRoles: NewRole[];
}

export interface NeedRow {
  key: string;
  kind: "plan" | "merge" | "paused" | "alert";
  jobId: string | null;
  personId: string | null;
  title: string;
  detail: string;
  standing?: boolean;
  plan?: PlanDetail;
}

type Names = { floors: Floor[]; roles: RoleDef[] };

const ORDER: Record<NeedRow["kind"], number> = { plan: 0, paused: 1, alert: 2, merge: 3 };
const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;

function planDetail(plan: JobPlan, limit: number, names: Names): PlanDetail {
  const floorName = (id: string) => names.floors.find((f) => f.id === id)?.name ?? id;
  const newRoles = plan.newRoles ?? [];
  const roleTitle = (id: string) => [...names.roles, ...newRoles].find((r) => r.id === id)?.title ?? id;
  return {
    hires: plan.hires.map((h) => `${roleTitle(h.role)} on ${floorName(h.floor)}`),
    floors: [...new Set(plan.cards.map((c) => c.floor))].map((f) => ({ floor: floorName(f), cards: plan.cards.filter((c) => c.floor === f).map((c) => c.title) })),
    tokenCap: planCap(plan, limit),
    newRoles,
  };
}

/** A Job with several people has a branch each: its one row takes the owner to
 *  the next one still to merge, and says how far along the merging is. */
function mergeRow(j: Job, merged: (personId: string) => boolean): NeedRow {
  const summary = j.summary ?? "";
  const n = j.people.length;
  const done = j.people.filter(merged).length;
  const next = n > 1 ? (j.people.find((p) => !merged(p)) ?? null) : (j.people[0] ?? null);
  const left = done === 0 ? `${n} people to merge.` : done === n ? `All ${n} merged.` : `${done} of ${n} merged.`;
  const detail = n > 1 ? [summary, left].filter(Boolean).join(" ") : summary;
  return { key: `merge-${j.id}`, kind: "merge", jobId: j.id, personId: next, title: `Review and merge: ${j.title}`, detail };
}

export function needsYou(jobs: Job[], alerts: Alert[], names: Names = { floors: [], roles: [] }, merged: (personId: string) => boolean = () => false): NeedRow[] {
  const rows: NeedRow[] = [];
  for (const j of jobs) {
    if (j.status === "awaiting-plan" && j.plan) {
      rows.push({
        key: `plan-${j.id}`, kind: "plan", jobId: j.id, personId: null, title: `Approve plan: ${j.title}`,
        detail: `${j.plan.summary} ${plural(j.plan.cards.length, "card")}, ${plural(j.plan.hires.length, "hire")}.`,
        plan: planDetail(j.plan, j.tokenCap, names),
      });
    }
    if (j.status === "paused") {
      rows.push({ key: `paused-${j.id}`, kind: "paused", jobId: j.id, personId: null, title: `Paused: ${j.title}`, detail: "It reached its token cap." });
    }
    if (j.status === "awaiting-merge") {
      rows.push(mergeRow(j, merged));
    }
  }
  for (const a of alerts) {
    if (a.level === "stop") continue; // a paused Job already has its row
    rows.push({ key: a.key, kind: "alert", jobId: a.job, personId: a.personId, title: a.text, detail: "", standing: !!a.standing });
  }
  return rows.sort((a, b) => ORDER[a.kind] - ORDER[b.kind]);
}
