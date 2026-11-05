// src/company/needs.ts
/* What the owner has to act on, in the order worth acting on it. */
import type { Alert } from "./watch";
import type { Job } from "./types";

export interface NeedRow {
  key: string;
  kind: "plan" | "merge" | "paused" | "alert";
  jobId: string | null;
  personId: string | null;
  title: string;
  detail: string;
}

const ORDER: Record<NeedRow["kind"], number> = { plan: 0, paused: 1, alert: 2, merge: 3 };
const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;

export function needsYou(jobs: Job[], alerts: Alert[]): NeedRow[] {
  const rows: NeedRow[] = [];
  for (const j of jobs) {
    if (j.status === "awaiting-plan" && j.plan) {
      rows.push({
        key: `plan-${j.id}`, kind: "plan", jobId: j.id, personId: null, title: `Approve plan: ${j.title}`,
        detail: `${j.plan.summary} ${plural(j.plan.cards.length, "card")}, ${plural(j.plan.hires.length, "hire")}.`,
      });
    }
    if (j.status === "paused") {
      rows.push({ key: `paused-${j.id}`, kind: "paused", jobId: j.id, personId: null, title: `Paused: ${j.title}`, detail: "It reached its token cap." });
    }
    if (j.status === "awaiting-merge") {
      rows.push({ key: `merge-${j.id}`, kind: "merge", jobId: j.id, personId: j.people[0] ?? null, title: `Review and merge: ${j.title}`, detail: j.summary ?? "" });
    }
  }
  for (const a of alerts) {
    if (a.level === "stop") continue; // a paused Job already has its row
    rows.push({ key: a.key, kind: "alert", jobId: a.job, personId: a.personId, title: a.text, detail: "" });
  }
  return rows.sort((a, b) => ORDER[a.kind] - ORDER[b.kind]);
}
