// Starting agents: New agent (spawnAgents), the Director's agent_spawn over
// MCP (spawnForConductor), scheduled crews (spawnCrew + saved templates), a
// plain terminal, and which CLIs are installed. Workspace/pane creation is
// injected via configureSpawnModal to avoid a circular import with main.ts.

import { gitRepoRoot, programsOnPath, sendMessage } from "./ipc";
import { getPref } from "./prefs";
import { parseAsk } from "./askparse";
import { composerDrawn } from "./typing";
import { revealPane } from "./agentbridge";
import {
  CLI_PRESETS,
  taskTitle,
  expandCrew,
  runLimited,
  effectiveArgs,
  nameForNewPane,
  resolvePreset,
  type CrewState,
  type CliPreset,
} from "./crew";
import { type Pane, type Workspace, type AgentSpec } from "./panetypes";
import { workspaces, activeWs } from "./appstate";
import { basename } from "./workspaces";
import { DIRECTOR_LAWS } from "./laws";

let onCreateAgent: (
  ws: Workspace,
  spec: AgentSpec,
  restore?: boolean,
  attach?: { id: string; spawnedAt: number | null },
) => () => Promise<void> = () => async () => {};
let onCreateWorkspace: (dir: string | null, name?: string) => Workspace = () => {
  throw new Error("spawnmodal: configureSpawnModal not called");
};
let onCliLook: (badge: string, label: string) => { color: string; mono: string } = () => ({
  color: "#c6f135",
  mono: "?",
});
export function configureSpawnModal(deps: {
  createAgent: (
    ws: Workspace,
    spec: AgentSpec,
    restore?: boolean,
    attach?: { id: string; spawnedAt: number | null },
  ) => () => Promise<void>;
  createWorkspace: (dir: string | null, name?: string) => Workspace;
  cliLook: (badge: string, label: string) => { color: string; mono: string };
}): void {
  onCreateAgent = deps.createAgent;
  onCreateWorkspace = deps.createWorkspace;
  onCliLook = deps.cliLook;
}

/* ---------------- which CLIs are installed ---------------- */

// Which preset binaries resolve on PATH. Null until the first probe lands.
let cliAvailable: Record<string, boolean> | null = null;

/** Probe every preset's binary once. Fire-and-forget: a failed probe leaves
 *  everything treated as installed. Call at startup and when New agent opens. */
export function refreshCliAvailability(): Promise<void> {
  const programs = CLI_PRESETS.map((p) => p.program);
  return programsOnPath(programs)
    .then((results) => {
      const map: Record<string, boolean> = {};
      programs.forEach((prog, i) => { map[prog] = results[i] ?? true; });
      cliAvailable = map;
    })
    .catch(() => { /* leave everything as installed */ });
}

const STORE_KEY = "maestro.crew";
const MAX_CONCURRENT_BOOT = 3;

interface SavedCrew extends CrewState {
  dir: string;
  skipPerms: boolean;
}

export function loadCrew(): SavedCrew {
  try {
    const s = JSON.parse(localStorage.getItem(STORE_KEY) || "{}");
    return {
      counts: s.counts && typeof s.counts === "object" ? s.counts : {},
      custom: typeof s.custom === "string" ? s.custom : "",
      customCount: Number.isFinite(s.customCount) ? s.customCount : 0,
      dir: typeof s.dir === "string" ? s.dir : "",
      skipPerms: s.skipPerms === true,
    };
  } catch {
    return { counts: {}, custom: "", customCount: 0, dir: "", skipPerms: false };
  }
}

/** Persist just the permission mode, leaving the rest of the saved crew alone.
 *  The inline +Agent menu writes through this so both spawn entry points agree
 *  on the mode — an agent booting in manual mode when the last one skipped
 *  permissions reads as a bug, not a default. */
export function saveSkipPerms(skipPerms: boolean): void {
  const saved = loadCrew();
  localStorage.setItem(STORE_KEY, JSON.stringify({ ...saved, skipPerms }));
}

/** Core spawn: expand a crew → choose/create a workspace → mount & boot the
 *  fleet (concurrency-limited). Shared by the spawn modal and saved templates. */
export async function spawnCrew(
  crewState: CrewState,
  dir: string | null,
  skipPerms: boolean,
  mode: "new" | "current",
  conductor = false,
): Promise<void> {
  const fleet = expandCrew(crewState);
  // Conductor CONVERTS the first agent — it doesn't add one. With no workers
  // picked, spawn a single Claude conductor.
  if (fleet.length === 0 && conductor) fleet.push(CLI_PRESETS.find((x) => x.id === "claude")!);
  if (fleet.length === 0) return;

  // Spawn into the active workspace, or a brand-new tab.
  const ws = mode === "current" && activeWs ? activeWs : onCreateWorkspace(dir);
  if (mode === "current" && activeWs && !activeWs.dir && dir) activeWs.dir = dir;

  // The first agent becomes the conductor when the toggle is on (same CLI as
  // your first pick). Name the rest per CLI: "Claude Code #1", "#2"; plain when
  // there is only one worker of that CLI.
  const conductorIdx = conductor ? 0 : -1;
  // Each pane gets a short persona name (Ana, Bob, …), unique in this workspace;
  // the director keeps the "Director" label — which is also the name the fleet
  // tools address it by, so it has to match what the header shows.
  // Renameable from the title bar.
  const taken: string[] = [...ws.panes.values()].map((x) => x.spec.name);
  let conductorIsClaude = false;

  const boots = fleet.map((p: CliPreset, i) => {
    if (i === conductorIdx) {
      conductorIsClaude = p.badge === "claude";
      taken.push("Director");
      return onCreateAgent(ws, {
        program: p.program,
        args: effectiveArgs(p, skipPerms),
        cwd: dir,
        name: "Director",
        badge: p.badge,
        role: "conductor",
        ...onCliLook(p.badge, p.label),
      });
    }
    const name = nameForNewPane(p.badge, taken);
    taken.push(name);
    return onCreateAgent(ws, {
      program: p.program,
      args: effectiveArgs(p, skipPerms),
      cwd: dir,
      name,
      badge: p.badge,
      role: p.role,
      ...onCliLook(p.badge, p.label),
    });
  });

  // Boot through a concurrency-limited queue so many heavy CLIs don't all start
  // at once and spike the CPU (panes already appeared above as "queued…").
  await runLimited(boots, MAX_CONCURRENT_BOOT);

  // A Claude director gets DIRECTOR_LAWS via --append-system-prompt at launch.
  // Other CLIs have no such flag, so prime the director by typing the same
  // instructions once it reaches its prompt.
  if (conductor && !conductorIsClaude) {
    const pane = [...ws.panes.values()].find((x) => x.spec.role === "conductor");
    if (pane) deliverTask(ws, [pane.spec.name], DIRECTOR_LAWS, 3500);
  }
}

/** A conductor agent asked (via the bytehive-mcp agent_spawn tool) to grow its
 *  crew. Spawn the worker(s) into the SAME open workspace (so they share the
 *  board + fleet), with names unique in that workspace, and — if a task was
 *  given — type it into each once they've had a moment to reach their prompt. */
export async function spawnForConductor(
  dir: string,
  req: { cli: string; task: string | null; count: number },
): Promise<void> {
  const ws = [...workspaces.values()].find((w) => w.dir === dir);
  if (!ws) return; // the requesting agent's workspace isn't open anymore
  const preset = CLI_PRESETS.find((p) => p.id === req.cli);
  const state: CrewState = preset
    ? { counts: { [req.cli]: req.count }, custom: "", customCount: 0 }
    : { counts: {}, custom: req.cli, customCount: req.count };
  const fleet = expandCrew(state);
  if (!fleet.length) return;
  await prepareIsolation(ws);
  const newNames: string[] = [];
  const boots = fleet.map((p) => {
    const base = p.shell && dir ? basename(dir) : p.label;
    const taken = new Set([...ws.panes.values()].map((x) => x.spec.name));
    let name = base;
    for (let n = 2; taken.has(name); n += 1) name = `${base} #${n}`;
    newNames.push(name);
    return onCreateAgent(ws, {
      program: p.program,
      args: effectiveArgs(p, false),
      cwd: dir,
      name,
      badge: p.badge,
      title: taskTitle(req.task),
      ...onCliLook(p.badge, p.label),
    });
  });
  await runLimited(boots, MAX_CONCURRENT_BOOT);
  if (req.task) deliverTask(ws, newNames, req.task, jobDelayMs());
}

/** A CLI's pane colour and letter, for code outside main.ts. */
export const cliLookOf = (badge: string, label: string) => onCliLook(badge, label);

/** Start a member of the company in its floor's project. Once the repo is known
 *  the pane gives a member a worktree by itself (needsWorktree), so the project's
 *  own setting stays as it is for the agents who aren't members. */
export async function spawnPerson(dir: string, spec: AgentSpec, boot: boolean): Promise<void> {
  const ws = onCreateWorkspace(dir);
  await findRepo(ws);
  const start = onCreateAgent(ws, spec);
  if (boot) await runLimited([start], MAX_CONCURRENT_BOOT);
}

/** Type the job into each new agent once it is at its prompt. The CLI needs a
 *  few seconds to get there, and some open on a question first: Claude asks
 *  whether to trust a folder it hasn't seen, like a new worktree, with "No,
 *  exit" selected, so the job's Enter would answer it and close the agent.
 *  While a question is on screen the job waits for you to answer it, then
 *  gives the CLI the same head start again: answering the trust question
 *  restarts Claude's session, and a job typed before it is ready is lost.
 *  `settleMs` is that second head start, when it isn't the same as the first. */
export function deliverTask(ws: Workspace, names: string[], task: string, delayMs: number, everyMs = 1000, giveUpMs = 15 * 60_000, settleMs = delayMs): void {
  const left = new Set(names);
  const start = Date.now() + delayMs;
  /** When each agent's question went away, for agents that asked one. */
  const answered = new Map<string, number | null>();
  const tick = () => {
    for (const name of [...left]) {
      const pane = [...ws.panes.values()].find((x) => x.spec.name === name);
      if (!pane || !pane.running) { left.delete(name); continue; }
      if (paneAsking(pane)) {
        answered.set(name, null);
        if (Date.now() - start > giveUpMs) left.delete(name); // never answer it for you
        continue;
      }
      if (answered.has(name)) {
        const at = answered.get(name) ?? Date.now();
        answered.set(name, at);
        if (Date.now() - at < settleMs) continue;
      }
      // A resume can take half a minute with hooks and MCP servers starting.
      if (!paneAtPrompt(pane) && Date.now() - start < PROMPT_WAIT_MS) continue;
      left.delete(name);
      void sendMessage(pane.id, task).catch(() => {});
    }
    if (left.size) window.setTimeout(tick, everyMs);
  };
  window.setTimeout(tick, delayMs);
}

/** Settings → Agents: give every agent its own worktree when the project is a
 *  git repo. Looked up once per project; the pane boot does the rest. */
export async function prepareIsolation(ws: Workspace): Promise<void> {
  if (!getPref("worktree")) { ws.isolated = false; return; }
  if (!ws.dir) return;
  await findRepo(ws);
  ws.isolated = !!ws.repoRoot;
}

async function findRepo(ws: Workspace): Promise<void> {
  if (ws.dir && !ws.repoRoot) ws.repoRoot = await gitRepoRoot(ws.dir).catch(() => null);
}

/** Screen lines looked at for a question the CLI opened on. */
const ASK_LINES = 40;
/** How long a job waits for Claude's input box. A screen that never shows one is
 *  one this can't read, and typing anyway beats losing the job. */
const PROMPT_WAIT_MS = 2 * 60_000;

/** Whether the CLI in this pane is showing a question, which an Enter would answer. */
export function paneAsking(pane: Pane): boolean {
  return !!parseAsk(pane.term.snapshot(ASK_LINES));
}

/** Whether this pane is ready to be typed into. Only Claude draws a box to look for. */
export function paneAtPrompt(pane: Pane): boolean {
  return pane.spec.badge !== "claude" || composerDrawn(pane.term.snapshot(ASK_LINES));
}

/** Settings → Agents: how long a new CLI gets to reach its prompt. */
function jobDelayMs(): number {
  return getPref("jobDelay") * 1000;
}

/** A plain PowerShell in `dir` (or the home folder), as its own project. */
export function quickTerminal(dir: string | null): void {
  const ps = CLI_PRESETS.find((p) => p.id === "powershell")!;
  const ws = onCreateWorkspace(dir);
  void onCreateAgent(ws, {
    program: ps.program,
    args: ps.args,
    cwd: dir,
    name: dir ? basename(dir) : "powershell",
    badge: ps.badge,
    ...onCliLook(ps.badge, ps.label),
  })();
}

/** True unless the probe found this program missing from PATH (unknown counts
 *  as installed, so nothing dims before the probe lands). */
export function presetAvailable(program: string): boolean {
  return cliAvailable === null || cliAvailable[program] !== false;
}

/** New agent (the Agent Inbox form): start `count` agents of one CLI in `ws`,
 *  named like every other pane (Ana, Bob, …), and type `task` into each once
 *  it has reached its prompt. Several agents on one task is a race: compare
 *  their diffs and keep the best. `presetId` is a preset id or a free-text
 *  custom command ("custom:…" or plain). Returns the new agents' names. */
export async function spawnAgents(
  ws: Workspace,
  presetId: string,
  count: number,
  task: string | null,
  skipPerms: boolean,
): Promise<string[]> {
  const preset = resolvePreset(presetId);
  if (!preset || count < 1) return [];
  saveSkipPerms(skipPerms);
  await prepareIsolation(ws);
  const taken: string[] = [...ws.panes.values()].map((x) => x.spec.name);
  const names: string[] = [];
  const title = taskTitle(task);
  const raceId = count > 1 ? `race-${Date.now().toString(36)}` : null;
  // Settings → Agents: the first agent of an empty project can be its Director.
  const director = getPref("directorFirst") && ws.panes.size === 0 && count === 1;
  const boots = Array.from({ length: count }, (_, i) => {
    const name = director ? "Director" : nameForNewPane(preset.badge, taken);
    taken.push(name);
    names.push(name);
    return onCreateAgent(ws, {
      program: preset.program,
      args: effectiveArgs(preset, skipPerms),
      cwd: ws.dir,
      name,
      badge: preset.badge,
      role: director ? "conductor" : preset.role,
      title,
      race: raceId ? { id: raceId, n: i + 1, of: count } : undefined,
      ...onCliLook(preset.badge, preset.label),
    });
  });
  await runLimited(boots, MAX_CONCURRENT_BOOT);
  // The agent you just started is the one on screen, not the one you were on.
  const first = [...ws.panes.values()].find((x) => x.spec.name === names[0]);
  if (first) revealPane(ws.id, first.id);
  if (task) deliverTask(ws, names, task, jobDelayMs());
  return names;
}

/* ---------------- crew templates ---------------- */

export interface Template {
  id: string;
  name: string;
  counts: Record<string, number>;
  custom: string;
  customCount: number;
  dir: string;
  skipPerms: boolean;
}

const TEMPLATES_KEY = "maestro.templates";

export function loadTemplates(): Template[] {
  try {
    const v = JSON.parse(localStorage.getItem(TEMPLATES_KEY) || "[]");
    return Array.isArray(v) ? (v as Template[]) : [];
  } catch {
    return [];
  }
}
export function saveTemplates(list: Template[]) {
  try {
    localStorage.setItem(TEMPLATES_KEY, JSON.stringify(list));
  } catch {
    /* best-effort */
  }
}

/** Human-readable summary of a template's crew, e.g.
 *  "2× Claude Code · 1× Codex · my-app". */
export function templateSummary(t: Template): string {
  const parts: string[] = [];
  for (const p of CLI_PRESETS) {
    const n = t.counts[p.id] ?? 0;
    if (n > 0) parts.push(`${n}× ${p.label}`);
  }
  const custom = (t.custom ?? "").trim();
  if (custom && t.customCount > 0) parts.push(`${t.customCount}× ${custom}`);
  if (t.dir) parts.push(basename(t.dir) || t.dir);
  return parts.join(" · ");
}

/** Save a crew as a preset that Settings → Sessions → Scheduled agents can launch. */
export function saveTemplate(name: string, counts: Record<string, number>, dir: string, skipPerms: boolean, custom = "", customCount = 0): void {
  saveTemplates([...loadTemplates(), { id: "tpl-" + Math.random().toString(36).slice(2, 9), name, counts, custom, customCount, dir, skipPerms }]);
}
