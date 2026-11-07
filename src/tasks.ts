// Live task list: one entry per agent pane, with its derived state.
//
// Runs next to the existing pane bookkeeping without changing it: every tick it
// reads each running pane's screen, asks git (throttled) what the agent changed,
// and derives needs / review / working / idle / stopped. An agent with a worktree
// has its branch to go by; the rest share a folder (folderwork.ts).
// The new Queue UI renders from here; for now the old UI only borrows the
// status line for notifications.

import { workspaces } from "./appstate";
import { agentChanges, folderState, sendInput } from "./ipc";
import { createFolderWork, type FolderWork } from "./folderwork";
import { freeTextKeys, type AskOption } from "./askparse";
import { deriveTaskState, statusLine, WORKING_MS, type TaskState, type TaskStatus } from "./taskstate";
import type { Pane, Workspace } from "./panetypes";

export interface Task {
  paneId: string;
  wsId: string;
  name: string;
  /** Workspace name, which is the project the agent works in. */
  project: string;
  branch: string | null;
  /** What it was asked to do, when ByteHive handed it the job. */
  title: string | null;
  /** One of several agents given the same job ("race 2/3"). */
  race: { id: string; n: number; of: number } | null;
  status: TaskStatus;
  changedFiles: number | null;
  /** Lines added and removed on its branch, when it has changes. */
  added: number | null;
  removed: number | null;
  /** Of its changed files, how many another agent in its folder may have changed instead. */
  shared?: number;
  /** The branch its work was merged into, once Review merged it. */
  mergedInto?: string | null;
  /** ms the current state began, for "waiting 4m" and queue order. */
  since: number;
  /** It has worked on something since ByteHive started: idle then means done,
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
  kind: TaskState | "answer";
  text: string;
}
/** Enough to scroll back through a long session without growing forever. */
const HISTORY_MAX = 200;

const tasks = new Map<string, Task>();
const history = new Map<string, HistoryEvent[]>();
const diffs = new Map<string, { n: number | null; add: number | null; del: number | null; at: number; pending: boolean; wasRunning: boolean }>();
const listeners = new Set<(list: Task[]) => void>();

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

/** Folders agents without a worktree work in, by the project's folder. */
const folders = new Map<string, { work: FolderWork; at: number; pending: boolean; wasRunning: boolean }>();

/** The folder this agent shares, when it has no worktree. A plain shell is you
 *  at a prompt, so it neither claims nor counts. */
function sharedFolder(ws: Workspace, pane: Pane): string | null {
  if (pane.spec.worktree || pane.spec.badge === "shell" || pane.spec.badge === "cmd") return null;
  return ws.dir ?? pane.spec.cwd ?? null;
}

/** Look at each shared folder (throttled) and credit what moved since the last
 *  look to the agents that printed in between. */
function lookAtFolders(now: number): void {
  const live = new Map<string, Pane[]>();
  for (const ws of workspaces.values())
    for (const p of ws.panes.values()) {
      const dir = sharedFolder(ws, p);
      if (dir) live.set(dir, [...(live.get(dir) ?? []), p]);
    }
  for (const dir of folders.keys()) if (!live.has(dir)) folders.delete(dir);
  for (const [dir, panes] of live) {
    const f = folders.get(dir) ?? { work: createFolderWork(), at: 0, pending: false, wasRunning: false };
    folders.set(dir, f);
    const running = panes.some((p) => p.running);
    if (!(running || f.wasRunning) || f.pending || (f.at !== 0 && now - f.at < DIFF_EVERY_MS)) continue;
    const since = f.at;
    f.pending = true;
    f.at = now;
    f.wasRunning = running;
    folderState(dir, f.work.head())
      .then((state) => f.work.see(state, panes.filter((p) => p.lastOutputAt >= since - WORKING_MS).map((p) => p.id)))
      .catch(() => {}) // not a repo: nothing can be said, and changes stay unknown
      .finally(() => { f.pending = false; });
  }
}

/** Where an agent's changes are read from. */
export interface ChangeScope {
  dir: string;
  from: string | null;
  /** Only these files; null for everything in its own worktree. */
  paths: string[] | null;
  shared: number;
}

function scopeOf(ws: Workspace, pane: Pane): ChangeScope | null {
  const wt = pane.spec.worktree;
  if (wt) return { dir: wt, from: null, paths: null, shared: 0 };
  const dir = sharedFolder(ws, pane);
  const f = dir ? folders.get(dir) : undefined;
  return dir && f?.work.seen() ? { dir, ...f.work.of(pane.id) } : null;
}

/** What Review reads, the same as the row and the state; null while unknown. */
export function changeScope(pane: Pane): ChangeScope | null {
  for (const ws of workspaces.values()) if (ws.panes.get(pane.id) === pane) return scopeOf(ws, pane);
  return null;
}

/** You have read this agent's changes in a shared folder: it is done with them,
 *  and no merge is coming to say so. */
export function markReviewed(paneId: string, at: number = Date.now()): void {
  for (const f of folders.values()) f.work.settle(paneId);
  diffs.delete(paneId);
  log(paneId, { at, kind: "idle", text: "You reviewed its changes" });
}

/** What the agent changed, by `agentChanges`, the same call for a worktree and a
 *  shared folder; unknown until there is something to compare.
 *  A stopped agent's tree doesn't change on its own: it is looked at once
 *  (first sight, or once more right after it stops), not every tick. */
function changesFor(ws: Workspace, pane: Pane, now: number): { n: number | null; add: number | null; del: number | null; shared: number } {
  const scope = scopeOf(ws, pane);
  if (!scope) return { n: null, add: null, del: null, shared: 0 };
  const d = diffs.get(pane.id) ?? { n: null, add: null, del: null, at: 0, pending: false, wasRunning: false };
  const due = pane.running || d.at === 0 || d.wasRunning;
  if (due && !d.pending && now - d.at >= DIFF_EVERY_MS) {
    d.pending = true;
    d.wasRunning = pane.running;
    agentChanges(scope.dir, scope.from, scope.paths)
      .then((files) => {
        d.n = files.length;
        let add = 0, del = 0;
        for (const f of files) { add += f.added ?? 0; del += f.removed ?? 0; }
        d.add = add; d.del = del;
      })
      .catch(() => { d.n = d.add = d.del = null; })
      .finally(() => { d.at = Date.now(); d.pending = false; });
  }
  diffs.set(pane.id, d);
  return { ...d, shared: scope.shared };
}

function toTask(ws: Workspace, pane: Pane, now: number): Task {
  // Started again after the merge: it is on something new.
  const m = merged.get(pane.id);
  if (m && pane.spawnedAt != null && pane.spawnedAt > m.at) merged.delete(pane.id);
  const done = merged.get(pane.id);
  const changes = changesFor(ws, pane, now);
  const changedFiles = changes.n;
  const status = deriveTaskState(
    {
      running: pane.running,
      lastOutputAt: pane.lastOutputAt,
      screen: pane.running ? pane.term.snapshot(SCREEN_LINES) : "",
      changedFiles,
      merged: !!done,
    },
    now,
  );
  const prev = tasks.get(pane.id);
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
    shared: changes.shared,
    mergedInto: done?.base ?? null,
    since: prev && prev.status.state === status.state ? prev.since : now,
    worked: !!prev?.worked || status.state === "working" || (history.get(pane.id) ?? []).some((e) => e.kind === "working"),
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
  lookAtFolders(now);
  for (const ws of workspaces.values())
    for (const pane of ws.panes.values()) {
      seen.add(pane.id);
      const next = toTask(ws, pane, now);
      const prev = tasks.get(pane.id);
      const moved = !prev || prev.status.state !== next.status.state || prev.status.ask?.prompt !== next.status.ask?.prompt ||
        prev.status.ask?.detail !== next.status.ask?.detail || !!prev.status.limit !== !!next.status.limit;
      if (moved) {
        changed = true;
        const text = describe(next);
        if (text) log(pane.id, { at: now, kind: next.status.state, text });
      }
      tasks.set(pane.id, next);
    }
  for (const id of [...tasks.keys()])
    if (!seen.has(id)) {
      tasks.delete(id); diffs.delete(id); history.delete(id); merged.delete(id); changed = true;
      for (const f of folders.values()) f.work.settle(id);
    }
  if (changed && listeners.size) {
    const list = allTasks();
    for (const cb of listeners) cb(list);
  }
}

const RANK: Record<TaskState, number> = { needs: 0, review: 1, working: 2, idle: 3, stopped: 4, merged: 5 };

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

/** Pick an option of the prompt currently on the agent's screen. */
export async function answerOption(paneId: string, option: AskOption): Promise<void> {
  log(paneId, { at: Date.now(), kind: "answer", text: `You picked “${option.label}”` });
  await sendInput(paneId, option.key);
}

/** Answer in free text, through the CLI's "type something" row when it has one. */
export async function answerText(paneId: string, text: string): Promise<void> {
  const ask = tasks.get(paneId)?.status.ask ?? null;
  log(paneId, { at: Date.now(), kind: "answer", text: `You said “${text.trim()}”` });
  for (const part of freeTextKeys(ask, text)) await sendInput(paneId, part);
}
