// When an agent finishes a piece of work, say so: done, ready to review, or
// stopped with an error. The "needs you" notification covers an agent that is
// waiting on a question; this covers the end of a run.
//
// A run is one stretch of work: it opens when the agent starts working and
// stays open through quiet moments and questions (needs), and ends when the
// agent has sat idle, with changes, or stopped for a few seconds. Short runs
// are noise (a reply to "hi", a slash command) and say nothing. Finishes that
// land within a few seconds of each other go out as one notification.
// No DOM, no IPC: unit-tested on its own (finishwatch.ts wires it up).

import type { TaskState } from "./taskstate";

export type FinishKind = "done" | "review" | "crash";

/** What a finish check needs to know about one agent right now. */
export interface FinishSnap {
  paneId: string;
  name: string;
  project: string;
  state: TaskState;
  /** It is sitting at its plan's usage limit: Limit Guard tells you that. */
  limited: boolean;
  /** Files changed on its branch, when known. */
  files: number | null;
  /** Exit code of its process once it stopped, when known. */
  exitCode: number | null;
  /** You (or Maestro, for a merge or quit) asked it to stop just now. */
  stopAsked: boolean;
}

export interface Finish {
  paneId: string;
  name: string;
  project: string;
  kind: FinishKind;
  files: number | null;
  exitCode: number | null;
  /** How long the run took, ms. */
  ranMs: number;
  /** When it finished, ms. */
  at: number;
}

/** Runs shorter than this say nothing. */
export const MIN_RUN_MS = 20_000;
/** How long an agent must stay quiet before its run counts as over: a pause
 *  between two tool calls is not the end of the job. */
export const SETTLE_MS = 5_000;
/** A burst closes after this long without another finish... */
export const GROUP_MS = 3_000;
/** ...or this long after its first one, whichever comes first. */
export const GROUP_MAX_MS = 10_000;

interface Run {
  start: number;
  /** When it stopped working, while that may still be a pause. */
  quietSince: number | null;
}

export class FinishWatch {
  private runs = new Map<string, Run>();
  private last = new Map<string, { kind: FinishKind; at: number }>();

  /** Look at every agent once (call each tick). Returns the runs that just ended
   *  with something worth telling. */
  step(snaps: FinishSnap[], now: number): Finish[] {
    const out: Finish[] = [];
    const seen = new Set<string>();
    for (const s of snaps) {
      seen.add(s.paneId);
      let run = this.runs.get(s.paneId);
      if (s.state === "working") {
        if (!run) {
          run = { start: now, quietSince: null };
          this.runs.set(s.paneId, run);
          this.last.delete(s.paneId); // on something new: its last finish is old news
        }
        run.quietSince = null;
        continue;
      }
      if (!run) continue;
      // A question mid-task is part of the same run (needs-you speaks for it).
      if (s.state === "needs") { run.quietSince = null; continue; }
      run.quietSince ??= now;
      const gone = s.state === "stopped" || s.state === "merged";
      if (!gone && now - run.quietSince < SETTLE_MS) continue;
      this.runs.delete(s.paneId);
      const kind = kindOf(s);
      if (!kind || run.quietSince - run.start < MIN_RUN_MS) continue;
      const f: Finish = {
        paneId: s.paneId, name: s.name, project: s.project, kind,
        files: s.files, exitCode: s.exitCode, ranMs: run.quietSince - run.start, at: run.quietSince,
      };
      this.last.set(s.paneId, { kind, at: f.at });
      out.push(f);
    }
    for (const id of [...this.runs.keys()]) if (!seen.has(id)) this.runs.delete(id);
    for (const id of [...this.last.keys()]) if (!seen.has(id)) this.last.delete(id);
    return out;
  }

  /** The agent's latest finish, until it starts working again. */
  lastFinish(paneId: string): { kind: FinishKind; at: number } | null {
    return this.last.get(paneId) ?? null;
  }

  clear(): void {
    this.runs.clear();
    this.last.clear();
  }
}

/** What an ended run comes to, or null when there is nothing to tell. */
function kindOf(s: FinishSnap): FinishKind | null {
  if (s.state === "stopped") return s.exitCode !== null && s.exitCode !== 0 && !s.stopAsked ? "crash" : null;
  if (s.state === "merged" || s.limited) return null;
  if (s.state === "review") return "review";
  if (s.state === "idle") return "done";
  return null;
}

/** Tell about this finish? Not when you are looking right at that agent. */
export function worthTelling(f: Finish, onScreen: string | null, focused: boolean): boolean {
  return !(focused && f.paneId === onScreen);
}

/** Collects finishes into bursts: several agents done at once is one notification. */
export class Burst {
  private items: Finish[] = [];
  private first = 0;
  private latest = 0;

  add(list: Finish[], now: number): void {
    if (!list.length) return;
    if (!this.items.length) this.first = now;
    this.latest = now;
    for (const f of list) {
      // the same agent twice in one burst: the newer finish says it
      this.items = this.items.filter((x) => x.paneId !== f.paneId);
      this.items.push(f);
    }
  }

  /** The burst, once it is over; null while it may still grow. */
  take(now: number): Finish[] | null {
    if (!this.items.length) return null;
    if (now - this.latest < GROUP_MS && now - this.first < GROUP_MAX_MS) return null;
    const out = this.items;
    this.items = [];
    return out;
  }

  clear(): void {
    this.items = [];
  }
}

/** "45 s", "4 min", "1 h 20 min". */
export function ranFor(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  return m % 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m / 60} h`;
}

function filesText(n: number | null): string {
  return n ? `${n} file${n === 1 ? "" : "s"} changed` : "changes to review";
}

/** One agent's finish in a few words, after its name. */
function what(f: Finish): string {
  if (f.kind === "review") return `ready to review, ${filesText(f.files)}`;
  if (f.kind === "crash") return "stopped with an error";
  return "done";
}

const WEIGHT: Record<FinishKind, number> = { crash: 0, review: 1, done: 2 };
/** Names a grouped notification lists before "and N more". */
const LIST_MAX = 3;

/** The notification for a burst, and which agent a click opens: the one that
 *  most wants you (an error, then changes to review), first to finish. */
export function finishNote(list: Finish[]): { title: string; body: string; open: string } {
  const order = [...list].sort((a, b) => WEIGHT[a.kind] - WEIGHT[b.kind] || a.at - b.at);
  const top = order[0];
  if (order.length === 1) {
    if (top.kind === "review") return { title: `${top.name} is ready to review`, body: `${filesText(top.files)} in ${top.project}, after ${ranFor(top.ranMs)}.`, open: top.paneId };
    if (top.kind === "crash") return { title: `${top.name} stopped with an error`, body: `It exited with code ${top.exitCode} after ${ranFor(top.ranMs)}, in ${top.project}.`, open: top.paneId };
    return { title: `${top.name} is done`, body: `Worked ${ranFor(top.ranMs)} in ${top.project}.`, open: top.paneId };
  }
  const lines = order.slice(0, LIST_MAX).map((f) => `${f.name}: ${what(f)}`);
  const more = order.length - LIST_MAX;
  if (more > 0) lines.push(`and ${more} more`);
  return { title: `${order.length} agents finished`, body: lines.join("\n"), open: top.paneId };
}

/** How the phone dashboard labels an agent that is through with its work. */
export function finishedLabel(state: TaskState, worked: boolean, limited: boolean): "done" | "review" | null {
  if (state === "review") return "review";
  if (state === "idle" && worked && !limited) return "done";
  return null;
}
