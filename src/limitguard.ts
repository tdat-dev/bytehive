// Limit Guard: no agent left sitting at its plan's usage limit.
//   - An agent that stops at its limit shows it ("Out of usage · back 7:09 PM",
//     taskstate/limits.ts), and you get a notification.
//   - When the limit lifts, Maestro tells it to carry on (Settings: Carry on
//     when a usage limit resets). The time comes from its own screen, else
//     from the plan's numbers (planmeter).
//   - Or the task goes on at once with another CLI: a new agent in the same
//     folder, briefed from the first one's conversation (handoff.ts).
//   - Before any of that: a notification when a plan passes 90% of a limit.

import { allTasks, onTasksChange, type Task } from "./tasks";
import { notify, sendMessage } from "./ipc";
import { getPref } from "./prefs";
import { clockOf } from "./taskstate";
import { limitName, limitResetsAt, planResetFor, planSnapshot } from "./planmeter";
import { workspaces } from "./appstate";
import { handoffBrief } from "./handoff";
import { sourceOf } from "./chatsource";
import { CLI_PRESETS } from "./crew";
import { loadCrew, presetAvailable, spawnAgents } from "./spawnmodal";
import type { Chat } from "./chatmodel";
import type { Pane, Workspace } from "./panetypes";

/** A little after the reset, so the plan has surely turned over. */
const GRACE_MS = 90_000;
/** Never tell the same agent to carry on twice within this. */
const RESUME_GAP_MS = 20 * 60_000;
const CHECK_MS = 30_000;
/** What a plan's warning is at. */
const WARN_AT = 90;

export const CARRY_ON = "Your usage limit has reset. Carry on with the task where you left off.";

/** Whose plan an agent's CLI runs on, in planmeter's words. */
export function planOf(badge: string): string {
  return badge === "claude" ? "Claude" : badge === "codex" ? "Codex" : "";
}

interface Episode { what: string; reset: number | null; notified: boolean }
const episodes = new Map<string, Episode>();
const resumedAt = new Map<string, number>();
const warned = new Set<string>();
let timer: number | null = null;
let off: (() => void) | null = null;

function find(paneId: string): { ws: Workspace; pane: Pane } | null {
  for (const ws of workspaces.values()) {
    const pane = ws.panes.get(paneId);
    if (pane) return { ws, pane };
  }
  return null;
}

/** When the limit this agent ran into lifts: what its screen says, else its plan. */
export function limitLiftsAt(t: Task, badge: string, now = Date.now()): number | null {
  return t.status.limit?.resetsAt ?? planResetFor(planOf(badge), now);
}

function checkAgents(now: number): void {
  for (const t of allTasks()) {
    const lim = t.status.limit;
    const found = find(t.paneId);
    if (!lim || !found) { episodes.delete(t.paneId); continue; }
    // one notice is one episode, and keeps the reset first seen for it: the
    // plan only knows a reset until it has passed
    let e = episodes.get(t.paneId);
    if (!e || e.what !== lim.what) {
      e = { what: lim.what, reset: limitLiftsAt(t, found.pane.spec.badge, now), notified: false };
      episodes.set(t.paneId, e);
    } else if (e.reset === null) e.reset = limitLiftsAt(t, found.pane.spec.badge, now);
    const reset = e.reset;
    const carryOn = getPref("autoResume");
    if (!e.notified) {
      e.notified = true;
      if (getPref("limitNotify")) {
        void notify(`${t.name} is out of usage`, reset
          ? `Its limit resets at ${clockOf(reset, now)}.${carryOn ? " Maestro will tell it to carry on then." : ""}`
          : "Its plan's usage limit ran out. You can hand its task to another CLI from its menu.");
      }
    }
    if (!carryOn || !reset || now < reset + GRACE_MS) continue;
    if (!found.pane.running || t.status.state === "working") continue;
    if (now - (resumedAt.get(t.paneId) ?? 0) < RESUME_GAP_MS) continue;
    resumedAt.set(t.paneId, now);
    void sendMessage(t.paneId, CARRY_ON);
    if (getPref("limitNotify")) void notify(`${t.name} is back at work`, "Its usage limit reset, so Maestro told it to carry on.");
  }
}

function checkPlans(now: number): void {
  if (!getPref("limitNotify")) return;
  for (const u of planSnapshot(now)) {
    for (const l of u.limits) {
      const at = limitResetsAt(l);
      if (l.percent < WARN_AT || (at !== null && at <= now)) continue;
      const key = `${u.provider}|${l.kind}|${l.model ?? ""}|${at ?? ""}`;
      if (warned.has(key)) continue;
      warned.add(key);
      void notify(`${u.provider ?? "Claude"} ${limitName(l.kind, l.model)}: ${Math.round(l.percent)}% used`,
        `${at ? `It resets at ${clockOf(at, now)}. ` : ""}Agents stop when it runs out; another CLI can take over their tasks.`);
    }
  }
}

/** Look now (and every CHECK_MS, and whenever an agent changes). */
export function checkLimits(now = Date.now()): void {
  checkAgents(now);
  checkPlans(now);
}

export function startLimitGuard(): void {
  if (timer !== null) return;
  timer = window.setInterval(() => checkLimits(), CHECK_MS);
  off = onTasksChange(() => checkLimits());
}

export function stopLimitGuard(): void {
  if (timer !== null) window.clearInterval(timer);
  timer = null;
  off?.();
  off = null;
  episodes.clear();
  resumedAt.clear();
  warned.clear();
}

/** CLIs that could take over this agent's task: installed, not a shell, not
 *  its own, the ones whose plan has room first. */
export function handoffTargets(pane: Pane, now = Date.now()): Array<{ id: string; label: string }> {
  const full = (badge: string) => planResetFor(planOf(badge), now) !== null;
  return CLI_PRESETS
    .filter((p) => !p.shell && p.badge !== pane.spec.badge && p.id !== "custom" && presetAvailable(p.program))
    .sort((a, b) => Number(full(a.badge)) - Number(full(b.badge)))
    .map((p) => ({ id: p.id, label: p.label }));
}

/** The agent's conversation for a brief, read from its CLI's files (the
 *  start, for the first asks, and the end). */
async function conversationOf(pane: Pane): Promise<{ chat: Chat | null; firstAsks: string[] }> {
  const src = sourceOf(pane);
  const dir = pane.spec.ranIn;
  if (!src || !dir) return { chat: null, firstAsks: [] };
  const firstAsks: string[] = [];
  // its first asks, from the start
  try {
    const head = src.createChat();
    const r = await src.read(pane, dir, 0);
    head.feed(r.text);
    for (const i of head.items) if (i.kind === "user" && i.text && firstAsks.length < 2) firstAsks.push(i.text);
  } catch { /* go on without */ }
  const chat = src.createChat();
  let offset = 0;
  try {
    for (let k = 0; k < 40; k++) {
      const r = await src.read(pane, dir, offset, k === 0 ? 4 * 1024 * 1024 : undefined);
      if (!r.text) break;
      chat.feed(r.text);
      offset = r.next;
    }
  } catch { /* what it has */ }
  return { chat: chat.items.length ? chat : null, firstAsks };
}

/** Start an agent of `presetId` in the same folder on this agent's task,
 *  briefed from its conversation. Returns the new agent's name. */
export async function handOff(paneId: string, presetId: string, why = "it hit its plan's usage limit"): Promise<string | null> {
  const found = find(paneId);
  const preset = CLI_PRESETS.find((p) => p.id === presetId);
  if (!found || !preset) return null;
  const { ws, pane } = found;
  const { chat, firstAsks } = await conversationOf(pane);
  const cli = CLI_PRESETS.find((p) => p.badge === pane.spec.badge)?.label ?? pane.spec.badge;
  const brief = handoffBrief({ name: pane.spec.name, cli, why, title: pane.spec.title ?? null, firstAsks }, chat);
  const [name] = await spawnAgents(ws, preset.id, 1, brief, loadCrew().skipPerms);
  // its row says whose task it carries on, not the brief's first words
  const next = [...ws.panes.values()].find((p) => p.spec.name === name);
  if (next) next.spec.title = `Carries on ${pane.spec.name}'s task${pane.spec.title ? `: ${pane.spec.title}` : ""}`;
  return name ?? null;
}
