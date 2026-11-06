// src/company/watch.ts
/* Agents do not report back on their own, and managers loop. These two checks
 * are what stops a Job from quietly stalling or burning tokens. */
import type { Card } from "../board";
import type { Effect } from "./reduce";
import type { Job, Limits } from "./types";

export interface Alert {
  key: string;
  job: string | null;
  personId: string | null;
  text: string;
  level: "warn" | "stop";
  standing?: boolean; // stays until dismissed, see alerts.ts
}

export interface WatchedCard {
  id: string;
  title: string;
  job: string;
  assigneeId: string;
  quietSince: number | null; // when the person went idle, null while working
  returns: number;
  list?: "doing" | "todo";
}

const cardsOf = (board: WatchBoard, title: string) =>
  board.lists.find((l) => l.title.trim().toLowerCase() === title)?.cards ?? [];

export interface WatchBoard { lists: Array<{ title: string; cards: Card[] }> }

/** The cards of running Jobs on one board that could go quiet. A To do card is lost
 *  when nobody has it: a Job with a card in Doing is moving, and its To do cards are
 *  only waiting their turn. Times are kept per list, so moving on starts them over. */
export function cardsToWatch(i: {
  board: WatchBoard;
  jobs: Job[];
  contact: string | null;
  isWorking: (personId: string) => boolean;
  quiet: Map<string, number>;
  now: number;
}): { cards: WatchedCard[]; seen: string[] } {
  const cards: WatchedCard[] = [];
  const seen: string[] = [];
  const running = (job: string | undefined) => !!job && i.jobs.find((j) => j.id === job)?.status === "running";
  const doing = cardsOf(i.board, "doing");
  for (const c of doing) {
    const key = `doing:${c.id}`;
    seen.push(key);
    if (!c.job || !c.assigneeId) continue;
    if (!running(c.job)) { i.quiet.delete(key); continue; }
    if (i.isWorking(c.assigneeId)) i.quiet.delete(key);
    else if (!i.quiet.has(key)) i.quiet.set(key, i.now);
    cards.push({ id: c.id, title: c.title, job: c.job, assigneeId: c.assigneeId, quietSince: i.quiet.get(key) ?? null, returns: c.returns ?? 0, list: "doing" });
  }
  for (const c of cardsOf(i.board, "to do")) {
    const key = `todo:${c.id}`;
    seen.push(key);
    const who = c.assigneeId ?? i.contact;
    if (!c.job || !who) continue;
    if (!running(c.job) || doing.some((d) => d.job === c.job)) { i.quiet.delete(key); continue; }
    if (!i.quiet.has(key)) i.quiet.set(key, i.now);
    cards.push({ id: c.id, title: c.title, job: c.job, assigneeId: who, quietSince: i.quiet.get(key) ?? null, returns: c.returns ?? 0, list: "todo" });
  }
  return { cards, seen };
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
    const waiting = c.list === "todo";
    const nudge = waiting ? `"${c.title}" has waited in To do for ${i.limits.stallMinutes} minutes. Hand it out or start it.` : `"${c.title}" has gone quiet for ${i.limits.stallMinutes} minutes. Check on it.`;
    const stuck = waiting ? `"${c.title}" has not been picked up.` : `"${c.title}" is stuck.`;
    if (had === 0 && boss) {
      nudges.set(c.id, 1);
      effects.push({ kind: "deliver", personId: boss, text: nudge });
    } else {
      alerts.push({ key: `stall-${c.id}`, job: c.job, personId: c.assigneeId, level: "warn", text: stuck });
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
