// Resume carries on the conversation an agent was last in.
//
// ByteHive starts a Claude agent on a session id it chose (--session-id) and
// resumes that id later. The conversation can move on inside the CLI though: a
// /clear starts a new one, a /resume switches to another. Resuming the id it
// was started with then brings back an old conversation. Claude Code writes the
// session each running process is in to ~/.claude/sessions/<pid>.json, so this
// follows it: it finds the agent's process (by the session it was started on,
// else by its folder and start time) and moves the agent's sessionId along,
// which is saved, so a restart of the app or of the computer resumes it too.

import { workspaces } from "./appstate";
import { claudeLiveSessions, type LiveSession } from "./ipc";
import { saveSession } from "./session";
import type { Pane } from "./panetypes";

const CHECK_MS = 8000;
/** A process started this close to the pane's spawn may be its CLI. */
export const START_SLACK_MS = 20_000;

/** The process each run of a pane's CLI is, once found ("paneId:run"). */
const pidOf = new Map<string, number>();
let timer: number | null = null;

const keyOf = (p: Pane) => `${p.id}:${p.run ?? 0}`;
const norm = (p: string) => p.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();

/** Move each running Claude agent's sessionId to the session its process is
 *  in now. Returns the agents that moved on. */
export function follow(panes: Pane[], live: LiveSession[]): Pane[] {
  const mine = panes.filter((p) => p.running && p.spec.badge === "claude" && p.spawnedAt);
  const keys = new Set(mine.map(keyOf));
  for (const k of [...pidOf.keys()]) if (!keys.has(k)) pidOf.delete(k);
  const found = new Map<Pane, LiveSession>();
  const claimed = new Set<number>();
  const take = (p: Pane, s: LiveSession) => { found.set(p, s); claimed.add(s.pid); pidOf.set(keyOf(p), s.pid); };
  // First the sure ones: a process seen before, or the session it was started on.
  for (const p of mine) {
    const pid = pidOf.get(keyOf(p));
    const s = pid !== undefined ? live.find((l) => l.pid === pid) : live.find((l) => l.session_id === p.spec.sessionId);
    if (s && !claimed.has(s.pid)) take(p, s);
  }
  // Then one started in its folder when it was, if only one fits.
  for (const p of mine) {
    if (found.has(p) || pidOf.has(keyOf(p)) || !p.spec.ranIn) continue;
    const near = live.filter((l) => !claimed.has(l.pid) && norm(l.cwd) === norm(p.spec.ranIn!) &&
      Math.abs(l.started_ms - p.spawnedAt!) <= START_SLACK_MS);
    if (near.length === 1) take(p, near[0]);
  }
  const moved: Pane[] = [];
  for (const [p, s] of found) {
    if (s.session_id === p.spec.sessionId) continue;
    p.spec.sessionId = s.session_id;
    moved.push(p);
  }
  return moved;
}

async function check(): Promise<void> {
  const panes = [...workspaces.values()].flatMap((ws) => [...ws.panes.values()]);
  if (!panes.some((p) => p.running && p.spec.badge === "claude")) { pidOf.clear(); return; }
  const live = await claudeLiveSessions().catch(() => null);
  if (live && follow(panes, live).length) saveSession();
}

export function startSessionFollow(): void {
  if (timer !== null) return;
  timer = window.setInterval(() => void check(), CHECK_MS);
}

export function stopSessionFollow(): void {
  if (timer !== null) window.clearInterval(timer);
  timer = null;
  pidOf.clear();
}
