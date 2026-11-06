// src/company/engine.ts
/* Runs the company: reads requests, applies the rules, starts people, types
 * messages into their terminals. All decisions live in reduce.ts and watch.ts. */
import { homeDir, resolveResource } from "@tauri-apps/api/path";
import { agentUsage } from "../agentcost";
import { appPid, claudeSessionExists, fsCreateDir, fsReadDir, gitExcludeBytehive, processAlive, runCapture, sendMessage } from "../ipc";
import { activeWs, workspaces, newId } from "../appstate";
import { drainJsonl } from "../fleetbridge";
import { CLI_PRESETS, launchSpec, nameForNewPane, effectiveArgs } from "../crew";
import { cliLookOf, closeProject, deliverTask, paneAsking, paneAtPrompt, spawnPerson } from "../spawnmodal";
import { readBoardFile, writeBoardFile } from "../boardfile";
import { mkCard, defaultBoard } from "../board";
import { sameFolder } from "../workspaces";
import { revealPane } from "../agentbridge";
import type { AgentSpec, Pane, Workspace } from "../panetypes";
import { buildRoleLaunch, composePrompt, founderFolder, FOUNDER_SETTINGS, mcpConfig, nodeVerdict, pluginManifest } from "./launch";
import { floorId, personById, personByName } from "./org";
import { applyRequest, approvePlan, haltJob, joinJob, newJob, notePerson, closeJob, pausedNote, pauseJob, rejectPlan, reportFailures, resumedNotes, resumeJob, stoppedNote, type Effect, type State } from "./reduce";
import { clearAlert, onStandingChange, raiseAlert, standingAlerts } from "./alerts";
import { LOCK_STALE_MS, lockMove, lockSilent } from "./lock";
import { companyRoot, ensureDirs, loadCompany, loadJobs, readLock, saveCompany, saveJob, writeLock, writeNew } from "./store";
import { loadRoles, roleSkills, seedRoles } from "./roles";
import { watchBudget, watchWork, type Alert, type WatchedCard } from "./watch";
import { FOUNDER_ID, type CompanyRequest, type Floor, type Job, type Person, type PlanCard, type RoleDef } from "./types";

/** Where 0.14.0 ran the Founder: only here to close the tab left from then. The folder stays on disk. */
const OLD_OFFICE_REL = "office";
const REQUESTS_REL = "requests.jsonl";
/** Where tauri.conf.json puts the MCP server, under the app's resources. */
const MCP_RESOURCE = "mcp-server/bytehive-mcp.mjs";
const TICK_MS = 1500;
const WATCH_MS = 30_000;
/** How long a new CLI gets to reach its prompt. */
const BOOT_MS = 8000;
/** How long `node --version` gets. */
const NODE_WAIT_MS = 30_000;
/** Tries at a board an agent keeps writing to. */
const BOARD_TRIES = 3;
const NO_PROJECT = "Open a project first.";
const FOUNDER_WAITS = "founder-no-project";

let root = "";
let me = 0; // this app's pid
/** This app runs the company. When it doesn't, state is null, and every operation
 *  stops at that: nothing here reads a request or writes a file. */
let holding = false;
/** Another running app does, as far as the lock says. */
let elsewhere = false;
/** When the last heartbeat ran, and when this app last woke or saw its clock jump. */
let lastBeat = 0;
let awakeSince = 0;
let state: State | null = null;
let loadError: string | null = null;
let alerts: Alert[] = [];
let nudges = new Map<string, number>();
/** When each card in Doing went quiet. */
const quiet = new Map<string, number>();
const consumed = new Map<string, number>();
/** Folders whose git has been told about .bytehive/ in this run. */
const excluded = new Set<string>();
const listeners = new Set<() => void>();

export const onCompanyChange = (fn: () => void) => { listeners.add(fn); };
const changed = () => listeners.forEach((fn) => fn());
onStandingChange(changed);

export function companyView() { return state; }

/** Why the company could not be loaded, when it could not. */
export const companyError = () => loadError;

/** Where the company runs, when that isn't here. */
export function companyElsewhere(): string | null {
  if (new URLSearchParams(location.search).has("detach")) return "The Company runs in the main window.";
  return elsewhere ? "The Company is running in another ByteHive window." : null;
}

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
  try {
    await saveCompany(root, state.company);
    for (const j of jobs) await saveJob(root, j);
  } finally {
    changed(); // the screen follows memory, saved or not
  }
}

/** Save, and carry on if the disk says no (antivirus holding a file, a full
 *  disk): what a request asked for must still happen. The next save tries again. */
async function tryPersist(jobs: Job[]): Promise<void> {
  try {
    await persist(jobs);
  } catch (e) {
    console.error(`[bytehive] the company could not be saved: ${reason(e)}`);
  }
}

/** Keep a project's .bytehive/ out of its git status. Not awaited by anyone: a git
 *  that fails or hangs must not hold up a job. */
async function keepOutOfGit(dir: string): Promise<void> {
  const key = dir.toLowerCase();
  if (excluded.has(key)) return;
  excluded.add(key);
  await gitExcludeBytehive(dir).catch((e) => {
    excluded.delete(key); // the next job tries again
    console.error(`[bytehive] .bytehive/ could not be excluded in ${dir}: ${reason(e)}`);
  });
}

const writeSettings = (p: Person) => writeNew(root, `people\\${p.id}\\settings.json`, FOUNDER_SETTINGS);

const writeMcpConfig = async (p: Person) =>
  writeNew(root, `people\\${p.id}\\mcp.json`, mcpConfig(await resolveResource(MCP_RESOURCE)));

/** Write a person's plugin from their role's skills. Gives back its folder, or null with no skills. */
async function writePlugin(p: Person, role: RoleDef): Promise<string | null> {
  const skills = await roleSkills(root, role);
  if (!skills.length) return null;
  const base = `people\\${p.id}\\plugin`;
  await writeNew(root, `${base}\\.claude-plugin\\plugin.json`, pluginManifest(p));
  for (const s of skills) await writeNew(root, `${base}\\skills\\${s.name}\\SKILL.md`, s.text);
  return `${root}\\${base}`;
}

/** Write a person's prompt, MCP config and plugin, and give back their launch recipe. */
async function personSpec(p: Person, role: RoleDef, dir: string): Promise<AgentSpec> {
  const base = `people\\${p.id}`;
  await writeNew(root, `${base}\\prompt.md`, composePrompt(p, role, state!.company));
  await writeMcpConfig(p);
  const pluginDir = await writePlugin(p, role);
  // The Founder has no shell and needs no hooks, and the owner's write into the project it sits in.
  const founder = p.rank === "founder";
  if (founder) await writeSettings(p);
  const launch = buildRoleLaunch({ person: p, role, promptFile: `${root}\\${base}\\prompt.md`, mcpConfig: `${root}\\${base}\\mcp.json`, pluginDir, settings: founder ? `${root}\\${base}\\settings.json` : null });
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
    ...(p.session ? { sessionId: p.session } : {}),
    ...(p.worktree ? { worktree: p.worktree, branch: p.branch } : {}),
  };
}

/** A person comes back on the conversation and branch they had. What is gone can't
 *  be: take it off the spec so they start clean, and tell the owner they start blank.
 *  A conversation belongs to its folder, so it goes with a worktree. */
async function checkRestore(p: Person, spec: AgentSpec): Promise<void> {
  const gone: string[] = [];
  let commits = "";
  if (spec.worktree && !(await fsReadDir(spec.worktree, "").then(() => true, () => false))) {
    gone.push("worktree");
    if (spec.branch) commits = ` Anything they committed is on branch ${spec.branch}.`;
    delete spec.worktree;
    delete spec.branch;
  }
  if (spec.sessionId && !(await claudeSessionExists(spec.worktree ?? spec.cwd ?? "", spec.sessionId).catch(() => false))) {
    // The Founder moves with the owner's projects, and the Job files carry its work: nothing was lost.
    if (p.rank !== "founder") gone.push("conversation");
    delete spec.sessionId;
  }
  if (gone.length) raiseAlert(`lost-${p.id}`, `${p.name}'s ${gone.join(" and ")} could not be found, so they start blank.${commits}`, p.id);
}

/** Note what every pane runs on, so closing one loses nothing. Gives back whether anything moved. */
function remember(): boolean {
  if (!state) return false;
  const before = state;
  for (const p of before.company.people) {
    const spec = paneOf(p.id)?.pane.spec;
    if (spec) state = notePerson(state, p.id, { session: spec.sessionId, worktree: spec.worktree, branch: spec.branch });
  }
  return state !== before;
}

/** The folder a Founder with no pane starts in. */
const founderDir = () => founderFolder(activeWs, [...workspaces.values()]);

async function ensurePerson(p: Person): Promise<void> {
  if (paneOf(p.id) || !state) return;
  const role = state.roles.find((r) => r.id === p.role);
  const floor = state.company.floors.find((f) => f.id === p.floor);
  const dir = p.rank === "founder" ? founderDir() : floor?.dir;
  if (!role) throw new Error(`no role ${p.role}`);
  if (!dir) throw new Error(p.rank === "founder" ? NO_PROJECT : `no floor ${p.floor}`);
  if (p.rank === "founder") void keepOutOfGit(dir);
  const spec = await personSpec(p, role, dir);
  await checkRestore(p, spec);
  await spawnPerson(dir, spec, true);
  if (p.rank === "founder") clearAlert(FOUNDER_WAITS);
  if (remember()) await tryPersist([]);
}

/** Type into a person's terminal, waking them first if they are stopped. A CLI
 *  that only just started isn't at its prompt yet, and a question on screen would
 *  take the message's Enter as its answer: a new worktree opens on one, with
 *  "No, exit" selected. deliverTask waits out both. */
async function deliver(personId: string, text: string): Promise<void> {
  const p = state && personById(state.company, personId);
  if (!p) return;
  // Starting them would throw, and take the rest of this round's effects with it.
  if (p.rank === "founder" && !paneOf(p.id) && !founderDir()) {
    raiseAlert(FOUNDER_WAITS, "No project is open, so a message for the Founder was not delivered. Open a project: the Founder starts there with its next job and sees the open work.");
    return;
  }
  await ensurePerson(p);
  const found = paneOf(personId);
  if (!found) return;
  const { ws, pane } = found;
  if (!pane.running && !pane.spawning) await pane.restart?.();
  const left = pane.running ? BOOT_MS - (Date.now() - (pane.spawnedAt ?? 0)) : BOOT_MS;
  if (left > 0 || paneAsking(pane) || !paneAtPrompt(pane)) deliverTask(ws, [p.name], text, Math.max(left, 0), undefined, undefined, BOOT_MS);
  else await sendMessage(pane.id, text).catch(() => {});
}

async function run(effects: Effect[]): Promise<void> {
  for (const e of effects) {
    if (e.kind === "owner") raiseAlert(e.key, e.text);
    else await deliver(e.personId, e.text);
  }
}

/** Write the roles a plan drafted, now the owner has approved them. Gives back which could not be. */
async function saveRoles(roles: RoleDef[]): Promise<string[]> {
  const failed: string[] = [];
  for (const { prompt, ...def } of roles) {
    try {
      await writeNew(root, `roles\\${def.id}\\role.json`, JSON.stringify(def, null, 2));
      await writeNew(root, `roles\\${def.id}\\prompt.md`, prompt);
    } catch (e) {
      failed.push(`Role ${def.id} was not saved: ${reason(e)}.`);
    }
  }
  return failed;
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
        await tryPersist(state.jobs);
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
    if (!ws.dir || floors.some((f) => sameFolder(f.dir, ws.dir!))) continue;
    floors.push({ id: floorId(ws.name, floors.map((f) => f.id)), dir: ws.dir, name: ws.name });
  }
  if (floors.length !== state.company.floors.length) state = { ...state, company: { ...state.company, floors } };
}

/** The floors a Job can touch. Never for a launch alone: a project is a floor just
 *  by being open, and someone who has not used the Company has their repo left alone. */
function excludeFloors(): void {
  for (const f of state?.company.floors ?? []) void keepOutOfGit(f.dir);
}

export const giveJob = (ask: string): Promise<void> => enqueue(async () => {
  if (!state || !ask.trim()) return;
  if (!paneOf(FOUNDER_ID) && !founderDir()) throw new Error(NO_PROJECT);
  // Every member's MCP server runs on it: without Node the Founder has no job_plan.
  const node = launchSpec("node", ["--version"]);
  const verdict = nodeVerdict(await runCapture(node.program, node.args, null, NODE_WAIT_MS).then((printed) => ({ printed }), (e) => ({ failed: reason(e) })));
  if (verdict.refuse) throw new Error(verdict.refuse);
  if (verdict.log) console.error(`[bytehive] ${verdict.log}`);
  syncFloors();
  excludeFloors();
  const r = newJob(state, ask, Date.now());
  state = r.state;
  await tryPersist([r.job]);
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

/** What a person's prompt says about their boss changes with the floor, their MCP
 *  config follows the app, and the owner can edit a skill: whoever restarts reads
 *  these files again. Gives back whose could not be written. */
async function refreshFiles(people: Person[]): Promise<string[]> {
  const failed: string[] = [];
  for (const p of people) {
    const role = state?.roles.find((r) => r.id === p.role);
    if (!state || !role) continue;
    await writeNew(root, `people\\${p.id}\\prompt.md`, composePrompt(p, role, state.company))
      .catch((e) => failed.push(`${p.name}'s prompt was not updated: ${reason(e)}.`));
    await writeMcpConfig(p).catch((e) => failed.push(`${p.name}'s MCP config was not updated: ${reason(e)}.`));
    await writePlugin(p, role).catch((e) => failed.push(`${p.name}'s skills were not updated: ${reason(e)}.`));
    if (p.rank === "founder") await writeSettings(p).catch((e) => failed.push(`${p.name}'s settings were not updated: ${reason(e)}.`));
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
  // Whatever fails from here, the Job is running: the Founder has to hear about it.
  const failed: string[] = [];
  try {
    await persist([job]);
  } catch (e) {
    failed.push(`${job.id} could not be saved: ${reason(e)}.`);
  }
  failed.push(...await saveRoles(r.saved), ...await refreshFiles(r.rewired), ...await placeCards(job), ...await startHires(r.hired));
  await run(reportFailures(r.effects, failed));
});

export const reject = (id: string, note: string): Promise<void> => enqueue(async () => {
  if (!state) return;
  const r = rejectPlan(state, id, note);
  if (r.state === state) return;
  state = r.state;
  await tryPersist(state.jobs.filter((j) => j.id === id));
  await run(r.effects);
});

/** 0.14.0 ran the Founder in a folder of its own, which opened as a project. The
 *  session brings that tab back, stopped: it goes, with the Founder's pane in it. */
async function closeOldOffice(): Promise<void> {
  for (const ws of [...workspaces.values()]) {
    if (!sameFolder(ws.dir, `${root}\\${OLD_OFFICE_REL}`)) continue;
    await closeProject(ws).catch((e) => console.error(`[bytehive] the old office tab could not be closed: ${reason(e)}`));
  }
}

/** Load the company and take charge of it. Runs in the queue. */
async function start(): Promise<void> {
  loadError = null;
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
  await closeOldOffice();
  syncFloors();
  if (state.jobs.length) excludeFloors();
  try {
    await saveCompany(root, state.company);
  } catch (e) {
    // Nothing is lost: the next change saves it again.
    console.error(`[bytehive] the company could not be saved: ${reason(e)}`);
  }
  // A session restarts on the file it was given: after an update that is old laws.
  for (const failed of await refreshFiles(state.company.people)) console.error(`[bytehive] ${failed}`);
  changed();
}

/** Another app holds the lock: let go of everything, the files are its now. In the
 *  queue, so no operation loses its state halfway. */
async function stepDown(): Promise<void> {
  holding = false;
  state = null;
  alerts = [];
  nudges = new Map();
  quiet.clear();
  consumed.clear();
  changed();
}

/** Who runs the company, every TICK_MS and outside the queue: a long operation
 *  must not make the lock look abandoned. */
async function heartbeat(): Promise<void> {
  const read = await readLock(root);
  const now = Date.now();
  // A beat comes every TICK_MS. A gap this long, either way, is a sleep or a moved clock: it says nothing about the holder.
  if (Math.abs(now - lastBeat) >= LOCK_STALE_MS) awakeSince = now;
  lastBeat = now;
  // Asked only of a lock gone quiet. If the answer doesn't come, count them alive: staying out costs less than starting everyone twice.
  const silent = read && read !== "unknown" && lockSilent(read, me, now) ? read : null;
  const alive = silent ? await processAlive(silent.pid).catch(() => true) : false;
  const move = lockMove(read, me, now, alive, awakeSince);
  if (move === "skip") return;
  if (move === "stay-out") {
    const news = !elsewhere;
    elsewhere = true;
    if (holding) await enqueue(stepDown);
    else if (news) changed();
    return;
  }
  await writeLock(root, { pid: me, at: Date.now() }).catch(() => {});
  elsewhere = false;
  if (move === "take-over") raiseAlert("lock-takeover", `Another ByteHive app (process ${silent?.pid}) stopped answering for 10 minutes, so this one took over the Company. If that app is still open, close it: the people it started are still running, next to the ones started here.`);
  if (holding) return;
  holding = true;
  // Not awaited: start waits its turn in the queue, and this is what keeps the lock fresh meanwhile.
  void enqueue(start).catch((e) => {
    loadError = reason(e);
    console.error(`[bytehive] the company could not start: ${loadError}`);
    changed();
  });
}

export async function initCompany(): Promise<void> {
  const home = await homeDir();
  await fsCreateDir(home, ".bytehive").catch(() => {});
  await fsCreateDir(home, ".bytehive\\company").catch(() => {});
  root = companyRoot(home);
  me = await appPid();
  lastBeat = Date.now();
  await heartbeat();
  const beat = () => window.setTimeout(() => void heartbeat().catch(() => {}).finally(beat), TICK_MS);
  beat();
  const next = () => window.setTimeout(() => void enqueue(tick).catch(() => {}).finally(next), TICK_MS);
  next();
  window.setInterval(() => void enqueue(watch).catch(() => {}), WATCH_MS);
}

/** What everyone on a Job has added in fresh tokens since it last looked. Each
 *  person's last-seen figure is kept in tokenBase and only what they add on top
 *  is counted, so a closed pane costs nothing already counted, and a session that
 *  starts over (/clear, a respawn) is rebased rather than subtracted. A person
 *  whose usage has not been read yet is left out until it has: counting it as
 *  zero would charge the Job for everything they did before. Gives back the same
 *  Job when nothing moved, so an idle company writes nothing. */
function measure(job: Job): Job {
  if (job.status !== "running" && job.status !== "paused") return job;
  const seen = { ...job.tokenBase };
  let added = 0;
  let moved = false;
  for (const id of job.people) {
    const found = paneOf(id);
    const usage = found ? agentUsage(found.pane) : null;
    if (!usage) continue;
    // No figure for a person we have seen is a read that came back empty, not a restart.
    if (usage.fresh === null && id in seen) continue;
    const fresh = usage.fresh ?? 0;
    if (seen[id] === fresh) continue;
    if (id in seen && fresh > seen[id]) added += fresh - seen[id];
    seen[id] = fresh;
    moved = true;
  }
  return moved ? { ...job, tokenBase: seen, tokensUsed: job.tokensUsed + added } : job;
}

/** The cards in Doing that belong to a running Job, and how long each has been quiet. */
async function doingCards(): Promise<WatchedCard[]> {
  const out: WatchedCard[] = [];
  const inDoing = new Set<string>();
  let readAll = true;
  // Every open project is a floor: only one with people on it can hold a Job's card.
  const floors = state?.company.floors.filter((f) => state?.company.people.some((p) => p.floor === f.id)) ?? [];
  for (const floor of floors) {
    const file = await readBoardFile(floor.dir).catch(() => { readAll = false; return null; });
    const doing = file?.board.lists.find((l) => l.title.trim().toLowerCase() === "doing");
    for (const c of doing?.cards ?? []) {
      inDoing.add(c.id);
      if (!c.job || !c.assigneeId) continue;
      // A paused Job is quiet on purpose.
      if (state?.jobs.find((j) => j.id === c.job)?.status !== "running") { quiet.delete(c.id); continue; }
      const pane = paneOf(c.assigneeId)?.pane;
      const working = !!pane?.running && Date.now() - pane.lastOutputAt < 60_000;
      if (working) quiet.delete(c.id);
      else if (!quiet.has(c.id)) quiet.set(c.id, Date.now());
      out.push({ id: c.id, title: c.title, job: c.job, assigneeId: c.assigneeId, quietSince: quiet.get(c.id) ?? null, returns: c.returns ?? 0 });
    }
  }
  // A card that left Doing starts its quiet time over when it comes back.
  if (readAll) for (const id of quiet.keys()) if (!inDoing.has(id)) quiet.delete(id);
  return out;
}

/** Every WATCH_MS: who has gone quiet, and how many tokens each Job has used. Runs in the queue. */
async function watch(): Promise<void> {
  if (!state) return;
  const cards = await doingCards();
  const work = watchWork({
    cards,
    nudges,
    limits: state.company.limits,
    bossOf: (id) => (state ? personById(state.company, id)?.boss ?? null : null),
    now: Date.now(),
  });
  nudges = work.nudges;
  const next: Alert[] = [...work.alerts];
  const before = state.jobs;
  // A plan can't name who a Lead hands a card to: they join when they pick it up.
  state = cards.reduce((s, c) => joinJob(s, c.job, c.assigneeId), state);
  state = { ...state, jobs: state.jobs.map(measure) };
  const paused: Effect[] = [];
  for (const job of state.jobs) {
    const b = watchBudget(job);
    if (b.pause) {
      state = pauseJob(state, job.id);
      paused.push(pausedNote(job.id));
    } else if (b.alert) next.push(b.alert);
  }
  // watchBudget only looks at a running Job: a paused one keeps its stop alert until it is resumed.
  for (const job of state.jobs) {
    const stop = job.status === "paused" ? watchBudget({ ...job, status: "running" }).alert : null;
    if (stop?.level === "stop") next.push(stop);
  }
  const moved = state.jobs.filter((j, i) => j !== before[i]);
  const same = JSON.stringify(next) === JSON.stringify(alerts);
  alerts = next;
  if (remember() || moved.length) await tryPersist(moved);
  else if (!same) changed();
  await run([...work.effects, ...paused]);
}

export const companyAlerts = (): Alert[] => [...standingAlerts(), ...alerts];
export const dismissAlert = clearAlert;

/** The owner has merged what a Job made: close it. */
export const finishJob = (id: string): Promise<void> => enqueue(async () => {
  if (!state) return;
  const next = closeJob(state, id);
  if (next === state) return;
  state = next;
  await tryPersist(state.jobs.filter((j) => j.id === id));
});

/** The owner ends a Job that is still going, for good. */
export const stopJob = (id: string): Promise<void> => enqueue(async () => {
  if (!state) return;
  const next = haltJob(state, id);
  if (next === state) return;
  state = next;
  alerts = alerts.filter((a) => a.job !== id); // the watch would only drop them on its next round
  await tryPersist(state.jobs.filter((j) => j.id === id));
  await run([stoppedNote(id)]);
});

/** Give a paused Job more tokens and let it carry on. */
export const raiseCap = (id: string, extra: number): Promise<void> => enqueue(async () => {
  if (!state || !Number.isFinite(extra) || extra <= 0) return;
  const next = resumeJob(state, id, extra);
  if (next === state) return;
  state = next;
  await tryPersist(state.jobs.filter((j) => j.id === id));
  await run(resumedNotes(state, id, (person) => !!paneOf(person)?.pane.running));
});
