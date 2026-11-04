// Limit Guard: no agent left sitting at its plan's usage limit.
//   - An agent that stops at its limit shows it ("Out of usage · back 7:09 PM",
//     taskstate/limits.ts), and you get a notification.
//   - When the limit lifts, ByteHive tells it to carry on (Settings: Carry on
//     when a usage limit resets). The time comes from its own screen, else
//     from the plan's numbers (planmeter).
//   - Or the task goes on at once with another CLI: an agent already open in
//     the same folder, told to read the first one's context (bytehive-mcp
//     agent_context), or a new agent there, briefed from its conversation
//     (handoff.ts).
//     With "Hand over when an agent runs out of usage" on (the default) that
//     happens by itself, the moment it runs out.
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

interface Episode {
  what: string;
  reset: number | null;
  notified: boolean;
  /** Its task was handed over in this episode (or is being): no carry-on. */
  handed: boolean;
  /** A handover was tried: never twice in one episode. */
  tried: boolean;
}
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
      e = { what: lim.what, reset: limitLiftsAt(t, found.pane.spec.badge, now), notified: false, handed: false, tried: false };
      episodes.set(t.paneId, e);
    } else if (e.reset === null) e.reset = limitLiftsAt(t, found.pane.spec.badge, now);
    const reset = e.reset;
    const carryOn = getPref("autoResume");
    if (!e.tried && !found.pane.handedTo && getPref("autoHandover")) {
      e.tried = true;
      const to = takeoverFor(found.pane, now);
      if (to) {
        e.handed = true;
        e.notified = true;
        void notify(`${t.name} ran out of usage`, `${to.label} takes its task over.`);
        void to.run().then((name) => { if (!name) e!.handed = false; });
      }
    }
    if (e.handed) continue;
    if (!e.notified) {
      e.notified = true;
      if (getPref("limitNotify")) {
        void notify(`${t.name} is out of usage`, reset
          ? `Its limit resets at ${clockOf(reset, now)}.${carryOn ? " ByteHive will tell it to carry on then." : ""}`
          : "Its plan's usage limit ran out. You can hand its task to another CLI from its menu.");
      }
    }
    // another agent has its task: telling it to carry on too would make two
    if (found.pane.handedTo) continue;
    if (!carryOn || !reset || now < reset + GRACE_MS) continue;
    if (!found.pane.running || t.status.state === "working") continue;
    if (now - (resumedAt.get(t.paneId) ?? 0) < RESUME_GAP_MS) continue;
    resumedAt.set(t.paneId, now);
    void sendMessage(t.paneId, CARRY_ON);
    if (getPref("limitNotify")) void notify(`${t.name} is back at work`, "Its usage limit reset, so ByteHive told it to carry on.");
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

/** Whether a CLI's plan is out until a reset. */
const full = (badge: string, now: number) => planResetFor(planOf(badge), now) !== null;
const presetOf = (badge: string) => CLI_PRESETS.find((p) => p.badge === badge);
/** "Claude Code" for "claude". */
export const cliLabel = (badge: string) => presetOf(badge)?.label ?? badge;
/** "D:\App\" and "d:/app" are the same folder. */
const sameFolder = (a: string, b: string) => {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  return norm(a) === norm(b);
};

export interface OpenTarget { paneId: string; name: string; cli: string; badge: string }

/** Agents already open that could take over this agent's task: running, of
 *  another CLI, not a shell or the Director, free (idle, or done with its
 *  changes waiting), and in the same folder, so they see its changes. The
 *  ones whose plan has room first. */
export function openTargets(pane: Pane, now = Date.now()): OpenTarget[] {
  const found = find(pane.id);
  const dir = pane.spec.ranIn;
  if (!found || !dir) return [];
  const state = new Map(allTasks().map((t) => [t.paneId, t.status.state]));
  return [...found.ws.panes.values()]
    .filter((p) => p.id !== pane.id && p.running && p.spec.badge !== pane.spec.badge && !presetOf(p.spec.badge)?.shell
      && p.spec.role !== "conductor" && !!p.spec.ranIn && sameFolder(p.spec.ranIn, dir)
      && (state.get(p.id) === "idle" || state.get(p.id) === "review"))
    .sort((a, b) => Number(full(a.spec.badge, now)) - Number(full(b.spec.badge, now)))
    .map((p) => ({ paneId: p.id, name: p.spec.name, cli: cliLabel(p.spec.badge), badge: p.spec.badge }));
}

/** What an open agent is told when it takes a task over: one line, the rest
 *  it reads itself with the bytehive MCP. */
export function takeOverLine(from: { name: string; cli: string }, why: string): string {
  return `Take over the unfinished task of ${from.name} (${from.cli}), which stopped because ${why}. Call the bytehive agent_context tool with who set to ${from.name} to read its task, plan, changed files and last words, check git status, then finish what is left. Say in one line what you picked up first.`;
}

/** Its row says who has its task now. */
function markHanded(pane: Pane, to: string): void {
  pane.handedTo = to;
  const title = (pane.spec.title ?? "").replace(/^Handed to [^:]+: /, "");
  pane.spec.title = `Handed to ${to}${title ? `: ${title}` : ""}`;
}

/** An open agent takes this agent's task over. Returns its name. */
export async function handOverToAgent(fromId: string, toId: string, why: string): Promise<string | null> {
  const from = find(fromId);
  const to = find(toId);
  if (!from || !to || !to.pane.running) return null;
  await sendMessage(toId, takeOverLine({ name: from.pane.spec.name, cli: cliLabel(from.pane.spec.badge) }, why));
  markHanded(from.pane, to.pane.spec.name);
  return to.pane.spec.name;
}

const LIMIT_WHY = "it hit its plan's usage limit";

/** Who takes an agent's task when it runs out of usage: an open agent in its
 *  folder whose plan has room, else a new agent of an installed CLI whose plan
 *  has room; null when every plan is out too. */
export function takeoverFor(pane: Pane, now = Date.now()): { label: string; run: () => Promise<string | null> } | null {
  const open = openTargets(pane, now).find((o) => !full(o.badge, now));
  if (open) return { label: `${open.name} (${open.cli})`, run: () => handOverToAgent(pane.id, open.paneId, LIMIT_WHY) };
  const fresh = handoffTargets(pane, now).find((c) => !full(CLI_PRESETS.find((p) => p.id === c.id)?.badge ?? "", now));
  if (fresh) return { label: `A new ${fresh.label} agent`, run: () => handOff(pane.id, fresh.id, LIMIT_WHY) };
  return null;
}

/** CLIs that could take over this agent's task: installed, not a shell, not
 *  its own, the ones whose plan has room first. */
export function handoffTargets(pane: Pane, now = Date.now()): Array<{ id: string; label: string }> {
  return CLI_PRESETS
    .filter((p) => !p.shell && p.badge !== pane.spec.badge && p.id !== "custom" && presetAvailable(p.program))
    .sort((a, b) => Number(full(a.badge, now)) - Number(full(b.badge, now)))
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
  if (name) markHanded(pane, name);
  return name ?? null;
}
