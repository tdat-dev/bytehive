// What each agent's task is doing right now, derived from facts Maestro already
// has instead of set by hand: is the process alive, when did it last print,
// what is on its screen, and has it changed files on its branch.
//
// This replaces the single "went quiet ⇒ needs you" guess. A quiet agent can be
// asking to run a command, asking a question, finished with changes to review,
// or simply waiting for work; only the first two need you, and the queue sorts
// on the difference.

import { parseAsk, type Ask } from "./askparse";
import { limitOnScreen, type LimitHit } from "./limits";

export type TaskState = "needs" | "review" | "working" | "idle" | "stopped" | "merged";

export interface TaskFacts {
  running: boolean;
  /** ms of the last PTY output. */
  lastOutputAt: number;
  /** Visible screen text (TerminalHandle.snapshot). */
  screen: string;
  /** Files changed on the agent's branch vs HEAD; null when unknown (no repo, not checked yet). */
  changedFiles: number | null;
  /** Its branch was merged into the base (and its worktree removed) from Review. */
  merged?: boolean;
}

export interface TaskStatus {
  state: TaskState;
  /** Present when state is "needs": what it is asking and which keys answer it. */
  ask: Ask | null;
  /** It stopped at its plan's usage limit (and when that lifts). */
  limit?: LimitHit | null;
}

/** Spinners and streaming redraw several times a second; this long without a
 *  byte means the agent has stopped producing and is waiting. */
export const WORKING_MS = 2500;

/** Order the queue uses: what needs you first, then what's ready for you. */
export const STATE_RANK: Record<TaskState, number> = { needs: 0, review: 1, working: 2, idle: 3, stopped: 4, merged: 5 };

export function deriveTaskState(f: TaskFacts, now: number): TaskStatus {
  // A merged agent was stopped to land its branch: its job is done, not cut short.
  if (!f.running) return { state: f.merged ? "merged" : "stopped", ask: null };
  // A prompt on screen wins over recency: selectors redraw while you move the
  // cursor, and a question is still a question the moment it appears.
  const ask = parseAsk(f.screen);
  if (ask) return { state: "needs", ask };
  if (now - f.lastOutputAt <= WORKING_MS) return { state: "working", ask: null };
  // quiet after hitting its plan's limit: not done, waiting for the limit to lift
  const limit = limitOnScreen(f.screen, now);
  if (f.changedFiles !== null && f.changedFiles > 0) return { state: "review", ask: null, limit };
  return { state: "idle", ask: null, limit };
}

/** "7:09 PM" today, "Thu 1:00 AM" later. */
export function clockOf(at: number, now = Date.now()): string {
  const d = new Date(at);
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return d.toDateString() === new Date(now).toDateString() ? time : `${d.toLocaleDateString([], { weekday: "short" })} ${time}`;
}

/** One line for a queue row or a notification, from the agent's point of view. */
export function statusLine(name: string, s: TaskStatus): string {
  const a = s.ask;
  if (s.state === "needs" && a) {
    if (a.kind === "run") return a.detail ? `${name} wants to run ${a.detail}` : `${name} wants to run a command`;
    if (a.kind === "edit") return a.detail ? `${name} wants to edit ${a.detail}` : `${name} wants to edit a file`;
    return `${name} asks: ${a.prompt}`;
  }
  if (s.limit) return s.limit.resetsAt ? `${name} is out of usage until ${clockOf(s.limit.resetsAt)}` : `${name} is out of usage`;
  if (s.state === "review") return `${name} is done, changes ready to review`;
  if (s.state === "working") return `${name} is working`;
  if (s.state === "stopped") return `${name} stopped`;
  if (s.state === "merged") return `${name}'s work is merged`;
  return `${name} is waiting for a task`;
}
