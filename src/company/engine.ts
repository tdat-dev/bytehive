// src/company/engine.ts
/* Runs the company: reads requests, applies the rules, starts people, types
 * messages into their terminals. All decisions live in reduce.ts and watch.ts. */
import { homeDir } from "@tauri-apps/api/path";
import { fsCreateDir, sendMessage } from "../ipc";
import { workspaces, newId } from "../appstate";
import { drainJsonl } from "../fleetbridge";
import { CLI_PRESETS, nameForNewPane, effectiveArgs } from "../crew";
import { cliLookOf, deliverTask, paneAsking, spawnPerson } from "../spawnmodal";
import { readBoardFile, writeBoardFile } from "../boardfile";
import { mkCard, defaultBoard } from "../board";
import { sameFolder } from "../workspaces";
import { revealPane } from "../agentbridge";
import type { AgentSpec, Pane, Workspace } from "../panetypes";
import { buildRoleLaunch, composePrompt, pluginManifest } from "./launch";
import { floorId, personById, personByName } from "./org";
import { applyRequest, approvePlan, newJob, rejectPlan, reportFailures, type Effect, type State } from "./reduce";
import { companyRoot, ensureDirs, loadCompany, loadJobs, saveCompany, saveJob, writeNew } from "./store";
import { loadRoles, seedRoles, skillFiles } from "./roles";
import type { CompanyRequest, Floor, Job, Person, PlanCard, RoleDef } from "./types";

const OFFICE_REL = "office";
const REQUESTS_REL = "requests.jsonl";
const TICK_MS = 1500;
/** How long a new CLI gets to reach its prompt. */
const BOOT_MS = 8000;
/** Tries at a board an agent keeps writing to. */
const BOARD_TRIES = 3;

let root = "";
let state: State | null = null;
let loadError: string | null = null;
const consumed = new Map<string, number>();
const listeners = new Set<() => void>();

export const onCompanyChange = (fn: () => void) => { listeners.add(fn); };
const changed = () => listeners.forEach((fn) => fn());

export function companyView() { return state; }

/** Why the company could not be loaded, when it could not. */
export const companyError = () => loadError;

let queue: Promise<unknown> = Promise.resolve();

/** Run one engine operation after the ones before it. Every operation goes
 *  through here: they read, wait on files and panes, then write, and two at once
 *  would start a person twice or write an old Job over a new one. An operation
 *  must not enqueue another and wait for it. */
export function enqueue<T>(op: () => Promise<T>): Promise<T> {
  const done = queue.then(op);
  queue = done.catch(() => {});
  return done;
}

/** A failure as words: backend errors are objects, not Errors. */
function reason(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  const failed = (e as { Failed?: unknown })?.Failed;
  return typeof failed === "string" ? failed : JSON.stringify(e);
}

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
  const preset = CLI_PRESETS.find((x) => x.id === p.cli);
  if (!preset) throw new Error(`unknown CLI ${p.cli}`);
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
  if (!role) throw new Error(`no role ${p.role}`);
  if (!dir) throw new Error(`no floor ${p.floor}`);
  await spawnPerson(dir, await personSpec(p, role, dir), true);
}

/** Type into a person's terminal, waking them first if they are stopped. A CLI
 *  that only just started isn't at its prompt yet, and a question on screen would
 *  take the message's Enter as its answer: a new worktree opens on one, with
 *  "No, exit" selected. deliverTask waits out both. */
async function deliver(personId: string, text: string): Promise<void> {
  const p = state && personById(state.company, personId);
  if (!p) return;
  await ensurePerson(p);
  const found = paneOf(personId);
  if (!found) return;
  const { ws, pane } = found;
  if (!pane.running && !pane.spawning) await pane.restart?.();
  const left = pane.running ? BOOT_MS - (Date.now() - (pane.spawnedAt ?? 0)) : BOOT_MS;
  if (left > 0 || paneAsking(pane)) deliverTask(ws, [p.name], text, Math.max(left, 0), undefined, undefined, BOOT_MS);
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

/** Read and carry out whatever has come in. Runs in the queue. */
async function tick(): Promise<void> {
  if (!state) return;
  const lines: string[] = [];
  await drainJsonl(root, REQUESTS_REL, consumed, (l) => { lines.push(l); });
  for (const line of lines) {
    const req = parseRequest(line);
    if (!req) continue;
    try {
      const r = applyRequest(state, req);
      if (r.state !== state) {
        state = r.state;
        await persist(state.jobs);
      }
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
    floors.push({ id: floorId(ws.name, floors.map((f) => f.id)), dir: ws.dir, name: ws.name });
  }
  if (floors.length !== state.company.floors.length) state = { ...state, company: { ...state.company, floors } };
}

export const giveJob = (ask: string): Promise<void> => enqueue(async () => {
  if (!state || !ask.trim()) return;
  syncFloors();
  const r = newJob(state, ask, Date.now());
  state = r.state;
  await persist([r.job]);
  await run(r.effects);
});

/** Put a floor's cards on its board. An agent writing the board at the same
 *  moment is a Conflict: read its copy and try again. */
async function placeOnFloor(company: State["company"], job: Job, floor: Floor, cards: PlanCard[]): Promise<void> {
  for (let tries = 1; ; tries += 1) {
    const file = await readBoardFile(floor.dir);
    const board = file?.board ?? defaultBoard();
    let todo = board.lists.find((l) => l.title.trim().toLowerCase() === "to do");
    if (!todo) {
      todo = { id: `l${newId()}`, title: "To do", cards: [] };
      board.lists.unshift(todo);
    }
    for (const c of cards) {
      const assignee = c.assignee ? personByName(company, c.assignee) : undefined;
      todo.cards.push({
        ...mkCard(c.title), desc: c.desc, job: job.id,
        checklist: c.checklist.map((text, i) => ({ id: `${job.id}-${i}`, text, done: false })),
        ...(assignee ? { assignee: assignee.name, assigneeId: assignee.id } : {}),
      });
    }
    try {
      await writeBoardFile(floor.dir, board, file?.mtime ?? null);
      return;
    } catch (e) {
      if ((e as { Conflict?: number })?.Conflict === undefined || tries >= BOARD_TRIES) throw e;
    }
  }
}

/** Put a Job's cards on the boards. Gives back what could not be placed. */
async function placeCards(job: Job): Promise<string[]> {
  const failed: string[] = [];
  if (!state || !job.plan) return failed;
  for (const floor of state.company.floors) {
    const cards = job.plan.cards.filter((c) => c.floor === floor.id);
    if (!cards.length) continue;
    try {
      await placeOnFloor(state.company, job, floor, cards);
    } catch (e) {
      failed.push(`Cards for ${floor.id} were not placed: ${reason(e)}.`);
    }
  }
  return failed;
}

/** Start the new hires. Gives back who did not start, and why. */
async function startHires(hired: Person[]): Promise<string[]> {
  const failed: string[] = [];
  for (const p of hired) {
    try {
      await ensurePerson(p);
      const error = paneOf(p.id)?.pane.error;
      if (error) failed.push(`${p.name} did not start: ${error}.`);
    } catch (e) {
      failed.push(`${p.name} did not start: ${reason(e)}.`);
    }
  }
  return failed;
}

/** Every pane's name: deliverTask finds a pane by name in its project, so a new
 *  hire must not share one with an agent that isn't in the company. */
const paneNames = () => [...workspaces.values()].flatMap((ws) => [...ws.panes.values()].map((p) => p.spec.name));

export const approve = (id: string): Promise<void> => enqueue(async () => {
  if (state?.jobs.find((j) => j.id === id)?.status !== "awaiting-plan") return;
  const r = approvePlan(state, id, {
    name: (taken) => nameForNewPane("agent", [...taken, ...paneNames()]),
    id: () => `p${newId()}`,
  });
  state = r.state;
  const job = state.jobs.find((j) => j.id === id);
  if (!job) return;
  await persist([job]);
  // Whatever fails from here, the Job is running: the Founder has to hear about it.
  const failed = [...await placeCards(job), ...await startHires(r.hired)];
  await run(reportFailures(r.effects, failed));
});

export const reject = (id: string, note: string): Promise<void> => enqueue(async () => {
  if (!state) return;
  const r = rejectPlan(state, id, note);
  if (r.state === state) return;
  state = r.state;
  await persist(state.jobs.filter((j) => j.id === id));
  await run(r.effects);
});

export async function initCompany(): Promise<void> {
  const home = await homeDir();
  await fsCreateDir(home, ".bytehive").catch(() => {});
  await fsCreateDir(home, ".bytehive\\company").catch(() => {});
  root = companyRoot(home);
  await ensureDirs(root, OFFICE_REL);
  await ensureDirs(root, "roles");
  await seedRoles(root);
  try {
    const [company, jobs, roles] = await Promise.all([loadCompany(root), loadJobs(root), loadRoles(root)]);
    state = { company, jobs, roles };
  } catch (e) {
    // No state means nothing writes: the file stays as it is for you to mend.
    loadError = reason(e);
    console.error(`[bytehive] the company could not be loaded: ${loadError}`);
    changed();
    return;
  }
  syncFloors();
  await saveCompany(root, state.company);
  changed();
  const next = () => window.setTimeout(() => void enqueue(tick).catch(() => {}).finally(next), TICK_MS);
  next();
}
