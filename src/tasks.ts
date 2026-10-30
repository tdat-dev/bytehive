// Live task list: one entry per agent pane, with its derived state.
//
// Runs next to the existing pane bookkeeping without changing it: every tick it
// reads each running pane's screen, asks git (throttled) whether the agent's
// branch has changes, and derives needs / blocked / review / working / idle / stopped.
// The new Queue UI renders from here; for now the old UI only borrows the
// status line for notifications.

import { workspaces } from "./appstate";
import { gitChangedFiles, gitCheckpoint, sendInput } from "./ipc";
import { freeTextKeys, riskOfAsk, type AskOption } from "./askparse";
import { deriveTaskState, statusLine, type TaskState, type TaskStatus } from "./taskstate";
import { clashHistory, clashNotes, type ClashInput } from "./clashwatch";
import { FEED_READ_MS, forgetFeed, itemsFor, refreshGlance } from "./gridcards";
import { forgetCost } from "./agentcost";
import { forgetConversation } from "./chatsource";
import { StuckWatch } from "./stuckwatch";
import type { Pane, Workspace } from "./panetypes";

export interface Task {
  paneId: string;
  wsId: string;
  name: string;
  /** Workspace name, which is the project the agent works in. */
  project: string;
  branch: string | null;
  /** What it was asked to do, when Maestro handed it the job. */
  title: string | null;
  /** One of several agents given the same job ("race 2/3"). */
  race: { id: string; n: number; of: number } | null;
  status: TaskStatus;
  changedFiles: number | null;
  /** Lines added and removed on its branch, when it has changes. */
  added: number | null;
  removed: number | null;
  /** The branch its work was merged into, once Review merged it. */
  mergedInto?: string | null;
  /** ms the current state began, for "waiting 4m" and queue order. */
  since: number;
  /** It has worked on something since Maestro started: idle then means done,
   *  not still waiting for its first task. */
  worked?: boolean;
}

/** Screen rows read per pane: enough for a boxed prompt with its context. */
const SCREEN_LINES = 40;
/** How often each agent's branch is asked for changed files. */
export const DIFF_EVERY_MS = 5000;

/** One line in an agent's History: what happened and when. */
export interface HistoryEvent {
  at: number;
  kind: TaskState | "answer" | "stuck" | "clash";
  text: string;
}
/** Enough to scroll back through a long session without growing forever. */
const HISTORY_MAX = 200;

const tasks = new Map<string, Task>();
const history = new Map<string, HistoryEvent[]>();
const diffs = new Map<string, { n: number | null; add: number | null; del: number | null; paths: string[]; at: number; pending: boolean; wasRunning: boolean }>();
const listeners = new Set<(list: Task[]) => void>();
/** Per-agent stuck trails: loop and stall evidence from its conversation. */
const stuckWatch = new StuckWatch();
/** Last transcript read per agent; the tick re-reads on the shared cadence. */
const feedAt = new Map<string, number>();

/** Agents whose branch Review merged, until they start again. */
const merged = new Map<string, { at: number; base: string; branch: string }>();

/** Review merged this agent's branch into `base` (its worktree is gone). */
export function markMerged(paneId: string, base: string, branch: string, at: number = Date.now()): void {
  merged.set(paneId, { at, base, branch });
  log(paneId, { at, kind: "merged", text: `Merged ${branch} into ${base}` });
}

/** Where this agent's work was merged, while it shows as merged. */
export function mergedInfo(paneId: string): { base: string; branch: string } | null {
  return merged.get(paneId) ?? null;
}

/** Changed files can only be pinned on one agent when it has its own worktree;
 *  agents sharing a checkout share one diff, so for them it stays unknown.
 *  A stopped agent's tree doesn't change on its own: it is looked at once
 *  (first sight, or once more right after it stops), not every tick. */
function changesFor(pane: Pane, now: number): { n: number | null; add: number | null; del: number | null; paths: string[] } {
  const wt = pane.spec.worktree;
  if (!wt) return { n: null, add: null, del: null, paths: [] };
  const d = diffs.get(pane.id) ?? { n: null, add: null, del: null, paths: [], at: 0, pending: false, wasRunning: false };
  const due = pane.running || d.at === 0 || d.wasRunning;
  if (due && !d.pending && now - d.at >= DIFF_EVERY_MS) {
    d.pending = true;
    d.wasRunning = pane.running;
    gitChangedFiles(wt)
      .then((files) => {
        d.n = files.length;
        d.paths = files.map((f) => f.path);
        let add = 0, del = 0;
        for (const f of files) { add += f.added ?? 0; del += f.removed ?? 0; }
        d.add = add; d.del = del;
      })
      .catch(() => { d.n = d.add = d.del = null; d.paths = []; })
      .finally(() => { d.at = Date.now(); d.pending = false; });
  }
  diffs.set(pane.id, d);
  return d;
}

function toTask(ws: Workspace, pane: Pane, now: number): Task {
  // Started again after the merge: it is on something new.
  const m = merged.get(pane.id);
  if (m && pane.spawnedAt != null && pane.spawnedAt > m.at) merged.delete(pane.id);
  const done = merged.get(pane.id);
  const changes = changesFor(pane, now);
  const changedFiles = changes.n;
  const prev = tasks.get(pane.id);
  const hadWorked = !!prev?.worked || (history.get(pane.id) ?? []).some((e) => e.kind === "working");
  const status = deriveTaskState(
    {
      running: pane.running,
      lastOutputAt: pane.lastOutputAt,
      screen: pane.running ? pane.term.snapshot(SCREEN_LINES) : "",
      changedFiles,
      merged: !!done,
      worked: hadWorked,
    },
    now,
  );
  status.stuck = stuckWatch.step(pane.id, {
    run: pane.spawnedAt ?? 0,
    state: status.state,
    items: itemsFor(pane.id),
    diff: changedFiles === null ? null : { n: changedFiles, add: changes.add ?? 0, del: changes.del ?? 0 },
    now,
  });
  // "resets 7pm" read after 7pm would be tomorrow's: the same notice keeps the
  // time it was first read with
  const was = prev?.status.limit;
  if (status.limit && was && was.what === status.limit.what) status.limit = { ...status.limit, resetsAt: was.resetsAt ?? status.limit.resetsAt };
  return {
    paneId: pane.id,
    wsId: ws.id,
    name: pane.spec.name,
    project: ws.name,
    branch: pane.spec.branch ?? null,
    title: pane.spec.title ?? null,
    race: pane.spec.race ?? null,
    status,
    changedFiles,
    added: changes.add,
    removed: changes.del,
    mergedInto: done?.base ?? null,
    since: prev && prev.status.state === status.state ? prev.since : now,
    worked: hadWorked || status.state === "working",
  };
}

/** The History line for a task entering its current state, or null for
 *  states not worth a line of their own. */
export function describe(t: Pick<Task, "status" | "changedFiles">): string | null {
  const a = t.status.ask;
  switch (t.status.state) {
    case "needs":
      if (a?.kind === "run") return a.detail ? `Asked to run ${a.detail}` : "Asked to run a command";
      if (a?.kind === "edit") return a.detail ? `Asked to edit ${a.detail}` : "Asked to edit a file";
      return a ? `Asked: ${a.prompt}` : "Waited for you";
    case "blocked": {
      const b = t.status.blocked?.kind;
      if (b === "captcha") return "Blocked on a CAPTCHA";
      if (b === "key") return "Blocked on an API key";
      return "Blocked on sign-in";
    }
    case "idle":
      return t.status.limit ? "Hit its usage limit" : null;
    case "review": {
      const n = t.changedFiles ?? 0;
      return `Finished with ${n} file${n === 1 ? "" : "s"} changed`;
    }
    case "working": return "Started working";
    case "stopped": return "Stopped";
    case "merged": return null; // markMerged wrote the line, with where it went
    default: return null;
  }
}

function log(paneId: string, ev: HistoryEvent): void {
  const list = history.get(paneId) ?? [];
  const last = list[list.length - 1];
  // A burst of output flips working on and off; one "Started working" per stretch is enough.
  if (last && last.text === ev.text) return;
  if (ev.kind === "working" && last?.kind === "working") return;
  list.push(ev);
  if (list.length > HISTORY_MAX) list.splice(0, list.length - HISTORY_MAX);
  history.set(paneId, list);
}

/** What an agent has done, oldest first. */
export function historyOf(paneId: string): HistoryEvent[] {
  return history.get(paneId) ?? [];
}

/** Re-derive every task. Call once per tick; cheap when nothing runs. */
export function updateTasks(now: number = Date.now()): void {
  let changed = false;
  const seen = new Set<string>();
  const prevClash = new Map<string, string | null>();
  for (const [id, t] of tasks) prevClash.set(id, t.status.clash?.key ?? null);
  for (const ws of workspaces.values())
    for (const pane of ws.panes.values()) {
      seen.add(pane.id);
      const next = toTask(ws, pane, now);
      const prev = tasks.get(pane.id);
      const moved = !prev || prev.status.state !== next.status.state || prev.status.ask?.prompt !== next.status.ask?.prompt ||
        prev.status.ask?.detail !== next.status.ask?.detail || !!prev.status.limit !== !!next.status.limit || (prev.status.stuck?.text ?? null) !== (next.status.stuck?.text ?? null);
      if (moved) {
        changed = true;
        const text = describe(next);
        if (text) log(pane.id, { at: now, kind: next.status.state, text });
        if (next.status.stuck && !prev?.status.stuck) log(pane.id, { at: now, kind: "stuck", text: `Looks stuck: ${next.status.stuck.text}` });
      }
      tasks.set(pane.id, next);
    }
  // Early clash warnings need every agent's files: a second pass once all
  // states are in.
  const clashInputs: ClashInput[] = [];
  for (const ws of workspaces.values())
    for (const pane of ws.panes.values()) {
      const t = tasks.get(pane.id);
      if (!t) continue;
      clashInputs.push({
        paneId: pane.id, name: t.name, wsId: ws.id, state: t.status.state, running: pane.running,
        title: t.title, race: pane.spec.race?.id ?? null, paths: diffs.get(pane.id)?.paths ?? [],
      });
    }
  const clashMap = clashNotes(clashInputs);
  for (const [id, t] of tasks) {
    const note = clashMap.get(id) ?? null;
    t.status.clash = note;
    if ((note?.key ?? null) !== (prevClash.get(id) ?? null)) {
      changed = true;
      if (note && !prevClash.get(id)) log(id, { at: now, kind: "clash", text: clashHistory(note) });
    }
  }
  // Transcripts feed detection, not just cards: re-read running agents'
  // conversations on the shared cadence; the rail reads its own as well.
  for (const ws of workspaces.values())
    for (const pane of ws.panes.values()) {
      if (!pane.running || !pane.spawnedAt) continue;
      if (now - (feedAt.get(pane.id) ?? 0) < FEED_READ_MS) continue;
      feedAt.set(pane.id, now);
      void refreshGlance(pane);
    }
  for (const id of [...tasks.keys()])
    if (!seen.has(id)) { tasks.delete(id); diffs.delete(id); history.delete(id); merged.delete(id); feedAt.delete(id); forgetFeed(id); forgetCost(id); forgetConversation(id); stuckWatch.forget(id); changed = true; }
  if (changed && listeners.size) {
    const list = allTasks();
    for (const cb of listeners) cb(list);
  }
}

const RANK: Record<TaskState, number> = { needs: 0, blocked: 1, review: 2, working: 3, idle: 4, stopped: 5, merged: 6 };

/** Every task, the queue's order: what needs you first, longest waiting first. */
export function allTasks(): Task[] {
  return [...tasks.values()].sort((a, b) => RANK[a.status.state] - RANK[b.status.state] || a.since - b.since);
}

export function taskOf(paneId: string): Task | undefined {
  return tasks.get(paneId);
}

/** Plain-words line for a pane ("Ana wants to run npm test"), or null if unknown. */
export function taskLine(paneId: string): string | null {
  const t = tasks.get(paneId);
  return t ? statusLine(t.name, t.status) : null;
}

/** Be told when any task changes state or starts asking something new. */
export function onTasksChange(cb: (list: Task[]) => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Snapshot the worktree before a high-risk approval lands, so a disaster
 *  restores from `git stash list`. Best-effort: a failure never blocks you. */
async function checkpointFor(paneId: string): Promise<string> {
  const t = tasks.get(paneId);
  const ask = t?.status.ask;
  if (!ask || riskOfAsk(ask) !== "high") return "";
  const dir = t ? workspaces.get(t.wsId)?.panes.get(paneId)?.spec.worktree ?? workspaces.get(t.wsId)?.panes.get(paneId)?.spec.cwd ?? null : null;
  if (!dir) return "";
  try {
    const sha = await gitCheckpoint(dir, `maestro checkpoint before approving: ${(ask.detail ?? ask.prompt).slice(0, 80)}`);
    return sha ? ` (checkpoint ${sha.slice(0, 7)})` : "";
  } catch {
    return "";
  }
}

/** Pick an option of the prompt currently on the agent's screen. */
export async function answerOption(paneId: string, option: AskOption): Promise<void> {
  const note = await checkpointFor(paneId);
  log(paneId, { at: Date.now(), kind: "answer", text: `You picked “${option.label}”${note}` });
  await sendInput(paneId, option.key);
}

/** Answer in free text, through the CLI's "type something" row when it has one. */
export async function answerText(paneId: string, text: string): Promise<void> {
  const ask = tasks.get(paneId)?.status.ask ?? null;
  const note = await checkpointFor(paneId);
  log(paneId, { at: Date.now(), kind: "answer", text: `You said “${text.trim()}”${note}` });
  for (const part of freeTextKeys(ask, text)) await sendInput(paneId, part);
}
