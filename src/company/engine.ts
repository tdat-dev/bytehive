// src/company/engine.ts
/* Runs the company: reads requests, applies the rules, starts people, types
 * messages into their terminals. All decisions live in reduce.ts and watch.ts. */
import { homeDir } from "@tauri-apps/api/path";
import { fsCreateDir, sendMessage } from "../ipc";
import { workspaces, newId } from "../appstate";
import { drainJsonl } from "../fleetbridge";
import { CLI_PRESETS, nameForNewPane, effectiveArgs } from "../crew";
import { cliLookOf, deliverTask, spawnPerson } from "../spawnmodal";
import { readBoardFile, writeBoardFile } from "../boardfile";
import { mkCard, defaultBoard } from "../board";
import { sameFolder } from "../workspaces";
import { revealPane } from "../agentbridge";
import type { AgentSpec, Pane, Workspace } from "../panetypes";
import { buildRoleLaunch, composePrompt, pluginManifest } from "./launch";
import { personById } from "./org";
import { applyRequest, approvePlan, newJob, rejectPlan, type Effect, type State } from "./reduce";
import { companyRoot, ensureDirs, loadCompany, loadJobs, saveCompany, saveJob, writeNew } from "./store";
import { loadRoles, seedRoles, skillFiles } from "./roles";
import type { CompanyRequest, Job, Person, RoleDef } from "./types";

const OFFICE_REL = "office";
const REQUESTS_REL = "requests.jsonl";
const TICK_MS = 1500;
/** How long a new CLI gets to reach its prompt. */
const BOOT_MS = 8000;

let root = "";
let state: State | null = null;
const consumed = new Map<string, number>();
const listeners = new Set<() => void>();

export const onCompanyChange = (fn: () => void) => { listeners.add(fn); };
const changed = () => listeners.forEach((fn) => fn());

export function companyView() { return state; }

/** A floor someone from the company works on. Every open project is a floor,
 *  but one with nobody on it is still just your project. */
export function isCompanyFloor(dir: string | null): boolean {
  if (!dir || !state) return false;
  const floor = state.company.floors.find((f) => sameFolder(f.dir, dir));
  return !!floor && state.company.people.some((p) => p.floor === floor.id);
}

function paneOf(personId: string): { ws: Workspace; pane: Pane } | null {
  for (const ws of workspaces.values())
    for (const pane of ws.panes.values())
      if (pane.spec.personId === personId) return { ws, pane };
  return null;
}

export function openPerson(id: string): void {
  const found = paneOf(id);
  if (found) revealPane(found.ws.id, found.pane.id);
}

async function persist(jobs: Job[]): Promise<void> {
  if (!state) return;
  await saveCompany(root, state.company);
  for (const j of jobs) await saveJob(root, j);
  changed();
}

/** Write a person's prompt and plugin, and give back their launch recipe. */
async function personSpec(p: Person, role: RoleDef, dir: string): Promise<AgentSpec> {
  const base = `people\\${p.id}`;
  await writeNew(root, `${base}\\prompt.md`, composePrompt(p, role, state!.company));
  const skills = skillFiles(role);
  let pluginDir: string | null = null;
  if (skills.length) {
    pluginDir = `${root}\\${base}\\plugin`;
    await writeNew(root, `${base}\\plugin\\.claude-plugin\\plugin.json`, pluginManifest(p));
    for (const s of skills) await writeNew(root, `${base}\\plugin\\skills\\${s.name}\\SKILL.md`, s.text);
  }
  const launch = buildRoleLaunch({ person: p, role, promptFile: `${root}\\${base}\\prompt.md`, pluginDir });
  const preset = CLI_PRESETS.find((x) => x.id === p.cli)!;
  return {
    program: preset.program,
    args: [...effectiveArgs(preset, true), ...launch.args],
    cwd: dir,
    name: p.name,
    badge: preset.badge,
    personId: p.id,
    env: launch.env,
    title: role.title,
    ...cliLookOf(preset.badge, preset.label),
  };
}

async function ensurePerson(p: Person): Promise<void> {
  if (paneOf(p.id) || !state) return;
  const role = state.roles.find((r) => r.id === p.role);
  const floor = state.company.floors.find((f) => f.id === p.floor);
  const dir = p.rank === "founder" ? `${root}\\${OFFICE_REL}` : floor?.dir;
  if (!role || !dir) return;
  await spawnPerson(dir, await personSpec(p, role, dir), true);
}

/** Type into a person's terminal, waking them first if they are stopped. A CLI
 *  that only just started isn't at its prompt yet, and can open on a question
 *  that an Enter would answer, so deliverTask waits for it. */
async function deliver(personId: string, text: string): Promise<void> {
  const p = state && personById(state.company, personId);
  if (!p) return;
  await ensurePerson(p);
  const found = paneOf(personId);
  if (!found) return;
  const { ws, pane } = found;
  if (!pane.running) await pane.restart?.();
  const age = Date.now() - (pane.spawnedAt ?? 0);
  if (age < BOOT_MS) deliverTask(ws, [p.name], text, BOOT_MS - age);
  else await sendMessage(pane.id, text).catch(() => {});
}

async function run(effects: Effect[]): Promise<void> {
  for (const e of effects) {
    if (e.kind === "deliver") await deliver(e.personId, e.text);
    else {
      const { prompt, ...def } = e.role;
      await writeNew(root, `roles\\${e.role.id}\\role.json`, JSON.stringify(def, null, 2));
      await writeNew(root, `roles\\${e.role.id}\\prompt.md`, prompt);
    }
  }
}

function parseRequest(line: string): CompanyRequest | null {
  try {
    const r = JSON.parse(line) as CompanyRequest;
    return r && typeof r.type === "string" && typeof r.from === "string" ? r : null;
  } catch {
    return null;
  }
}

async function tick(): Promise<void> {
  if (!state) return;
  const lines: string[] = [];
  await drainJsonl(root, REQUESTS_REL, consumed, (l) => { lines.push(l); });
  for (const line of lines) {
    const req = parseRequest(line);
    if (!req) continue;
    try {
      const r = applyRequest(state, req);
      state = r.state;
      await persist(state.jobs);
      await run(r.effects);
    } catch { /* these lines are read only once: one bad request never costs the rest */ }
  }
}

/** Every open project with a folder is a floor. */
function syncFloors(): void {
  if (!state) return;
  const floors = [...state.company.floors];
  for (const ws of workspaces.values()) {
    if (!ws.dir || sameFolder(ws.dir, `${root}\\${OFFICE_REL}`) || floors.some((f) => sameFolder(f.dir, ws.dir!))) continue;
    floors.push({ id: ws.name.toLowerCase().replace(/[^a-z0-9]+/g, "-"), dir: ws.dir, name: ws.name });
  }
  if (floors.length !== state.company.floors.length) state = { ...state, company: { ...state.company, floors } };
}

export async function giveJob(ask: string): Promise<void> {
  if (!state || !ask.trim()) return;
  syncFloors();
  const r = newJob(state, ask, Date.now());
  state = r.state;
  await persist([r.job]);
  await run(r.effects);
}

async function placeCards(job: Job): Promise<void> {
  if (!state || !job.plan) return;
  for (const floor of state.company.floors) {
    const cards = job.plan.cards.filter((c) => c.floor === floor.id);
    if (!cards.length) continue;
    const file = await readBoardFile(floor.dir);
    const board = file?.board ?? defaultBoard();
    let todo = board.lists.find((l) => l.title.trim().toLowerCase() === "to do");
    if (!todo) {
      todo = { id: `l${newId()}`, title: "To do", cards: [] };
      board.lists.unshift(todo);
    }
    for (const c of cards) {
      const assignee = c.assignee ? state.company.people.find((p) => p.name === c.assignee) : undefined;
      todo.cards.push({
        ...mkCard(c.title), desc: c.desc, job: job.id,
        checklist: c.checklist.map((text, i) => ({ id: `${job.id}-${i}`, text, done: false })),
        ...(assignee ? { assignee: assignee.name, assigneeId: assignee.id } : {}),
      });
    }
    await writeBoardFile(floor.dir, board, file?.mtime ?? null);
  }
}

/** Every pane's name: deliverTask finds a pane by name in its project, so a new
 *  hire must not share one with an agent that isn't in the company. */
const paneNames = () => [...workspaces.values()].flatMap((ws) => [...ws.panes.values()].map((p) => p.spec.name));

export async function approve(id: string): Promise<void> {
  if (!state) return;
  const r = approvePlan(state, id, {
    name: (taken) => nameForNewPane("agent", [...taken, ...paneNames()]),
    id: () => `p${newId()}`,
  });
  state = r.state;
  const job = state.jobs.find((j) => j.id === id);
  if (!job) return;
  await persist([job]);
  await placeCards(job);
  for (const p of r.hired) await ensurePerson(p);
  await run(r.effects);
}

export async function reject(id: string, note: string): Promise<void> {
  if (!state) return;
  const r = rejectPlan(state, id, note);
  state = r.state;
  await persist(state.jobs.filter((j) => j.id === id));
  await run(r.effects);
}

export async function initCompany(): Promise<void> {
  const home = await homeDir();
  await fsCreateDir(home, ".bytehive").catch(() => {});
  await fsCreateDir(home, ".bytehive\\company").catch(() => {});
  root = companyRoot(home);
  await ensureDirs(root, OFFICE_REL);
  await ensureDirs(root, "roles");
  await seedRoles(root);
  const [company, jobs, roles] = await Promise.all([loadCompany(root), loadJobs(root), loadRoles(root)]);
  state = { company, jobs, roles };
  syncFloors();
  await saveCompany(root, state.company);
  changed();
  const next = () => window.setTimeout(() => void tick().catch(() => {}).finally(next), TICK_MS);
  next();
}
