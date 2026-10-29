// Notices when a working agent is stuck: looping the same failure, or busy
// for a long while without its branch moving. Reads the conversation the
// Grid cards already keep (gridcards.ts) plus the branch's diff fingerprint
// from tasks.ts. Advisory only: it reports evidence, it never stops anything.

import type { ChatItem, StepItem } from "./chatmodel";

/** Same failing action this many times in the recent window counts as a loop. */
export const LOOP_COUNT = 3;
/** Completed steps after your last message looked at for a loop. */
export const LOOP_WINDOW = 12;
/** A branch this long unchanged, while steps land without touching files, counts as a stall. */
export const STALL_MS = 10 * 60_000;
/** Finished steps in a row that touched no files, needed to call it a stall. */
export const STALL_STEPS = 8;

export type StuckKind = "loop" | "stall";

export interface StuckEvidence {
  kind: StuckKind;
  /** Plain words with the count ("same failure 3 times: FAIL x"). */
  text: string;
  count: number;
  /** The repeated error line, or "stall". */
  sample: string;
  /** ms the first matching step ran, 0 when unknown. */
  since: number;
}

/** Finished steps since you last spoke: a new instruction restarts the count. */
function freshSteps(items: ChatItem[]): StepItem[] {
  let start = 0;
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].kind === "user") { start = i + 1; break; }
  }
  return items.slice(start).filter((it): it is StepItem => it.kind === "step" && it.done);
}

/** First error line, with run-specific numbers blanked so "failed after 3
 *  attempts" and "failed after 4 attempts" read as the same failure. */
function errLineOf(s: StepItem): string {
  const first = (s.output ?? "").split(/\r?\n/).map((l) => l.trim()).find(Boolean) ?? "";
  return first.toLowerCase().replace(/\s+/g, " ").replace(/\d[\d.,:]*/g, "#").slice(0, 160);
}

/** The error's own first line for the evidence, else what it ran. */
function showLine(s: StepItem): string {
  const first = (s.output ?? "").split(/\r?\n/).map((l) => l.trim()).find(Boolean) ?? "";
  return (first || s.target).trim().slice(0, 80);
}

export interface Loop {
  count: number;
  sample: string;
  since: number;
}

/** The same failure repeating in the recent steps, or null. Only failures
 *  count: an agent that runs green commands over and over is busy, not
 *  looping. Ties go to the failure seen last. */
export function loopOf(items: ChatItem[]): Loop | null {
  const tail = freshSteps(items).slice(-LOOP_WINDOW);
  const counts = new Map<string, { count: number; sample: string; since: number; last: number }>();
  tail.forEach((s, i) => {
    if (!s.error) return;
    const sig = `${s.tool}‖${s.target}‖${errLineOf(s)}`;
    const cur = counts.get(sig);
    if (cur) { cur.count += 1; cur.sample = showLine(s); cur.last = i; }
    else counts.set(sig, { count: 1, sample: showLine(s), since: s.at, last: i });
  });
  let best: { count: number; sample: string; since: number; last: number } | null = null;
  for (const c of counts.values()) {
    if (c.count < LOOP_COUNT) continue;
    if (!best || c.count > best.count || (c.count === best.count && c.last > best.last)) best = c;
  }
  return best ? { count: best.count, sample: best.sample, since: best.since } : null;
}

/** A step counts as touching files when it says what changed, or its verb is
 *  an edit ("Updated the plan" is not: plans aren't files). */
function touchedFiles(s: StepItem): boolean {
  if ((s.added ?? 0) > 0 || (s.removed ?? 0) > 0 || (s.diff?.length ?? 0) > 0) return true;
  return s.verb === "Edited" || s.verb === "Wrote";
}

/** The recent run of finished steps that touched no files, when it is long
 *  enough to suspect a stall. Null while a step is still open: a long test
 *  running is not stuck. */
export function stallSteps(items: ChatItem[]): StepItem[] | null {
  const steps = items.filter((it): it is StepItem => it.kind === "step");
  const newest = steps[steps.length - 1];
  if (!newest || !newest.done) return null;
  const fresh = freshSteps(items).slice(-STALL_STEPS);
  if (fresh.length < STALL_STEPS || fresh.some(touchedFiles)) return null;
  return fresh;
}

export interface StuckSnap {
  /** pane.spawnedAt: a restart forgets the old run. */
  run: number;
  state: string;
  items: ChatItem[];
  /** Branch diff fingerprint, or null when the diff isn't one agent's. */
  diff: { n: number; add: number; del: number } | null;
  now: number;
}

interface Trail {
  run: number;
  print: string | null;
  stableSince: number;
}

export class StuckWatch {
  private trails = new Map<string, Trail>();

  /** What this agent's stuck verdict is now, or null. Only a working agent
   *  can be stuck; without a conversation there is nothing to read. */
  step(paneId: string, snap: StuckSnap): StuckEvidence | null {
    let t = this.trails.get(paneId);
    if (!t || t.run !== snap.run) {
      t = { run: snap.run, print: null, stableSince: snap.now };
      this.trails.set(paneId, t);
    }
    const print = snap.diff ? `${snap.diff.n}:${snap.diff.add}:${snap.diff.del}` : null;
    if (print !== t.print) { t.print = print; t.stableSince = snap.now; }
    if (snap.state !== "working") return null;
    const loop = loopOf(snap.items);
    if (loop) return { kind: "loop", text: `same failure ${loop.count} times: ${loop.sample}`, count: loop.count, sample: loop.sample, since: loop.since };
    if (snap.diff === null) return null;
    const stalled = stallSteps(snap.items);
    if (!stalled || snap.now - t.stableSince < STALL_MS) return null;
    const mins = Math.floor((snap.now - t.stableSince) / 60_000);
    return { kind: "stall", text: `no file changes in ${mins}m across ${stalled.length} steps`, count: stalled.length, sample: "stall", since: t.stableSince };
  }

  /** Forget agents that are gone (tests; a closed pane). */
  forget(paneId?: string): void {
    if (paneId === undefined) this.trails.clear();
    else this.trails.delete(paneId);
  }
}
