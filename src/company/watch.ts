// src/company/watch.ts
/* Agents do not report back on their own, and managers loop. These two checks
 * are what stops a Job from quietly stalling or burning tokens. */
import type { Effect } from "./reduce";
import type { Job, Limits } from "./types";

export interface Alert {
  key: string;
  job: string | null;
  personId: string | null;
  text: string;
  level: "warn" | "stop";
}

export interface WatchedCard {
  id: string;
  title: string;
  job: string;
  assigneeId: string;
  quietSince: number | null; // when the person went idle, null while working
  returns: number;
}

export function watchWork(i: {
  cards: WatchedCard[];
  nudges: Map<string, number>;
  limits: Limits;
  bossOf: (id: string) => string | null;
  now: number;
}): { alerts: Alert[]; effects: Effect[]; nudges: Map<string, number> } {
  const alerts: Alert[] = [];
  const effects: Effect[] = [];
  const nudges = new Map<string, number>();
  const span = i.limits.stallMinutes * 60_000;
  for (const c of i.cards) {
    if (c.returns >= i.limits.maxReturns) {
      alerts.push({ key: `returns-${c.id}`, job: c.job, personId: c.assigneeId, level: "warn", text: `"${c.title}" went back ${c.returns} times and needs you.` });
    }
    if (c.quietSince === null) continue;
    const quiet = i.now - c.quietSince;
    const had = i.nudges.get(c.id) ?? 0;
    nudges.set(c.id, had);
    if (quiet < span * (had + 1)) continue;
    const boss = i.bossOf(c.assigneeId);
    if (had === 0 && boss) {
      nudges.set(c.id, 1);
      effects.push({ kind: "deliver", personId: boss, text: `"${c.title}" has gone quiet for ${i.limits.stallMinutes} minutes. Check on it.` });
    } else {
      alerts.push({ key: `stall-${c.id}`, job: c.job, personId: c.assigneeId, level: "warn", text: `"${c.title}" is stuck.` });
    }
  }
  return { alerts, effects, nudges };
}

export function watchBudget(job: Job): { alert: Alert | null; pause: boolean } {
  if (job.status !== "running") return { alert: null, pause: false };
  const share = job.tokensUsed / job.tokenCap;
  if (share >= 1) {
    return { pause: true, alert: { key: `cap-${job.id}`, job: job.id, personId: null, level: "stop", text: `${job.id} reached its token cap and is paused.` } };
  }
  if (share >= 0.8) {
    return { pause: false, alert: { key: `cap-${job.id}`, job: job.id, personId: null, level: "warn", text: `${job.id} has used ${Math.round(share * 100)}% of its tokens.` } };
  }
  return { alert: null, pause: false };
}
