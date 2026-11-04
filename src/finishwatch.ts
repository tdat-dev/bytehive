// Notifications when agents finish (finishnote.ts decides what counts): each
// second, look at every task, collect finished runs into a burst, and send the
// burst as one Windows notification that opens the agent it names. Settings:
// Inbox → Tell me when an agent finishes. The phone dashboard reads lastFinish.

import { workspaces } from "./appstate";
import { notifyAgent, onNotificationOpen, stopAskedAt } from "./ipc";
import { getPref } from "./prefs";
import { allTasks, type Task } from "./tasks";
import { Burst, FinishWatch, finishNote, worthTelling, type FinishKind, type FinishSnap } from "./finishnote";
import type { Pane } from "./panetypes";

const CHECK_MS = 1000;
/** A stop asked this recently explains the exit: it isn't a crash. */
const STOP_ASKED_MS = 15_000;

const watch = new FinishWatch();
const burst = new Burst();
let timer: number | null = null;
let offOpen: (() => void) | null = null;
let onScreen: () => string | null = () => null;

function paneOf(t: Task): Pane | undefined {
  return workspaces.get(t.wsId)?.panes.get(t.paneId);
}

function snapOf(t: Task, now: number): FinishSnap {
  const pane = paneOf(t);
  const asked = stopAskedAt(t.paneId);
  return {
    paneId: t.paneId,
    name: t.name,
    project: t.project,
    state: t.status.state,
    limited: !!t.status.limit,
    files: t.changedFiles,
    exitCode: pane?.exitCode ?? null,
    stopAsked: asked !== null && now - asked < STOP_ASKED_MS,
  };
}

/** Look now. The timer calls this every second. */
export function checkFinishes(now = Date.now()): void {
  const ended = watch.step(allTasks().map((t) => snapOf(t, now)), now);
  if (!getPref("notifyDone")) { burst.clear(); return; }
  const focused = !document.hidden && document.hasFocus();
  const on = onScreen();
  burst.add(ended.filter((f) => worthTelling(f, on, focused)), now);
  const due = burst.take(now);
  if (!due) return;
  const n = finishNote(due);
  void notifyAgent(n.title, n.body, n.open).catch(() => {});
}

/** An agent's latest finish, until it starts working again (the phone dashboard). */
export function lastFinish(paneId: string): { kind: FinishKind; at: number } | null {
  return watch.lastFinish(paneId);
}

/** Start watching. `screen` names the agent on screen; `open` shows an agent
 *  when its notification is clicked. */
export function startFinishWatch(hooks: { screen: () => string | null; open: (paneId: string) => void }): void {
  if (timer !== null) return;
  onScreen = hooks.screen;
  timer = window.setInterval(() => checkFinishes(), CHECK_MS);
  let gone = false;
  offOpen = () => { gone = true; };
  void onNotificationOpen(hooks.open)
    .then((off) => { if (gone) off(); else offOpen = off; })
    .catch(() => {});
}

export function stopFinishWatch(): void {
  if (timer !== null) window.clearInterval(timer);
  timer = null;
  offOpen?.();
  offOpen = null;
  onScreen = () => null;
  watch.clear();
  burst.clear();
}
