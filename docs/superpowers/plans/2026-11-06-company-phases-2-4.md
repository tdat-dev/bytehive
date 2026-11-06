# ByteHive Company, Phases 2 to 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the 3D office as the Company's home screen (0.15.0), then character editing and real Codex and OpenCode roles (0.16.0), then remove what the Company replaced (0.17.0).

**Architecture:** A pure view model (`officeModel`, `officeState`, navigation, tower layout) turns company state into a plain object. A DOM shell (breadcrumb, Needs you rail, job box, tags) and a three.js scene both read only that object, through an `OfficeSource` interface with a live and a demo implementation. The real terminal never moves: at Desk the office layer narrows to a left column and the Inbox stage shows through on the right. three.js is imported dynamically and only after the Company has been used.

**Tech Stack:** TypeScript, Vite 8, vitest (happy-dom), Tauri 2, xterm.js, three 0.170.0 (pinned exact), `@modelcontextprotocol/sdk`, zod. No UI framework.

**Spec:** `docs/superpowers/specs/2026-11-05-company-office-design.md`. **Measurements so far:** `docs/superpowers/specs/2026-11-05-phase0-results.md`. **Approved look:** `.superpowers/brainstorm/921-1793861341/content/style-a.webp`, `floors-link-v2.html`.

## Global Constraints

- Never write the character U+2014 (long dash) anywhere: code, comments, UI copy, commit messages, docs.
- No `Co-Authored-By` line in any commit. Conventional Commits, one commit per task unless a task says otherwise.
- UI strings are English and plain. Agent-to-agent messages are English.
- Keep each edited file's line endings. Never run `cargo fmt`.
- Do not stage `.superpowers/`, `.playwright-mcp/` or `8bb623.rbf`. Stage files by name, never `git add .`.
- Run tests with `rtk proxy npx vitest run <path>`. Other shell commands go through `rtk`. From `mcp/` the same, with `rtk proxy npx vitest run src/<file>`.
- People who never use the Company see no change. Phase 4 is the one stated exception, and Task 18 says exactly what it costs them.
- Never kill a process you did not start. Stop only what this task started, by PID.
- Another agent works on `feat/remote` in `D:/.maestro-worktrees/bytehive-remote`. Do not touch that worktree or branch. Both of you share the Cargo target folder and port 17630, so check the port before `tauri:dev` (see Task 1) and wait rather than fight for it.
- `~/.bytehive/company` holds the owner's real data. Task 1 makes dev builds use `~/.bytehive/company-dev`. Nothing in this plan writes to the real folder, and each real run proves it with the hash check in Task 1.
- `maestro.*` localStorage keys keep their names. `com.maestro.app` stays.
- HTML5 drag and drop does not work in WebView2. Any drag uses Pointer Events.
- The honey cell logo (`design/logo.svg`) is never replaced or recoloured.
- Every task that builds or changes UI starts by invoking the `build-premium-website` skill (route `DASHBOARD_PRODUCT`), and is judged from real screenshots, not from the code.

## Decisions taken for the owner

Each one is final for this plan. The task named carries it out.

| # | Decision | Why | Task |
|---|---|---|---|
| D1 | "Used the Company" means at least one Job exists. The Founder and floors do not count: `emptyCompany()` has a Founder for everyone and every open project becomes a floor at start. | Anything weaker turns the office on for people who never touched it. | 4 |
| D2 | The office is a layer over the app, not a new page. At Desk it narrows to a left column and the real stage shows on the right. No xterm element is ever moved. | Moving terminals is the fragile part of the Grid. This needs CSS only. | 8 |
| D3 | The Inbox stays reachable by an Inbox button in the office bar and by Ctrl+Shift+O, which toggles Office and Inbox. The house icon in the Inbox goes to the office once the Company has been used. Ctrl+K works everywhere. | One key and one visible button, no new rail. | 9 |
| D4 | `officeModel` keeps the four named arguments and takes one optional fifth, `extra`, for roles, board counts, open folders and flights. | Shirt by department, wall boards and hand-offs need data the four do not carry. Every four-argument call still works. | 4 |
| D5 | Shirts are worked out from the role's department at draw time unless the look was edited (`Look.edited`). No stored data changes. | Today's role files give Developer and QA different shirts, and existing people would keep them. A migration would write the owner's file for no gain. | 4 |
| D6 | All text is DOM on solid chips: floor tags, name tags, rail. The 3D scene carries no text except the logo. | Text stays readable at any size, passes contrast, and gives keyboard users real buttons. | 8 |
| D7 | Working people animate at 8 frames a second, not 30. Camera and paper plane use 30. | A busy office would otherwise never stop drawing. The gate measures this. | 6 |
| D8 | The office needs a real GPU. Software WebGL (SwiftShader, Microsoft Basic Render) counts as missing, except on the dev demo page. | A software renderer would load every CPU core on a machine that also runs agents. | 6 |
| D9 | Send back gets its own note box inside the plan row, because the rail is at every level and the job box only at Building. | The Phase 1 trick of reading the job box does not survive the move. | 5 |
| D10 | Dev builds default to `~/.bytehive/company-dev`. The lock stays as the second guard. | A dev app started while the installed app is closed would otherwise take the real lock and rewrite `company.json`. | 1 |
| D11 | Founder and Lead always run on Claude Code, in Phase 3 too. | Only Claude can deny file tools per agent, and spec section 4 rests on that. | 14 |
| D12 | A Hire may name a CLI (`Hire.cli`). The role's `cli` is the default. | "Hire a Codex developer" must work without editing a file. | 14 |
| D13 | A reused person gets a new branch by `git switch -c` in their existing worktree, not a new worktree. Only when their earlier Jobs are closed and the tree is clean. | Review is built on one worktree per pane. Otherwise they stack and the owner is told. | 16 |
| D14 | Tokens are charged once, to one Job, from one ledger per person. | The present per-Job base counts a person on two Jobs twice and misses a hire's start-up. | 16 |
| D15 | Phase 4 removes Director, and the Conductor modes. Saved presets and Scheduled agents stay, although spec section 9 says Crew presets are replaced. | The crew picker is already gone from the UI. What is left under that name is a timer feature that the role catalog does not replace, and removing it breaks people who never used the Company. | 18, 19 |

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `tools/company_preflight.mjs` | Port, lock and data-hash checks before any dev run | 1 |
| `src/company/queue.ts` | Pure deadline for an engine operation | 2 |
| `src/company/office/model.ts` | Pure `officeState`, `officeModel`, shirts, hair, `sceneKey` | 4 |
| `src/company/office/nav.ts` | Pure levels, breadcrumb, Esc rule, row targets, home view, frame pacing | 4 |
| `src/company/office/layout.ts` | Pure tower geometry, seats, camera shots, flight path | 4 |
| `src/company/office/palette.ts` | Office colours and contrast check | 4 |
| `src/company/office/report.ts` | Pure Report tab content | 4 |
| `src/company/office/source.ts` | `OfficeSource` interface, types only | 5 |
| `src/company/office/live.ts` | The engine as an `OfficeSource` | 5 |
| `src/company/office/rail.ts` | Needs you rail, Jobs list, job box (DOM) | 5 |
| `src/company/office/office.css` | All office styles | 5 |
| `src/company/office/fixtures.ts`, `demo.ts`, `office-demo.html` | Dev only demo | 6 |
| `src/company/office/scene.ts`, `build.ts` | three.js: renderer, loop, camera, pick; building meshes | 6 |
| `src/company/office/people.ts`, `textures.ts` | Characters, props, bubble, plane, logo and badge textures | 7 |
| `src/company/office/shell.ts` | Layer, breadcrumb, tags, message box, Desk tabs | 8 |
| `src/company/office/office.ts` | Public API: show, hide, toggle, fallback, keys | 9 |
| `tools/office_shots.mjs` | Screenshots of the demo over CDP | 6 |
| `tools/office_gate.mjs` | WebView2 measurements over CDP | 10 |
| `src/company/office/lookeditor.ts` | Small look editor at Desk | 13 |
| `src/company/look.ts` | What a character can wear, shared by the editor and the rules | 13 |
| `src/company/cli.ts` | Pure: which CLIs can hold a role, readiness, session args | 14 |
| `src/company/branch.ts` | Pure: what happens to a reused person's branch | 16 |
| `src/company/ledger.ts` | Pure: who spent what, one figure per person | 16 |

Deleted: `src/company/panel.ts`, `panel.css`, `panel.test.ts` (Task 9); `src/conductor.ts`, `src/pipeline.ts` and tests (Task 18); `orchestrator/`, `tests/`, `pyproject.toml`, `src/assets/sprites/`, `tools/sprite_*.py` (Task 19).

---

## Phase 2: The 3D office (0.15.0)

### Task 1: Dev sandbox for the Company

Today a dev build reads `~/.bytehive/company`. If the installed app is closed, a dev app takes the lock and saves `company.json`. This task removes that risk before anything else runs.

**How later tasks use this.** Three ways to see the app, in this order of preference:

1. **Demo page** (Task 6): no Tauri, no IPC, no company at all. All look work uses it.
2. **Dev app with the demo source** (`VITE_OFFICE_DEMO=1`, Task 9): real WebView2, real terminals, fixture company, `initCompany` never runs. The gate uses it.
3. **Dev app with a real engine on the sandbox** (`npm run tauri:dev`, root `~/.bytehive/company-dev`): real Jobs and real agents, on scratch projects only. The end to end tasks use it.

The installed app and a dev app on the sandbox have different roots, so different locks, and they do not see each other. The lock rule still matters if a build ever points at the real root: then it stays out as long as the installed app's heartbeat is fresh, reads no requests and writes nothing. To check that behaviour without the real folder, a task writes a fresh lock with a living foreign pid into the sandbox root before starting the dev app (`--hold-lock`, below) and sees "The Company is running in another ByteHive window."

**Files:**
- Modify: `src/company/store.ts`, `src/company/engine.ts:540-553`, `src/company/engine.ts:169-193`, `mcp/src/company.ts:13`
- Create: `tools/company_preflight.mjs`
- Test: `src/company/store.test.ts`, `mcp/src/company.test.ts`

**Interfaces:**
- Produces: `pickRoot(home: string, o: { dev: boolean; root?: string }): string` in `store.ts`.
- Produces: env `BYTEHIVE_COMPANY_ROOT`, set on members when the root is not the default, read by the MCP server.
- Produces: `node tools/company_preflight.mjs [--hash] [--hold-lock <root>]`.

- [ ] **Step 1: Write the failing tests**

Add to `src/company/store.test.ts`:

```ts
import { pickRoot } from "./store";

describe("pickRoot", () => {
  it("is the real folder in a release build", () => {
    expect(pickRoot("C:\\Users\\me\\", { dev: false })).toBe("C:\\Users\\me\\.bytehive\\company");
  });
  it("ignores an override in a release build", () => {
    expect(pickRoot("C:\\Users\\me", { dev: false, root: "D:\\x" })).toBe("C:\\Users\\me\\.bytehive\\company");
  });
  it("is the sandbox in a dev build", () => {
    expect(pickRoot("C:\\Users\\me", { dev: true })).toBe("C:\\Users\\me\\.bytehive\\company-dev");
  });
  it("takes an override in a dev build, without a trailing slash", () => {
    expect(pickRoot("C:\\Users\\me", { dev: true, root: "D:\\scratch\\co\\" })).toBe("D:\\scratch\\co");
  });
});
```

Add to `mcp/src/company.test.ts`:

```ts
describe("companyRoot", () => {
  afterEach(() => { delete process.env.BYTEHIVE_COMPANY_ROOT; });
  it("is the home folder's company by default", () => {
    expect(companyRoot("C:\\h")).toBe(path.join("C:\\h", ".bytehive", "company"));
  });
  it("follows the app when it runs a sandbox", () => {
    process.env.BYTEHIVE_COMPANY_ROOT = "D:\\sandbox";
    expect(companyRoot("C:\\h")).toBe("D:\\sandbox");
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/store.test.ts`
Expected: FAIL, `pickRoot` is not exported.

- [ ] **Step 3: Implement**

In `src/company/store.ts`, under `companyRoot`:

```ts
/** Where this build keeps the company. A dev build never touches the owner's real
 *  one unless it is told to, and only a dev build can be told. */
export function pickRoot(home: string, o: { dev: boolean; root?: string }): string {
  if (!o.dev) return companyRoot(home);
  const set = o.root?.trim().replace(/[\\/]+$/, "");
  return set || `${companyRoot(home)}-dev`;
}
```

In `src/company/engine.ts`, `initCompany`:

```ts
export async function initCompany(): Promise<void> {
  const home = await homeDir();
  // vitest also runs with DEV set, and its tests expect the plain root.
  const dev = import.meta.env.DEV && import.meta.env.MODE !== "test";
  root = pickRoot(home, { dev, root: import.meta.env.VITE_COMPANY_ROOT });
  sandboxed = root !== companyRoot(home);
  await fsCreateDir(home, ".bytehive").catch(() => {});
  const cut = root.lastIndexOf("\\");
  await fsCreateDir(root.slice(0, cut), root.slice(cut + 1)).catch(() => {});
  me = await appPid();
```

Add `let sandboxed = false;` next to `let root`. In `personSpec`, change the env line to:

```ts
    env: sandboxed ? [...launch.env, ["BYTEHIVE_COMPANY_ROOT", root]] : launch.env,
```

In `mcp/src/company.ts`:

```ts
export const companyRoot = (home = os.homedir()) => process.env.BYTEHIVE_COMPANY_ROOT?.trim() || path.join(home, ".bytehive", "company");
```

The existing engine test `fsCreateDir` records `made`: if one asserts the old `.bytehive\\company` rel, update it to the new call (root `H:\.bytehive`, rel `company`).

- [ ] **Step 4: Write the preflight tool**

```js
// tools/company_preflight.mjs
/* Run before any dev app. Says where it would write and whether it is safe.
 *   node tools/company_preflight.mjs              port and lock report
 *   node tools/company_preflight.mjs --hash       hash of the owner's company files
 *   node tools/company_preflight.mjs --hold-lock <root>   a fresh lock held by this process, for 5 minutes
 */
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const real = path.join(os.homedir(), ".bytehive", "company");
const arg = process.argv[2];

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) return e.name === "people" ? [] : walk(p);
    return e.name === "engine.lock" ? [] : [p];
  });
}

if (arg === "--hash") {
  const h = createHash("sha256");
  for (const f of walk(real).sort()) { h.update(f); h.update(fs.readFileSync(f)); }
  console.log(h.digest("hex"));
  process.exit(0);
}

if (arg === "--hold-lock") {
  const root = process.argv[3];
  if (!root || path.resolve(root) === path.resolve(real)) { console.error("refused: a sandbox root only"); process.exit(1); }
  fs.mkdirSync(root, { recursive: true });
  const beat = () => fs.writeFileSync(path.join(root, "engine.lock"), JSON.stringify({ pid: process.pid, at: Date.now() }));
  beat();
  const t = setInterval(beat, 1500);
  setTimeout(() => { clearInterval(t); process.exit(0); }, 300_000);
} else {
  const lock = (() => { try { return JSON.parse(fs.readFileSync(path.join(real, "engine.lock"), "utf8")); } catch { return null; } })();
  const age = lock ? Math.round((Date.now() - lock.at) / 1000) : null;
  console.log(`real company lock: ${lock ? `pid ${lock.pid}, ${age}s old` : "none"}`);
  const netstat = execFileSync("netstat", ["-ano", "-p", "TCP"], { encoding: "utf8" });
  const held = netstat.split("\n").find((l) => /:17630\s/.test(l) && /LISTENING/.test(l));
  console.log(held ? `port 17630 is taken: ${held.trim()}. Wait for it, never kill it.` : "port 17630 is free");
  process.exit(held ? 1 : 0);
}
```

The hash leaves out `engine.lock` (the installed app rewrites it every 1.5 s) and `people/` (it refreshes prompts at its own start). Keep the real app's own writes in mind: a hash that changes while the owner is using the installed app means nothing, so compare hashes only when the installed app's Jobs are idle, or compare `company.json` alone.

- [ ] **Step 5: Run the tests**

Run: `rtk proxy npx vitest run src/company` and, from `mcp/`, `rtk proxy npx vitest run src/company.test.ts`
Expected: PASS.

- [ ] **Step 6: See it work**

```bash
rtk node tools/company_preflight.mjs --hash   # note the value
rtk node tools/company_preflight.mjs          # port must be free
rtk npm run build:mcp
rtk npm run tauri:dev
```

Expected: the window title is ByteHive Dev, `~/.bytehive/company-dev/company.json` appears, and the hash is unchanged after you stop the app you started, by its PID.

- [ ] **Step 7: Commit**

```bash
rtk git add src/company/store.ts src/company/store.test.ts src/company/engine.ts mcp/src/company.ts mcp/src/company.test.ts tools/company_preflight.mjs
rtk git commit -m "feat: dev builds keep their own company"
```

### Task 2: Engine fixes the office leans on

Backlog items closed here: queue timeout, Stop ends the running turn, a stopped Job's cards marked, To do cards watched, Inbox header counts Company rows. Also from `task-9-report.md` item 3: `fleet_send` no longer claims a member's message was sent when it was only queued.

The spec's rule that a pause never cuts a running turn stays. Only the owner's Stop does.

**Files:**
- Create: `src/company/queue.ts`, `src/company/queue.test.ts`
- Modify: `src/company/engine.ts`, `src/company/reduce.ts`, `src/company/watch.ts`, `src/company/org.ts`, `src/board.ts`, `mcp/src/board.ts`, `mcp/src/company.ts`, `src/kanban.ts:899-906`, `src/inbox.ts:113`, `src/inbox.ts:733-739`, `src/inbox.ts:1274`
- Test: `src/company/engine.test.ts`, `src/company/reduce.test.ts`, `src/company/watch.test.ts`, `src/company/org.test.ts`, `src/board.test.ts`, `src/inbox.test.ts`, `src/inboxstage.test.ts`, `mcp/src/company.test.ts`

**Interfaces:**
- Produces, `queue.ts`: `OP_LIMIT_MS = 300_000`, `class OpTimeout extends Error`, `withLimit<T>(op: () => Promise<T>, ms: number, label: string): Promise<T>`.
- Produces, `engine.ts`: `enqueue<T>(op, label?: string)`, `companyNeeds(): number`, `onCompanyChange(fn): () => void` (now gives back an unsubscribe).
- Produces, `reduce.ts`: `stopTurns(s: State, id: string): string[]`, `typeable(text: string): string` (exported).
- Produces, `watch.ts`: `cardsToWatch(i): { cards: WatchedCard[]; seen: string[] }`, `WatchedCard.list?: "doing" | "todo"`.
- Produces, `org.ts`: `floorContact(c: Company, floor: string): Person | null`.
- Produces, `board.ts`: `Card.stopped?: boolean`, `markJobStopped(board: Board, job: string): number`.
- Produces, `inbox.ts`: `headline(list: Task[], company?: number)`.

- [ ] **Step 1: Write the failing tests**

```ts
// src/company/queue.test.ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { OpTimeout, withLimit } from "./queue";

describe("withLimit", () => {
  afterEach(() => { vi.useRealTimers(); });
  it("passes a result through", async () => {
    await expect(withLimit(async () => 7, 1000, "x")).resolves.toBe(7);
  });
  it("passes a failure through", async () => {
    await expect(withLimit(async () => { throw new Error("no"); }, 1000, "x")).rejects.toThrow("no");
  });
  it("gives up at the limit and says what it gave up on", async () => {
    vi.useFakeTimers();
    const p = withLimit(() => new Promise<void>(() => {}), 2000, "Starting a plan");
    const seen = expect(p).rejects.toThrow(OpTimeout);
    await vi.advanceTimersByTimeAsync(2000);
    await seen;
    await expect(p).rejects.toThrow("Starting a plan took over 2 s, so ByteHive moved on. It may still finish.");
  });
});
```

Add to `src/company/reduce.test.ts`:

```ts
import { stopTurns, type State } from "./reduce";

describe("stopTurns", () => {
  const j = (id: string, status: Job["status"], people: string[]): Job =>
    ({ id, title: id, ask: id, status, plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people, createdAt: 1 });
  const s = (jobs: Job[]): State => ({ company: emptyCompany(), jobs, roles: [] });

  it("is everyone on the Job and the Founder when nothing else is going", () => {
    expect(stopTurns(s([j("J1", "stopped", ["a", "b"])]), "J1").sort()).toEqual(["a", "b", "founder"]);
  });
  it("leaves out someone working another Job, and the Founder", () => {
    expect(stopTurns(s([j("J1", "stopped", ["a", "b"]), j("J2", "running", ["b"])]), "J1")).toEqual(["a"]);
  });
  it("counts a finished Job as not going", () => {
    expect(stopTurns(s([j("J1", "stopped", ["a"]), j("J2", "done", ["a"])]), "J1").sort()).toEqual(["a", "founder"]);
  });
  it("is nobody for an unknown Job", () => {
    expect(stopTurns(s([]), "J9")).toEqual([]);
  });
});
```

Add to `src/company/org.test.ts`:

```ts
describe("floorContact", () => {
  it("is the Lead when there is one, else the first person, else nobody", () => {
    const lead = { id: "l", rank: "lead", floor: "a" };
    const staff = { id: "s", rank: "staff", floor: "a" };
    const c = { ...emptyCompany(), people: [staff, lead] } as unknown as Company;
    expect(floorContact(c, "a")?.id).toBe("l");
    expect(floorContact({ ...c, people: [staff] } as unknown as Company, "a")?.id).toBe("s");
    expect(floorContact(c, "b")).toBeNull();
  });
});
```

Add to `src/company/watch.test.ts`:

```ts
import { cardsToWatch, watchWork } from "./watch";

describe("cardsToWatch", () => {
  const running = [{ id: "J1", status: "running" }] as Job[];
  const card = (id: string, over: object = {}) => ({ id, title: id, job: "J1", ...over }) as Card;
  const board = (todo: Card[], doing: Card[] = []) => ({ lists: [{ title: "To do", cards: todo }, { title: "Doing", cards: doing }] });
  const base = { jobs: running, contact: "lead", isWorking: () => false, now: 100 };

  it("watches a Job's To do card through the floor's contact", () => {
    const quiet = new Map<string, number>();
    const r = cardsToWatch({ ...base, quiet, board: board([card("c1")]) });
    expect(r.cards).toEqual([{ id: "c1", title: "c1", job: "J1", assigneeId: "lead", quietSince: 100, returns: 0, list: "todo" }]);
  });
  it("uses the person a To do card names", () => {
    const r = cardsToWatch({ ...base, quiet: new Map(), board: board([card("c1", { assigneeId: "ana" })]) });
    expect(r.cards[0].assigneeId).toBe("ana");
  });
  it("leaves To do alone while a card of the same Job is in Doing", () => {
    const quiet = new Map([["todo:c1", 5]]);
    const r = cardsToWatch({ ...base, quiet, board: board([card("c1")], [card("c2", { assigneeId: "ana" })]) });
    expect(r.cards.map((c) => c.id)).toEqual(["c2"]);
    expect(quiet.has("todo:c1")).toBe(false);
  });
  it("starts a card's quiet time over when it moves to Doing", () => {
    const quiet = new Map([["todo:c1", 5]]);
    const r = cardsToWatch({ ...base, now: 200, quiet, board: board([], [card("c1", { assigneeId: "ana" })]) });
    expect(r.cards[0].quietSince).toBe(200);
  });
  it("clears a Doing card's quiet time while its person works", () => {
    const quiet = new Map([["doing:c1", 5]]);
    const r = cardsToWatch({ ...base, isWorking: () => true, quiet, board: board([], [card("c1", { assigneeId: "ana" })]) });
    expect(r.cards[0].quietSince).toBeNull();
  });
  it("ignores a paused Job, cards with no Job, and a To do card with nobody to ask", () => {
    const quiet = new Map<string, number>();
    const paused = [{ id: "J1", status: "paused" }] as Job[];
    expect(cardsToWatch({ ...base, jobs: paused, quiet, board: board([card("c1")]) }).cards).toEqual([]);
    expect(cardsToWatch({ ...base, quiet, board: board([card("c1", { job: undefined })]) }).cards).toEqual([]);
    expect(cardsToWatch({ ...base, contact: null, quiet, board: board([card("c1")]) }).cards).toEqual([]);
  });
  it("names every card it read, for clearing old quiet times", () => {
    expect(cardsToWatch({ ...base, quiet: new Map(), board: board([card("c1")], [card("c2")]) }).seen).toEqual(["doing:c2", "todo:c1"]);
  });
});

describe("watchWork on a To do card", () => {
  const limits = { stallMinutes: 10, maxReturns: 3 } as Limits;
  const c = { id: "c1", title: "Fix it", job: "J1", assigneeId: "lead", quietSince: 0, returns: 0, list: "todo" as const };
  it("nudges the boss in its own words, then raises a row", () => {
    const first = watchWork({ cards: [c], nudges: new Map(), limits, bossOf: () => "founder", now: 600_000 });
    expect(first.effects[0]).toEqual({ kind: "deliver", personId: "founder", text: '"Fix it" has waited in To do for 10 minutes. Hand it out or start it.' });
    const second = watchWork({ cards: [c], nudges: first.nudges, limits, bossOf: () => "founder", now: 1_200_000 });
    expect(second.alerts[0].text).toBe('"Fix it" has not been picked up.');
  });
});
```

Add to `src/board.test.ts`:

```ts
import { markJobStopped, normalizeCard, type Board } from "./board";

describe("markJobStopped", () => {
  const c = (id: string, job?: string) => ({ id, title: id, desc: "", labels: [], due: null, checklist: [], job });
  const board = (): Board => ({ lists: [{ id: "a", title: "To do", cards: [c("1", "J1"), c("2", "J2")] }, { id: "b", title: "Done", cards: [c("3", "J1")] }] });
  it("marks the Job's open cards and counts them, leaving Done and other Jobs", () => {
    const b = board();
    expect(markJobStopped(b, "J1")).toBe(1);
    expect(b.lists[0].cards.map((x) => !!x.stopped)).toEqual([true, false]);
    expect(b.lists[1].cards[0].stopped).toBeUndefined();
  });
  it("counts nothing the second time, so a board with no change is not written", () => {
    const b = board();
    markJobStopped(b, "J1");
    expect(markJobStopped(b, "J1")).toBe(0);
  });
  it("survives a read of the file", () => {
    const b = board();
    markJobStopped(b, "J1");
    expect(normalizeCard(JSON.parse(JSON.stringify(b.lists[0].cards[0]))).stopped).toBe(true);
  });
});
```

Add to `src/inbox.test.ts`, in the `headline` describe:

```ts
it("counts what the Company needs from you", () => {
  expect(headline([], 2)).toEqual({ lead: "2 Company items need you.", rest: "" });
  expect(headline([], 1).lead).toBe("1 Company item needs you.");
  expect(headline([task("needs")], 1).lead).toBe("1 agent and 1 Company item need you.");
  expect(headline([task("working")], 1)).toEqual({ lead: "1 Company item needs you.", rest: " 1 is working." });
});
it("says what it always said with nothing from the Company", () => {
  expect(headline([]).lead).toBe("No agents yet.");
  expect(headline([], 0).lead).toBe("No agents yet.");
});
```

`task` is the helper the file already uses for a `Task` in a state: use its real name.

Add to `src/company/engine.test.ts`. First add `keys: [] as Array<[string, string]>,` to the hoisted `io`, `sendInput: async (id: string, text: string) => { io.keys.push([id, text]); },` to the `../ipc` mock, and `io.keys = [];` to `beforeEach`.

```ts
const member = () => [...workspaces.values()].flatMap((w) => [...w.panes.values()]).find((p) => p.spec.personId && p.spec.personId !== "founder")!;

describe("the queue", () => {
  it("moves on from an operation that hangs, and tells the owner", async () => {
    void engine.enqueue(() => new Promise(() => {}), "Starting a plan").catch(() => {});
    const next = engine.enqueue(async () => "ran");
    await vi.advanceTimersByTimeAsync(300_000);
    await expect(next).resolves.toBe("ran");
    expect(engine.companyAlerts().map((a) => a.text).join(" ")).toContain("Starting a plan took over 300 s");
  });
});

describe("Stop", () => {
  it("ends the turn of someone working", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    const pane = member();
    pane.lastOutputAt = Date.now();
    await engine.stopJob(job().id);
    expect(io.keys).toContainEqual([pane.id, "\u001b"]);
  });
  it("sends no key to someone quiet, who may be at a prompt", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    const pane = member();
    pane.lastOutputAt = Date.now() - 60_000;
    await engine.stopJob(job().id);
    expect(io.keys.filter(([id]) => id === pane.id)).toEqual([]);
  });
  it("does not end a turn on a pause", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    const pane = member();
    pane.lastOutputAt = Date.now();
    job().tokenCap = 1;
    job().tokensUsed = 5;
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().status).toBe("paused");
    expect(io.keys).toEqual([]);
  });
  it("marks the Job's cards on the boards", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    await engine.stopJob(job().id);
    expect(cards("D:\\a").every((c) => c.stopped)).toBe(true);
  });
  it("still stops when a board cannot be written", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    io.writeFails = [{ Failed: "disk full" }];
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    await engine.stopJob(job().id);
    expect(job().status).toBe("stopped");
    spy.mockRestore();
  });
});

describe("the watch and To do", () => {
  it("nudges the boss about a card nobody picked up", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    io.sent.length = 0;
    io.told.length = 0;
    await vi.advanceTimersByTimeAsync(11 * 60_000);
    const said = [...io.sent.map(([, t]) => t), ...io.told.map((t) => t.text)].join(" ");
    expect(said).toContain("has waited in To do");
  });
});
```

In `mcp/src/company.test.ts:41` change the expected text to `"queued for Hai. If a job holds it, you will hear back."`.

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company src/board.test.ts src/inbox.test.ts`
Expected: FAIL on the new cases.

- [ ] **Step 3: Implement the queue deadline**

```ts
// src/company/queue.ts
/* A hung operation must not stop the company for good: git worktree add has no
 * timeout of its own, and everything else waits behind it. */

/** Starting a plan with five hires and a slow git can take minutes. */
export const OP_LIMIT_MS = 300_000;

export class OpTimeout extends Error {
  constructor(label: string, ms: number) {
    super(`${label} took over ${Math.round(ms / 1000)} s, so ByteHive moved on. It may still finish.`);
  }
}

/** The operation goes on running after a timeout: there is no cancelling a backend
 *  call. Only the queue stops waiting for it. */
export function withLimit<T>(op: () => Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new OpTimeout(label, ms)), ms);
    void Promise.resolve().then(op).then(resolve, reject).finally(() => clearTimeout(timer));
  });
}
```

In `src/company/engine.ts`, replace `enqueue`:

```ts
export function enqueue<T>(op: () => Promise<T>, label = "A company operation"): Promise<T> {
  const done = queue.then(() => withLimit(op, OP_LIMIT_MS, label));
  queue = done.catch((e) => { if (e instanceof OpTimeout) raiseAlert(`late-${label}`, e.message); });
  return done;
}
```

Pass labels at the slow call sites: `giveJob` gets `"Giving a job"`, `approve` gets `"Starting a plan"`, `start` gets `"Loading the company"`. Keep the comment that an operation must not enqueue another and wait for it, and add one line: an abandoned operation keeps running, which is why the owner is told.

- [ ] **Step 4: Implement Stop and the marks**

In `src/company/reduce.ts`, export `typeable` and add after `haltJob`:

```ts
/** Whose running turn a Stop ends: people on this Job with no other Job going. The
 *  Founder works every Job, so it counts as long as no other is going. */
export function stopTurns(s: State, id: string): string[] {
  const job = s.jobs.find((j) => j.id === id);
  if (!job) return [];
  const others = s.jobs.filter((j) => j.id !== id && GOING.includes(j.status));
  const busy = new Set(others.flatMap((j) => j.people));
  const people = job.people.filter((p) => !busy.has(p));
  return others.length ? people.filter((p) => p !== FOUNDER_ID) : [...new Set([...people, FOUNDER_ID])];
}
```

In `src/board.ts` add `stopped?: boolean; // its Job was stopped: nobody works it any more` to `Card`, `if (c.stopped === true) card.stopped = true;` beside line 84, and:

```ts
/** Mark a stopped Job's open cards. Gives back how many changed. */
export function markJobStopped(board: Board, job: string): number {
  let n = 0;
  for (const l of board.lists) {
    if (l.title.trim().toLowerCase() === "done") continue;
    for (const c of l.cards) if (c.job === job && !c.stopped) { c.stopped = true; n += 1; }
  }
  return n;
}
```

Add the same `Card` field and normalize line to `mcp/src/board.ts`, or an MCP write drops the mark.

In `src/kanban.ts` beside the assignee chip on line 900, add a `Stopped` chip when `card.stopped` (class `kb-stopped`, plain text, no icon), and to `src/styles/dock.css` a muted chip style.

In `src/company/engine.ts`, take the retry out of `placeOnFloor`:

```ts
/** Change a floor's board. An agent writing it at the same moment is a Conflict:
 *  read its copy and try again. `change` gives back false when there is nothing to write. */
async function changeBoard(dir: string, change: (b: Board) => boolean): Promise<void> {
  for (let tries = 1; ; tries += 1) {
    const file = await readBoardFile(dir);
    const board = file?.board ?? defaultBoard();
    if (!change(board)) return;
    try {
      await writeBoardFile(dir, board, file?.mtime ?? null);
      return;
    } catch (e) {
      if ((e as { Conflict?: number })?.Conflict === undefined || tries >= BOARD_TRIES) throw e;
    }
  }
}
```

`placeOnFloor` becomes a call to it whose callback does the To do list work and returns `true`. Then:

```ts
/** End a person's turn, if they are in one. Quiet means they are at a prompt or a
 *  question, where an Esc would only clear what the owner typed. */
async function interrupt(personId: string): Promise<void> {
  const pane = paneOf(personId)?.pane;
  if (pane?.running && Date.now() - pane.lastOutputAt <= WORKING_MS) await sendInput(pane.id, "\u001b").catch(() => {});
}

export const stopJob = (id: string): Promise<void> => enqueue(async () => {
  if (!state) return;
  const next = haltJob(state, id);
  if (next === state) return;
  state = next;
  alerts = alerts.filter((a) => a.job !== id); // the watch would only drop them on its next round
  await tryPersist(state.jobs.filter((j) => j.id === id));
  for (const p of stopTurns(state, id)) await interrupt(p);
  for (const f of floorsWithPeople()) {
    await changeBoard(f.dir, (b) => markJobStopped(b, id) > 0)
      .catch((e) => console.error(`[bytehive] ${id}'s cards on ${f.id} were not marked: ${reason(e)}`));
  }
  await run([stoppedNote(id)]);
}, "Stopping a job");
```

Add `const floorsWithPeople = () => state?.company.floors.filter((f) => state?.company.people.some((p) => p.floor === f.id)) ?? [];` and use it in `doingCards` too. Import `WORKING_MS` from `../taskstate`, `sendInput` from `../ipc`, `markJobStopped` and `type Board` from `../board`.

- [ ] **Step 5: Implement the To do watch**

In `src/company/org.ts`:

```ts
/** Who speaks for a floor: its Lead, else the one person there. */
export function floorContact(c: Company, floor: string): Person | null {
  const here = floorPeople(c, floor);
  return here.find((p) => p.rank === "lead") ?? here[0] ?? null;
}
```

Move `floorPeople` above it if needed. In `src/company/watch.ts` add `list?: "doing" | "todo";` to `WatchedCard`, and:

```ts
const cardsOf = (board: WatchBoard, title: string) =>
  board.lists.find((l) => l.title.trim().toLowerCase() === title)?.cards ?? [];

export interface WatchBoard { lists: Array<{ title: string; cards: Card[] }> }

/** The cards of running Jobs on one board that could go quiet. A To do card is lost
 *  when nobody has it: a Job with a card in Doing is moving, and its To do cards are
 *  only waiting their turn. Times are kept per list, so moving on starts them over. */
export function cardsToWatch(i: {
  board: WatchBoard;
  jobs: Job[];
  contact: string | null;
  isWorking: (personId: string) => boolean;
  quiet: Map<string, number>;
  now: number;
}): { cards: WatchedCard[]; seen: string[] } {
  const cards: WatchedCard[] = [];
  const seen: string[] = [];
  const running = (job: string | undefined) => !!job && i.jobs.find((j) => j.id === job)?.status === "running";
  const doing = cardsOf(i.board, "doing");
  for (const c of doing) {
    const key = `doing:${c.id}`;
    seen.push(key);
    if (!c.job || !c.assigneeId) continue;
    if (!running(c.job)) { i.quiet.delete(key); continue; }
    if (i.isWorking(c.assigneeId)) i.quiet.delete(key);
    else if (!i.quiet.has(key)) i.quiet.set(key, i.now);
    cards.push({ id: c.id, title: c.title, job: c.job, assigneeId: c.assigneeId, quietSince: i.quiet.get(key) ?? null, returns: c.returns ?? 0, list: "doing" });
  }
  for (const c of cardsOf(i.board, "to do")) {
    const key = `todo:${c.id}`;
    seen.push(key);
    const who = c.assigneeId ?? i.contact;
    if (!c.job || !who) continue;
    if (!running(c.job) || doing.some((d) => d.job === c.job)) { i.quiet.delete(key); continue; }
    if (!i.quiet.has(key)) i.quiet.set(key, i.now);
    cards.push({ id: c.id, title: c.title, job: c.job, assigneeId: who, quietSince: i.quiet.get(key) ?? null, returns: c.returns ?? 0, list: "todo" });
  }
  return { cards, seen };
}
```

Import `type { Card } from "../board"` in `watch.ts`. In `watchWork`, change the two texts:

```ts
const waiting = c.list === "todo";
const nudge = waiting ? `"${c.title}" has waited in To do for ${i.limits.stallMinutes} minutes. Hand it out or start it.` : `"${c.title}" has gone quiet for ${i.limits.stallMinutes} minutes. Check on it.`;
const stuck = waiting ? `"${c.title}" has not been picked up.` : `"${c.title}" is stuck.`;
```

In `src/company/engine.ts`, `doingCards` becomes:

```ts
/** The cards that could go quiet on every floor with people. */
async function watchedCards(): Promise<WatchedCard[]> {
  const out: WatchedCard[] = [];
  const seen = new Set<string>();
  let readAll = true;
  for (const floor of floorsWithPeople()) {
    const file = await readBoardFile(floor.dir).catch(() => { readAll = false; return null; });
    if (!file || !state) continue;
    const r = cardsToWatch({
      board: file.board, jobs: state.jobs, quiet, now: Date.now(),
      contact: floorContact(state.company, floor.id)?.id ?? null,
      isWorking: (id) => { const pane = paneOf(id)?.pane; return !!pane?.running && Date.now() - pane.lastOutputAt < 60_000; },
    });
    out.push(...r.cards);
    r.seen.forEach((k) => seen.add(k));
  }
  // A card that left its list starts its quiet time over when it comes back.
  if (readAll) for (const k of quiet.keys()) if (!seen.has(k)) quiet.delete(k);
  return out;
}
```

In `watch()`, only someone with a card in Doing joins a Job: `state = cards.filter((c) => c.list === "doing").reduce((s, c) => joinJob(s, c.job, c.assigneeId), state);`.

- [ ] **Step 6: Implement the header and the honest reply**

In `src/inbox.ts`, `headline`:

```ts
export function headline(list: Task[], company = 0): { lead: string; rest: string } {
  const n = (s: TaskState) => list.filter((t) => t.status.state === s).length;
  const needs = n("needs"), review = n("review"), working = n("working");
  const are = (k: number) => (k === 1 ? "is" : "are");
  if (!list.length && !company) return { lead: "No agents yet.", rest: " Start one with New agent." };
  const items = `${company} Company item${company === 1 ? "" : "s"}`;
  const lead =
    needs && company ? `${needs} agent${needs === 1 ? "" : "s"} and ${items} need you.`
    : needs ? `${needs} agent${needs === 1 ? " needs" : "s need"} you.`
    : company ? `${items} need${company === 1 ? "s" : ""} you.`
    : "Nothing needs you.";
```

The rest of the function stays. In `renderHead` call `headline(list, companyNeeds())`. In `mount`, after `onAgentCost(render);` add `onCompanyChange(render);`. Import both from `./company/engine`. Add `vi.mock("./company/engine", () => ({ companyNeeds: () => 0, onCompanyChange: () => () => {} }));` to `src/inbox.test.ts` and `src/inboxstage.test.ts`.

In `src/company/engine.ts`:

```ts
export const onCompanyChange = (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; };

/** How many rows Needs you has, for the Inbox header. */
export const companyNeeds = () => (state ? needsYou(state.jobs, companyAlerts(), { floors: state.company.floors, roles: state.roles }).length : 0);
```

In `mcp/src/company.ts`, `sendAsMember` ends with:

```ts
  return `queued for ${r.people.map((p) => p.name).join(", ")}. If a job holds it, you will hear back.`;
```

- [ ] **Step 7: Run all tests and the type check**

Run: `rtk proxy npx vitest run src/company src/board.test.ts src/inbox.test.ts src/inboxstage.test.ts src/kanban && rtk npx tsc --noEmit` and, from `mcp/`, `rtk proxy npx vitest run`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src/company src/board.ts src/board.test.ts src/kanban.ts src/styles/dock.css src/inbox.ts src/inbox.test.ts src/inboxstage.test.ts mcp/src/board.ts mcp/src/company.ts mcp/src/company.test.ts
rtk git commit -m "fix: a hung step, a stopped job and a lost card no longer pass in silence"
```

### Task 3: Card evidence and checklists

Backlog items closed here: `card_done` reads committed work on the branch, `card_update` can tick checklists. The Report tab in Task 8 and the wall board show these, so they ship with the office.

**Files:**
- Modify: `mcp/src/git.ts`, `mcp/src/ops.ts:107-121`, `mcp/src/server.ts:59-61`, `mcp/src/server.ts:150-165`, `mcp/src/server.ts:195-209`
- Test: `mcp/src/git.test.ts`, `mcp/src/ops.test.ts`

**Interfaces:**
- Produces: `branchFiles(runDir: string, boardDir: string): string[]`, `workFiles(runDir: string, boardDir: string): string[]`.
- Produces: `updateCard` patch gains `tick?: string[]`, `untick?: string[]`.

- [ ] **Step 1: Write the failing tests**

Add to `mcp/src/git.test.ts`:

```ts
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { branchFiles, workFiles } from "./git.js";

function repo(): { main: string; wt: string; git: (cwd: string, ...a: string[]) => void } {
  const main = mkdtempSync(join(tmpdir(), "bh-main-"));
  const git = (cwd: string, ...a: string[]) => { execFileSync("git", a, { cwd, stdio: "pipe" }); };
  git(main, "init", "-q", "-b", "main");
  git(main, "config", "user.email", "t@t.t");
  git(main, "config", "user.name", "t");
  writeFileSync(join(main, "a.txt"), "a");
  git(main, "add", ".");
  git(main, "commit", "-qm", "a");
  const wt = join(main, "..", `${basename(main)}-wt`);
  git(main, "worktree", "add", "-q", "-b", "ana", wt);
  return { main, wt, git };
}

describe("what a worker changed", () => {
  it("includes what they committed on their branch", () => {
    const { main, wt, git } = repo();
    writeFileSync(join(wt, "b.txt"), "b");
    git(wt, "add", ".");
    git(wt, "commit", "-qm", "b");
    expect(branchFiles(wt, main)).toEqual(["b.txt"]);
    expect(workFiles(wt, main)).toEqual(["b.txt"]);
  });
  it("adds what is not committed yet, once", () => {
    const { main, wt, git } = repo();
    writeFileSync(join(wt, "b.txt"), "b");
    git(wt, "add", ".");
    git(wt, "commit", "-qm", "b");
    writeFileSync(join(wt, "b.txt"), "changed");
    writeFileSync(join(wt, "c.txt"), "c");
    expect(workFiles(wt, main).sort()).toEqual(["b.txt", "c.txt"]);
  });
  it("leaves out what hooks write", () => {
    const { main, wt } = repo();
    mkdirSync(join(wt, ".claude"));
    writeFileSync(join(wt, ".claude", "x.json"), "{}");
    mkdirSync(join(wt, "logs"));
    writeFileSync(join(wt, "logs", "y.log"), "y");
    writeFileSync(join(wt, "real.txt"), "r");
    expect(workFiles(wt, main)).toEqual(["real.txt"]);
  });
  it("is what git status says for an agent in the project folder", () => {
    const { main } = repo();
    writeFileSync(join(main, "z.txt"), "z");
    expect(workFiles(main, main)).toEqual(["z.txt"]);
  });
  it("is nothing outside a repo", () => {
    const dir = mkdtempSync(join(tmpdir(), "bh-none-"));
    expect(workFiles(dir, dir)).toEqual([]);
  });
});
```

Add to `mcp/src/ops.test.ts`:

```ts
describe("ticking a checklist", () => {
  const board = () => {
    const b = { lists: [{ id: "l1", title: "Doing", cards: [] }] } as unknown as Board;
    addCard(b, "Doing", { title: "Card", checklist: ["Write it", "Test it"] });
    return b;
  };
  it("ticks by text, ignoring case, and by id", () => {
    const b = board();
    const id = b.lists[0].cards[0].checklist[1].id;
    const c = updateCard(b, "Card", { tick: ["write IT", id] });
    expect(c.checklist.map((i) => i.done)).toEqual([true, true]);
  });
  it("unticks", () => {
    const b = board();
    updateCard(b, "Card", { tick: ["Write it"] });
    expect(updateCard(b, "Card", { untick: ["Write it"] }).checklist[0].done).toBe(false);
  });
  it("names an item it cannot find", () => {
    expect(() => updateCard(board(), "Card", { tick: ["Ship it"] })).toThrow('No checklist item "Ship it" on "Card"');
  });
  it("ticks on the list a replacement just gave", () => {
    const c = updateCard(board(), "Card", { checklist: ["New"], tick: ["New"] });
    expect(c.checklist).toMatchObject([{ text: "New", done: true }]);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

From `mcp/`: `rtk proxy npx vitest run src/git.test.ts src/ops.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

In `mcp/src/git.ts`:

```ts
const run = (cwd: string, ...args: string[]) =>
  execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();

/** What this branch has committed since it left the project's checkout. People
 *  commit before card_done, so git status alone shows nothing of their work. */
export function branchFiles(runDir: string, boardDir: string): string[] {
  try {
    const base = run(boardDir, "rev-parse", "HEAD");
    const fork = run(runDir, "merge-base", "HEAD", base);
    return run(runDir, "diff", "--name-only", fork, "HEAD").split("\n").filter(Boolean);
  } catch {
    return [];
  }
}

/** What the owner's hooks and ByteHive write into a working folder. */
const NOISE = /^(\.claude|logs|\.bytehive)\//;

/** Everything a worker changed: committed and not yet committed. */
export function workFiles(runDir: string, boardDir: string): string[] {
  return [...new Set([...branchFiles(runDir, boardDir), ...changedFiles(runDir)])].filter((f) => !NOISE.test(f));
}
```

In `mcp/src/ops.ts`:

```ts
function setDone(card: Card, refs: string[], done: boolean): void {
  for (const ref of refs) {
    const item = card.checklist.find((i) => i.id === ref) ?? card.checklist.find((i) => norm(i.text) === norm(ref));
    if (!item) throw new BoardError(`No checklist item "${ref}" on "${card.title}": see board_get`);
    item.done = done;
  }
}

export function updateCard(board: Board, cardRef: string, patch: Partial<CardInput> & { tick?: string[]; untick?: string[] }): Card {
```

and at the end of `updateCard`, before `return card;`:

```ts
  setDone(card, patch.tick ?? [], true);
  setDone(card, patch.untick ?? [], false);
```

In `mcp/src/server.ts`, add to the `card_update` schema:

```ts
        tick: z.array(z.string()).optional().describe("Checklist items now finished, by id or exact text"),
        untick: z.array(z.string()).optional().describe("Checklist items to open again, by id or exact text"),
```

Change the `card_done` call to `files: workFiles(runDir, dir)`, its description to `"... attach evidence (summary + files changed on this branch, committed or not)."`, and `MEMBER_STEPS` to end: `with a one-line summary when it is finished. Tick checklist items with card_update as you finish them. Leave a card marked stopped: its job ended.`

- [ ] **Step 4: Run tests**

From `mcp/`: `rtk proxy npx vitest run`. From the root: `rtk npm run build:mcp`
Expected: PASS, bundle builds.

- [ ] **Step 5: Commit**

```bash
rtk git add mcp/src/git.ts mcp/src/git.test.ts mcp/src/ops.ts mcp/src/ops.test.ts mcp/src/server.ts
rtk git commit -m "fix: a finished card shows committed work and its checklist can be ticked"
```

### Task 4: Office view model

Everything visual below reads what this task produces. No DOM, no three.js, no IPC.

**Files:**
- Create: `src/company/office/model.ts`, `nav.ts`, `layout.ts`, `palette.ts`, `report.ts`, and a `.test.ts` beside each
- Modify: `src/company/types.ts:6` (`Look` gains `edited?: boolean`)

**Interfaces:**
- Consumes: `deriveTaskState` through `Task.status`, `statusLine` from `src/taskstate.ts`, `needsYou` and `NeedRow` from `src/company/needs.ts`, `sameFolder` from `src/workspaces.ts`.
- Produces, `model.ts`: the types below, `officeState`, `officeModel`, `departmentShirt`, `hairFor`, `lookOf`, `sceneKey`, `companyUsed`.
- Produces, `nav.ts`: `Level`, `DeskTab`, `Crumb`, `BUILDING`, `navUp`, `crumbs`, `escapeGoesUp`, `rowTarget`, `clampLevel`, `homeView`, `frameDelay`, `AMBIENT_MS`, `MOVE_MS`.
- Produces, `layout.ts`: constants including `FLIGHT_MS`, `seatSpot`, `towerLayout`, `shotFor`, `mixShots`, `flightPoint`.
- Produces, `palette.ts`: `OFFICE`, `TEXT_PAIRS`, `contrast`.
- Produces, `report.ts`: `DeskReport`, `deskReport`.

- [ ] **Step 1: Write the model tests**

```ts
// src/company/office/model.test.ts
import { describe, expect, it } from "vitest";
import { emptyCompany } from "../org";
import type { Company, Job, Person, RoleDef } from "../types";
import type { Alert } from "../watch";
import { companyUsed, departmentShirt, hairFor, lookOf, officeModel, officeState, sceneKey, type OfficePane } from "./model";

const status = (state: string, extra: object = {}) => ({ status: { state, ask: null, ...extra } }) as OfficePane["task"];
const person = (id: string, over: Partial<Person> = {}): Person => ({
  id, name: id.toUpperCase(), role: "developer", rank: "staff", floor: "a", boss: "founder", cli: "claude",
  look: { shirt: "#111111", hair: "#222222", prop: "laptop" }, ...over,
});
const company = (people: Person[] = []): Company => ({
  ...emptyCompany(),
  floors: [{ id: "a", dir: "D:\\a", name: "alpha" }, { id: "b", dir: "D:\\b", name: "beta" }],
  people: [...emptyCompany().people, ...people],
});
const pane = (personId: string, task: OfficePane["task"]): OfficePane => ({ personId, wsId: "w", paneId: `pane-${personId}`, task });
const job = (over: Partial<Job>): Job => ({ id: "J-1", title: "Add login", ask: "a", status: "running", plan: null, tokenCap: 100, tokensUsed: 5, tokenBase: {}, people: [], createdAt: 1, ...over });
const role = (id: string, department: string): RoleDef => ({ id, title: id.toUpperCase(), department, rank: "staff", cli: "claude", denyTools: [], look: { shirt: "#333333", hair: "#444444", prop: "pen" }, prompt: "", skills: [] });

describe("officeState", () => {
  it.each([["needs", "needs"], ["working", "working"], ["review", "review"], ["idle", "idle"], ["stopped", "stopped"], ["merged", "stopped"]])("%s draws as %s", (from, to) => {
    expect(officeState(status(from))).toBe(to);
  });
  it("is an empty chair with no pane", () => {
    expect(officeState(null)).toBe("stopped");
    expect(officeState(undefined)).toBe("stopped");
  });
  it("is idle when a person is out of usage: the limit is not a question", () => {
    expect(officeState(status("idle", { limit: { what: "x" } }))).toBe("idle");
  });
});

describe("companyUsed", () => {
  it("takes a Job, not a Founder or a floor", () => {
    expect(companyUsed([])).toBe(false);
    expect(companyUsed([job({ status: "rejected" })])).toBe(true);
  });
});

describe("looks", () => {
  it("dresses a department alike", () => {
    expect(departmentShirt("Engineering")).toBe(departmentShirt("Engineering"));
    expect(departmentShirt("Engineering")).not.toBe(departmentShirt("Marketing"));
  });
  it("gives a new department a steady colour from the spare list", () => {
    expect(departmentShirt("Sales")).toBe(departmentShirt("Sales"));
    expect(departmentShirt("Sales")).toMatch(/^#[0-9a-f]{6}$/);
  });
  it("gives people different hair by id, the same every time", () => {
    const hairs = new Set(["a1", "b2", "c3", "d4", "e5", "f6"].map(hairFor));
    expect(hairs.size).toBeGreaterThan(1);
    expect(hairFor("a1")).toBe(hairFor("a1"));
  });
  it("uses the department until the look was edited", () => {
    const r = role("qa", "Engineering");
    expect(lookOf(person("x"), r).shirt).toBe(departmentShirt("Engineering"));
    expect(lookOf(person("x", { look: { shirt: "#123456", hair: "#654321", prop: "book", edited: true } }), r)).toEqual({ shirt: "#123456", hair: "#654321", prop: "book" });
  });
  it("keeps the role's prop and the Founder's honey", () => {
    expect(lookOf(person("x"), role("qa", "Engineering")).prop).toBe("laptop");
    expect(lookOf(person("f", { rank: "founder" }), role("founder", "Management")).shirt).toBe("#f5b83d");
  });
  it("falls back to the stored shirt with no role", () => {
    expect(lookOf(person("x"), undefined).shirt).toBe("#111111");
  });
});

describe("officeModel", () => {
  it("puts the Founder on top, whatever project its pane sits in", () => {
    const m = officeModel(company(), [], [], [pane("founder", status("working"))], { openDirs: ["D:\\a"] });
    expect(m.founder?.state).toBe("working");
    expect(m.floors.flatMap((f) => f.people)).toEqual([]);
  });
  it("does not light a floor because the Founder's pane is there", () => {
    const m = officeModel(company(), [], [], [pane("founder", status("working"))], { openDirs: ["D:\\a"] });
    expect(m.floors.find((f) => f.id === "a")).toMatchObject({ lit: false, open: true });
  });
  it("shows floors with people, and open projects with nobody, and no others", () => {
    const m = officeModel(company([person("x", { floor: "b" })]), [], [], [], { openDirs: ["d:/a/"] });
    expect(m.floors.map((f) => f.id)).toEqual(["a", "b"]);
    expect(officeModel(company(), [], [], []).floors).toEqual([]);
  });
  it("keeps the company's floor order when states change", () => {
    const people = [person("x", { floor: "a" }), person("y", { floor: "b" })];
    const one = officeModel(company(people), [], [], [pane("y", status("needs"))]);
    const two = officeModel(company(people), [], [], [pane("x", status("needs"))]);
    expect(one.floors.map((f) => f.id)).toEqual(two.floors.map((f) => f.id));
  });
  it("seats the Lead first, then people in the order they joined", () => {
    const m = officeModel(company([person("s1"), person("l", { rank: "lead" }), person("s2")]), [], [], []);
    expect(m.floors[0].people.map((p) => [p.id, p.seat])).toEqual([["l", 0], ["s1", 1], ["s2", 2]]);
    expect(m.floors[0].lead).toBe("l");
  });
  it("darkens a closed project and keeps its people", () => {
    const m = officeModel(company([person("x")]), [], [], []);
    expect(m.floors[0]).toMatchObject({ open: false, lit: false });
    expect(m.floors[0].people[0]).toMatchObject({ id: "x", state: "stopped", paneId: null });
  });
  it("lights an open floor with someone at work", () => {
    const m = officeModel(company([person("x")]), [], [], [pane("x", status("idle"))], { openDirs: ["D:\\a"] });
    expect(m.floors[0]).toMatchObject({ open: true, lit: true });
  });
  it("counts papers for the Lead's desk, up to three", () => {
    const people = ["1", "2", "3", "4"].map((n) => person(n));
    const m = officeModel(company(people), [], [], people.map((p) => pane(p.id, status("review"))));
    expect(m.floors[0].papers).toBe(3);
  });
  it("puts what a person asks in Needs you, before merges, after plans", () => {
    const task = { status: { state: "needs", ask: { kind: "run", detail: "rm x", prompt: "", options: [] } } } as unknown as OfficePane["task"];
    const jobs = [job({ id: "J-1", status: "awaiting-plan", plan: { summary: "s", cards: [], hires: [] } }), job({ id: "J-2", status: "awaiting-merge", people: ["x"] })];
    const m = officeModel(company([person("x")]), jobs, [], [pane("x", task)]);
    expect(m.needs.map((r) => r.key)).toEqual(["plan-J-1", "ask-x", "merge-J-2"]);
    expect(m.needs[1]).toMatchObject({ kind: "alert", personId: "x", title: "X wants to run rm x", standing: false });
  });
  it("counts a floor's needs once per person", () => {
    const alert: Alert = { key: "k", job: null, personId: "x", text: "stuck", level: "warn" };
    const m = officeModel(company([person("x")]), [], [alert], [pane("x", status("needs"))]);
    expect(m.floors[0].needs).toBe(1);
    expect(m.counts.needs).toBe(m.needs.length);
  });
  it("lists the Jobs still going, in words", () => {
    const m = officeModel(company(), [job({ status: "running" }), job({ id: "J-2", status: "done" })], [], []);
    expect(m.jobs).toEqual([{ id: "J-1", title: "Add login", status: "running", label: "Running", tokensUsed: 5, tokenCap: 100 }]);
  });
  it("uses role titles, boards and flights when given them", () => {
    const m = officeModel(company([person("x", { role: "qa" })]), [], [], [], {
      roles: [role("qa", "Engineering")], boards: { a: { todo: 1, doing: 2, done: 3 } }, flights: [{ id: "f", from: "x", to: "founder", at: 1 }],
    });
    expect(m.floors[0].people[0]).toMatchObject({ title: "QA", department: "Engineering" });
    expect(m.floors[0].board).toEqual({ todo: 1, doing: 2, done: 3 });
    expect(m.flights).toHaveLength(1);
  });
  it("makes a readable title from the role id with no role file", () => {
    expect(officeModel(company([person("x")]), [], [], []).floors[0].people[0].title).toBe("Developer");
  });
  it("has no Founder with an empty list of people", () => {
    expect(officeModel({ ...company(), people: [] }, [], [], []).founder).toBeNull();
  });
});

describe("sceneKey", () => {
  const m = (state: string) => officeModel(company([person("x")]), [], [], [pane("x", status(state))], { openDirs: ["D:\\a"] });
  it("is the same for the same picture", () => { expect(sceneKey(m("idle"))).toBe(sceneKey(m("idle"))); });
  it("changes when a person's state does", () => { expect(sceneKey(m("idle"))).not.toBe(sceneKey(m("working"))); });
  it("ignores rail text, which the scene does not draw", () => {
    const a = m("idle");
    expect(sceneKey({ ...a, needs: [] })).toBe(sceneKey({ ...a, needs: [{ key: "k", kind: "alert", jobId: null, personId: null, title: "t", detail: "" }] }));
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/office/model.test.ts`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write the model**

In `src/company/types.ts`:

```ts
export interface Look { shirt: string; hair: string; prop: string; edited?: boolean } // edited: the owner chose this look (Phase 3)
```

```ts
// src/company/office/model.ts
/* The office as a plain object. The scene and the shell read this and nothing else,
 * so what they show cannot drift from the company, and all of it can be tested. */
import { statusLine, type TaskStatus } from "../../taskstate";
import type { Task } from "../../tasks";
import { sameFolder } from "../../workspaces";
import { needsYou, type NeedRow } from "../needs";
import type { Company, Job, JobStatus, Person, Rank, RoleDef } from "../types";
import type { Alert } from "../watch";

export type OfficeState = "needs" | "working" | "review" | "idle" | "stopped";
export type OfficeTask = Pick<Task, "status" | "title">;

export interface OfficePane { personId: string; wsId: string; paneId: string; task: OfficeTask | null }
export interface BoardCounts { todo: number; doing: number; done: number }
/** A message on its way, for the paper plane. `at` is when it left. */
export interface Flight { id: string; from: string; to: string; at: number }

export interface OfficeExtra {
  roles?: RoleDef[];
  boards?: Record<string, BoardCounts>;
  flights?: Flight[];
  openDirs?: string[];
}

export interface OfficePerson {
  id: string;
  name: string;
  rank: Rank;
  title: string;
  department: string;
  cli: string;
  shirt: string;
  hair: string;
  prop: string;
  state: OfficeState;
  doing: string | null;
  floorId: string | null;
  seat: number;
  paneId: string | null;
  wsId: string | null;
}

export interface OfficeFloor {
  id: string;
  name: string;
  dir: string;
  open: boolean;
  lit: boolean;
  people: OfficePerson[];
  lead: string | null;
  needs: number;
  papers: number;
  board: BoardCounts | null;
}

export interface OfficeJob { id: string; title: string; status: JobStatus; label: string; tokensUsed: number; tokenCap: number }

export interface OfficeModel {
  founder: OfficePerson | null;
  floors: OfficeFloor[];
  needs: NeedRow[];
  jobs: OfficeJob[];
  flights: Flight[];
  counts: { people: number; working: number; needs: number };
}

const MAX_PAPERS = 3;

/** The Jobs still going, as the rail names them. */
const GOING: Partial<Record<JobStatus, string>> = { planning: "Planning", "awaiting-plan": "Waiting for your approval", running: "Running", paused: "Paused" };

export const FOUNDER_SHIRT = "#f5b83d";
const SHIRTS: Record<string, string> = { Management: "#5ec2f0", Engineering: "#5fd08a", Design: "#ff6b6b", Marketing: "#e0913a", Research: "#b18cf0" };
const SPARE_SHIRTS = ["#7d8b99", "#c98fb4", "#9db36b", "#6f9fd8"];
export const HAIRS = ["#2a2e3a", "#4a3226", "#7a5230", "#b08a55", "#55585f", "#7a2f2a"];

const hash = (s: string) => [...s].reduce((n, c) => (n * 31 + c.charCodeAt(0)) >>> 0, 7);

/** What everyone chosen by the app sees. A role from the Founder may name any department. */
export const departmentShirt = (department: string): string => SHIRTS[department] ?? SPARE_SHIRTS[hash(department) % SPARE_SHIRTS.length];
export const hairFor = (id: string): string => HAIRS[hash(id) % HAIRS.length];

export function lookOf(p: Person, role: RoleDef | undefined): { shirt: string; hair: string; prop: string } {
  if (p.look.edited) return { shirt: p.look.shirt, hair: p.look.hair, prop: p.look.prop };
  const shirt = p.rank === "founder" ? FOUNDER_SHIRT : role ? departmentShirt(role.department) : p.look.shirt;
  return { shirt, hair: hairFor(p.id), prop: p.look.prop };
}

/** The Founder always exists, and every open project is a floor, so only a Job says
 *  that someone used the Company. */
export const companyUsed = (jobs: Job[]): boolean => jobs.length > 0;

/** The one place a character's state comes from. Stopped covers no pane and a merged agent: the chair is empty. */
export function officeState(task: Pick<Task, "status"> | null | undefined): OfficeState {
  const s = task?.status.state;
  return s === "needs" || s === "working" || s === "review" || s === "idle" ? s : "stopped";
}

const titleCase = (id: string) => id.replace(/(^|-)([a-z])/g, (_, a: string, b: string) => `${a ? " " : ""}${b.toUpperCase()}`);

function toPerson(p: Person, role: RoleDef | undefined, pane: OfficePane | undefined, seat: number): OfficePerson {
  return {
    id: p.id, name: p.name, rank: p.rank, title: role?.title ?? titleCase(p.role), department: role?.department ?? "",
    cli: p.cli, ...lookOf(p, role), state: officeState(pane?.task), doing: pane?.task?.title ?? null,
    floorId: p.floor, seat, paneId: pane?.paneId ?? null, wsId: pane?.wsId ?? null,
  };
}

/** What a person is asking shows in Needs you: they run with skip permissions, so it is rare, and worth a line. */
const askRow = (p: OfficePerson, status: TaskStatus): NeedRow =>
  ({ key: `ask-${p.id}`, kind: "alert", jobId: null, personId: p.id, title: statusLine(p.name, status), detail: "", standing: false });

export function officeModel(company: Company, jobs: Job[], alerts: Alert[], panes: OfficePane[], extra: OfficeExtra = {}): OfficeModel {
  const roleOf = (id: string) => extra.roles?.find((r) => r.id === id);
  const paneOf = (id: string) => panes.find((p) => p.personId === id);
  const open = (dir: string) => (extra.openDirs ?? []).some((d) => sameFolder(d, dir));

  const top = company.people.find((p) => p.rank === "founder");
  const founder = top ? toPerson(top, roleOf(top.role), paneOf(top.id), 0) : null;

  const rows = needsYou(jobs, alerts, { floors: company.floors, roles: extra.roles ?? [] });

  const floors = company.floors.map((f): OfficeFloor => {
    const here = company.people.filter((p) => p.floor === f.id);
    const seated = [...here.filter((p) => p.rank === "lead"), ...here.filter((p) => p.rank !== "lead")];
    const people = seated.map((p, seat) => toPerson(p, roleOf(p.role), paneOf(p.id), seat));
    const wanted = new Set([...people.filter((p) => p.state === "needs").map((p) => p.id), ...rows.filter((r) => people.some((p) => p.id === r.personId)).map((r) => r.personId!)]);
    return {
      id: f.id, name: f.name, dir: f.dir, open: open(f.dir), lit: open(f.dir) && people.some((p) => p.state !== "stopped"), people,
      lead: people.find((p) => p.rank === "lead")?.id ?? null, needs: wanted.size,
      papers: Math.min(MAX_PAPERS, people.filter((p) => p.state === "review").length), board: extra.boards?.[f.id] ?? null,
    };
  }).filter((f) => f.people.length || f.open);

  const everyone = [...(founder ? [founder] : []), ...floors.flatMap((f) => f.people)];
  const asks = everyone.filter((p) => p.state === "needs").map((p) => askRow(p, paneOf(p.id)!.task!.status));
  const at = rows.findIndex((r) => r.kind === "merge");
  const needs = at < 0 ? [...rows, ...asks] : [...rows.slice(0, at), ...asks, ...rows.slice(at)];

  return {
    founder, floors, needs,
    jobs: jobs.flatMap((j) => (GOING[j.status] ? [{ id: j.id, title: j.title, status: j.status, label: GOING[j.status]!, tokensUsed: j.tokensUsed, tokenCap: j.tokenCap }] : [])),
    flights: extra.flights ?? [],
    counts: { people: everyone.length, working: everyone.filter((p) => p.state === "working").length, needs: needs.length },
  };
}

/** What the scene draws, as one string: the same string means nothing to redraw. */
export function sceneKey(m: OfficeModel): string {
  const who = (p: OfficePerson) => [p.id, p.state, p.shirt, p.hair, p.prop, p.cli, p.seat, p.rank].join(":");
  return JSON.stringify([m.founder && who(m.founder), m.floors.map((f) => [f.id, f.open, f.lit, f.papers, f.board, f.people.map(who)]), m.flights.map((x) => x.id)]);
}
```

Note: `src/tasks.ts` is imported as a type only, so this file pulls in no IPC. `src/workspaces.ts` imports `FOUNDER_ID` and types only.

- [ ] **Step 4: Write the navigation tests and code**

```ts
// src/company/office/nav.test.ts
import { describe, expect, it } from "vitest";
import type { NeedRow } from "../needs";
import type { OfficeModel } from "./model";
import { AMBIENT_MS, BUILDING, MOVE_MS, clampLevel, crumbs, escapeGoesUp, frameDelay, homeView, navUp, rowTarget, type Level } from "./nav";

const founder = { id: "founder", name: "Founder", floorId: null };
const ana = { id: "ana", name: "Ana", floorId: "a" };
const model = { founder, floors: [{ id: "a", name: "alpha", people: [ana] }] } as unknown as OfficeModel;
const floor: Level = { kind: "floor", floorId: "a" };
const desk: Level = { kind: "desk", personId: "ana", floorId: "a", tab: "terminal" };
const row = (over: Partial<NeedRow>): NeedRow => ({ key: "k", kind: "alert", jobId: null, personId: null, title: "t", detail: "", ...over });

describe("navUp", () => {
  it("goes one level up and stops at the building", () => {
    expect(navUp(desk)).toEqual(floor);
    expect(navUp(floor)).toEqual(BUILDING);
    expect(navUp(BUILDING)).toEqual(BUILDING);
  });
  it("takes the Founder's desk straight to the building", () => {
    expect(navUp({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" })).toEqual(BUILDING);
  });
});

describe("crumbs", () => {
  it("names the path, with the last one current", () => {
    expect(crumbs(desk, model).map((c) => c.label)).toEqual(["ByteHive", "alpha", "Ana"]);
    expect(crumbs(desk, model).map((c) => c.current)).toEqual([false, false, true]);
    expect(crumbs(BUILDING, model)).toEqual([{ label: "ByteHive", level: BUILDING, current: true }]);
  });
  it("has no floor for the Founder", () => {
    expect(crumbs({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" }, model).map((c) => c.label)).toEqual(["ByteHive", "Founder"]);
  });
});

describe("escapeGoesUp", () => {
  const calm = { level: floor, terminal: false, dialog: false, dock: false, typing: false };
  it("goes up with nothing in the way", () => { expect(escapeGoesUp(calm)).toBe(true); });
  it.each(["terminal", "dialog", "dock", "typing"] as const)("leaves Esc to %s", (k) => {
    expect(escapeGoesUp({ ...calm, [k]: true })).toBe(false);
  });
  it("has nowhere to go from the building", () => { expect(escapeGoesUp({ ...calm, level: BUILDING })).toBe(false); });
});

describe("rowTarget", () => {
  it("flies to the person a row names", () => {
    expect(rowTarget(row({ personId: "ana" }), model)).toEqual(desk);
    expect(rowTarget(row({ personId: "founder" }), model)).toEqual({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" });
  });
  it("opens Changes for a merge", () => {
    expect(rowTarget(row({ kind: "merge", personId: "ana" }), model)).toEqual({ ...desk, tab: "changes" });
  });
  it("stays at the building for a row about a Job or nobody", () => {
    expect(rowTarget(row({ kind: "plan" }), model)).toEqual(BUILDING);
    expect(rowTarget(row({ personId: "gone" }), model)).toEqual(BUILDING);
  });
});

describe("clampLevel", () => {
  it("keeps a level that still exists", () => {
    expect(clampLevel(desk, model)).toBe(desk);
    expect(clampLevel(floor, model)).toBe(floor);
  });
  it("falls back when a person or a floor has gone", () => {
    expect(clampLevel({ ...desk, personId: "gone" }, model)).toEqual(floor);
    expect(clampLevel({ kind: "floor", floorId: "z" }, model)).toEqual(BUILDING);
  });
});

describe("homeView", () => {
  const used = { used: true, webgl: true, elsewhere: false };
  it("is the office once the Company has been used", () => { expect(homeView(used)).toBe("office"); });
  it("is the app as it was for everyone else", () => { expect(homeView({ ...used, used: false })).toBe("inbox"); });
  it("is the Inbox with no WebGL, or with the Company in another window", () => {
    expect(homeView({ ...used, webgl: false })).toBe("inbox");
    expect(homeView({ ...used, elsewhere: true })).toBe("inbox");
  });
});

describe("frameDelay", () => {
  const still = { visible: true, moving: false, ambient: false, reduced: false };
  it("does not draw at all when nothing moves", () => { expect(frameDelay(still)).toBeNull(); });
  it("stops when hidden, whatever is moving", () => { expect(frameDelay({ ...still, visible: false, moving: true, ambient: true })).toBeNull(); });
  it("caps moving at 30 frames a second", () => { expect(frameDelay({ ...still, moving: true })).toBe(MOVE_MS); expect(MOVE_MS).toBeGreaterThanOrEqual(1000 / 30); });
  it("draws working people at a low rate", () => { expect(frameDelay({ ...still, ambient: true })).toBe(AMBIENT_MS); });
  it("drops ambient motion for reduced motion, not a move in progress", () => {
    expect(frameDelay({ ...still, ambient: true, reduced: true })).toBeNull();
    expect(frameDelay({ ...still, moving: true, reduced: true })).toBe(MOVE_MS);
  });
});
```

```ts
// src/company/office/nav.ts
/* Where the owner is in the office, as data. One axis: Building, Floor, Desk. */
import type { NeedRow } from "../needs";
import { FOUNDER_ID } from "../types";
import type { OfficeModel, OfficePerson } from "./model";

export type DeskTab = "terminal" | "changes" | "report";
export type Level =
  | { kind: "building" }
  | { kind: "floor"; floorId: string }
  | { kind: "desk"; personId: string; floorId: string | null; tab: DeskTab };

export interface Crumb { label: string; level: Level; current: boolean }

export const BUILDING: Level = { kind: "building" };
/** 30 frames a second, rounded up so a slow timer never makes it 31. */
export const MOVE_MS = 34;
/** Working people move at 8 frames a second: toy-like, and cheap enough to leave on. */
export const AMBIENT_MS = 125;

const everyone = (m: OfficeModel): OfficePerson[] => [...(m.founder ? [m.founder] : []), ...m.floors.flatMap((f) => f.people)];

export function navUp(l: Level): Level {
  if (l.kind === "desk" && l.floorId) return { kind: "floor", floorId: l.floorId };
  return BUILDING;
}

export function crumbs(l: Level, m: OfficeModel): Crumb[] {
  const path: Array<{ label: string; level: Level }> = [{ label: "ByteHive", level: BUILDING }];
  const floorId = l.kind === "building" ? null : l.floorId;
  if (floorId) path.push({ label: m.floors.find((f) => f.id === floorId)?.name ?? floorId, level: { kind: "floor", floorId } });
  if (l.kind === "desk") path.push({ label: everyone(m).find((p) => p.id === l.personId)?.name ?? l.personId, level: l });
  return path.map((c, i) => ({ ...c, current: i === path.length - 1 }));
}

/** Esc is the terminal's while it has focus, and a dialog's, the dock's or a text field's while they are up. */
export function escapeGoesUp(o: { level: Level; terminal: boolean; dialog: boolean; dock: boolean; typing: boolean }): boolean {
  return o.level.kind !== "building" && !o.terminal && !o.dialog && !o.dock && !o.typing;
}

export function rowTarget(row: NeedRow, m: OfficeModel): Level {
  const p = row.personId ? everyone(m).find((x) => x.id === row.personId) : undefined;
  if (!p) return BUILDING;
  return { kind: "desk", personId: p.id, floorId: p.id === FOUNDER_ID ? null : p.floorId, tab: row.kind === "merge" ? "changes" : "terminal" };
}

/** People leave and projects close while the owner looks at them. */
export function clampLevel(l: Level, m: OfficeModel): Level {
  if (l.kind === "desk" && everyone(m).some((p) => p.id === l.personId)) return l;
  const floorId = l.kind === "building" ? null : l.floorId;
  if (floorId && m.floors.some((f) => f.id === floorId)) return l.kind === "floor" ? l : { kind: "floor", floorId };
  return BUILDING;
}

export function homeView(o: { used: boolean; webgl: boolean; elsewhere: boolean }): "office" | "inbox" {
  return o.used && o.webgl && !o.elsewhere ? "office" : "inbox";
}

/** How long until the next frame, or null to stop drawing. */
export function frameDelay(a: { visible: boolean; moving: boolean; ambient: boolean; reduced: boolean }): number | null {
  if (!a.visible) return null;
  if (a.moving) return MOVE_MS;
  return a.ambient && !a.reduced ? AMBIENT_MS : null;
}
```

- [ ] **Step 5: Write the layout tests and code**

```ts
// src/company/office/layout.test.ts
import { describe, expect, it } from "vitest";
import type { OfficeModel } from "./model";
import { BUILDING } from "./nav";
import { FOV, ROOM_D, ROOM_W, STOREY, flightPoint, mixShots, seatSpot, shotFor, towerLayout } from "./layout";

const floor = (id: string, people: string[]) => ({ id, name: id, people: people.map((p, seat) => ({ id: p, seat })) });
const model = (n: number) => ({ founder: { id: "founder" }, floors: Array.from({ length: n }, (_, i) => floor(`f${i}`, i ? ["x"] : ["a", "b"])) }) as unknown as OfficeModel;

describe("seatSpot", () => {
  it.each([1, 2, 6, 9])("keeps %i desks in the room, none on another", (count) => {
    const spots = Array.from({ length: count }, (_, i) => seatSpot(i, count));
    for (const s of spots) {
      expect(Math.abs(s.x)).toBeLessThan(ROOM_W / 2);
      expect(Math.abs(s.z)).toBeLessThan(ROOM_D / 2);
    }
    expect(new Set(spots.map((s) => `${s.x},${s.z}`)).size).toBe(count);
  });
  it("does not move anyone when someone joins a row that fits", () => {
    expect(seatSpot(0, 2)).toEqual(seatSpot(0, 3));
  });
});

describe("towerLayout", () => {
  it("stacks floors from the ground in model order, the roof above them", () => {
    const t = towerLayout(model(3));
    expect(t.floors.map((f) => f.id)).toEqual(["f0", "f1", "f2"]);
    expect(t.floors[1].y - t.floors[0].y).toBe(STOREY);
    expect(t.roofY).toBeGreaterThan(t.floors[2].y);
  });
  it("still has a roof with no floors", () => {
    const t = towerLayout(model(0));
    expect(t.floors).toEqual([]);
    expect(t.roofY).toBeGreaterThan(0);
  });
  it("has a desk per person and the lift runs the full height", () => {
    const t = towerLayout(model(2));
    expect(t.floors[0].desks.map((d) => d.personId)).toEqual(["a", "b"]);
    expect(t.lift.top).toBeGreaterThanOrEqual(t.roofY);
  });
});

describe("shotFor", () => {
  const t = towerLayout(model(4));
  it("frames the whole tower at the building, wider on a narrow screen", () => {
    const wide = shotFor(BUILDING, t, 2);
    const tall = shotFor(BUILDING, t, 0.7);
    expect(wide.ty).toBeCloseTo(t.roofY / 2, 0);
    const dist = (s: typeof wide) => Math.hypot(s.px - s.tx, s.py - s.ty, s.pz - s.tz);
    expect(dist(tall)).toBeGreaterThan(dist(wide));
    expect(wide.fov).toBe(FOV);
  });
  it("frames at most six floors, around the middle of a tall tower", () => {
    const tallTower = towerLayout(model(12));
    const six = towerLayout(model(6));
    const dist = (s: ReturnType<typeof shotFor>) => Math.hypot(s.px - s.tx, s.py - s.ty, s.pz - s.tz);
    expect(dist(shotFor(BUILDING, tallTower, 1.6))).toBeCloseTo(dist(shotFor(BUILDING, six, 1.6)), 3);
  });
  it("comes closer for a floor, closer again for a desk", () => {
    const dist = (s: ReturnType<typeof shotFor>) => Math.hypot(s.px - s.tx, s.py - s.ty, s.pz - s.tz);
    const b = shotFor(BUILDING, t, 1.6);
    const f = shotFor({ kind: "floor", floorId: "f1" }, t, 1.6);
    const d = shotFor({ kind: "desk", personId: "x", floorId: "f1", tab: "terminal" }, t, 1.6);
    expect(dist(f)).toBeLessThan(dist(b));
    expect(dist(d)).toBeLessThan(dist(f));
    expect(f.ty).toBeCloseTo(t.floors[1].y + 1, 0);
  });
  it("frames the roof for the Founder", () => {
    const s = shotFor({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" }, t, 1.6);
    expect(s.ty).toBeGreaterThanOrEqual(t.roofY);
  });
});

describe("mixShots", () => {
  it("runs from one shot to the other", () => {
    const a = shotFor(BUILDING, towerLayout(model(1)), 1.6);
    const b = { ...a, px: a.px + 10 };
    expect(mixShots(a, b, 0)).toEqual(a);
    expect(mixShots(a, b, 1).px).toBeCloseTo(b.px);
    expect(mixShots(a, b, 0.5).px).toBeCloseTo(a.px + 5);
  });
});

describe("flightPoint", () => {
  const t = towerLayout(model(3));
  it("leaves one floor, rises along the lift, arrives at the other", () => {
    expect(flightPoint(t, "f0", "f2", 0).y).toBeCloseTo(t.floors[0].y + 1.6);
    expect(flightPoint(t, "f0", "f2", 1).y).toBeCloseTo(t.floors[2].y + 1.6);
    expect(flightPoint(t, "f0", "f2", 0.5).x).toBeCloseTo(t.lift.x);
  });
  it("treats no floor as the roof", () => {
    expect(flightPoint(t, null, "f0", 0).y).toBeCloseTo(t.roofY + 1.6);
  });
});
```

```ts
// src/company/office/layout.ts
/* Where everything stands, in scene units, with no three.js: the tower's geometry and
 * the camera's shots are numbers, so they are tested here and the scene only draws them. */
import type { OfficeModel } from "./model";
import type { Level } from "./nav";

export const STOREY = 3.4;
export const SLAB = 0.42;
export const ROOM_W = 11;
export const ROOM_D = 7.4;
export const LIFT_W = 1.7;
export const GROUND = 0.5;
export const FOV = 24;
/** How long a message is in the air, for the paper plane. */
export const FLIGHT_MS = 2400;
const COLS = 3;
const DESK_W = 3;
const ROW_MAX = 2.6;
const MAX_FRAMED = 6;
const VIEW = { x: 0.62, y: 0.42, z: 1 };
const VIEW_LEN = Math.hypot(VIEW.x, VIEW.y, VIEW.z);

export interface Seat { x: number; z: number; rot: number }
export interface Desk extends Seat { personId: string }
export interface FloorSpot { id: string; y: number; desks: Desk[] }
export interface Tower { floors: FloorSpot[]; roofY: number; lift: { x: number; z: number; top: number }; width: number }
export interface Shot { px: number; py: number; pz: number; tx: number; ty: number; tz: number; fov: number }

/** Two rows of three in the usual floor. A Lead with more people, by hand, gets tighter rows. */
export function seatSpot(seat: number, count: number): Seat {
  const rows = Math.max(2, Math.ceil(count / COLS));
  const dz = Math.min(ROW_MAX, (ROOM_D - 1.6) / rows);
  const col = seat % COLS;
  const row = Math.floor(seat / COLS);
  return { x: (col - 1) * DESK_W, z: (row - (rows - 1) / 2) * dz, rot: row % 2 ? Math.PI : 0 };
}

export function towerLayout(m: OfficeModel): Tower {
  const floors = m.floors.map((f, i): FloorSpot => ({
    id: f.id,
    y: GROUND + i * STOREY,
    desks: f.people.map((p) => ({ personId: p.id, ...seatSpot(p.seat, f.people.length) })),
  }));
  const roofY = GROUND + m.floors.length * STOREY;
  return { floors, roofY, lift: { x: ROOM_W / 2 + LIFT_W / 2, z: -ROOM_D / 2 + LIFT_W / 2, top: roofY + STOREY }, width: ROOM_W + LIFT_W };
}

/** A shot from the fixed viewing angle, far enough to hold a box of this size. */
function shot(tx: number, ty: number, tz: number, w: number, h: number, aspect: number): Shot {
  const half = Math.tan((FOV * Math.PI) / 360);
  const dist = Math.max(h / 2 / half, w / 2 / (half * aspect)) * 1.18;
  const k = dist / VIEW_LEN;
  return { px: tx + VIEW.x * k, py: ty + VIEW.y * k, pz: tz + VIEW.z * k, tx, ty, tz, fov: FOV };
}

export function shotFor(l: Level, t: Tower, aspect: number): Shot {
  if (l.kind === "desk" && !l.floorId) return shot(0, t.roofY + 1.2, 0, ROOM_W * 0.7, STOREY * 1.2, aspect);
  const f = l.kind === "building" ? undefined : t.floors.find((x) => x.id === l.floorId);
  if (f && l.kind === "floor") return shot(0, f.y + 1, 0, t.width, STOREY * 1.3, aspect);
  if (f && l.kind === "desk") {
    const d = f.desks.find((x) => x.personId === l.personId);
    return shot(d?.x ?? 0, f.y + 1, d?.z ?? 0, ROOM_W * 0.62, STOREY * 1.05, aspect);
  }
  const h = Math.min(t.roofY + STOREY, MAX_FRAMED * STOREY + STOREY);
  return shot(0, t.roofY / 2, 0, t.width + 2, h, aspect);
}

export const mixShots = (a: Shot, b: Shot, k: number): Shot => {
  const at = (x: number, y: number) => x + (y - x) * k;
  return { px: at(a.px, b.px), py: at(a.py, b.py), pz: at(a.pz, b.pz), tx: at(a.tx, b.tx), ty: at(a.ty, b.ty), tz: at(a.tz, b.tz), fov: at(a.fov, b.fov) };
};

const CRUISE = 1.6;

/** A paper plane's place at `k` from 0 to 1: out to the lift, along it, in to the floor. */
export function flightPoint(t: Tower, from: string | null, to: string | null, k: number): { x: number; y: number; z: number } {
  const yOf = (id: string | null) => (t.floors.find((f) => f.id === id)?.y ?? t.roofY) + CRUISE;
  const a = yOf(from);
  const b = yOf(to);
  const mid = 0.15;
  if (k < mid) return { x: (t.lift.x * k) / mid, y: a, z: (t.lift.z * k) / mid };
  if (k > 1 - mid) return { x: (t.lift.x * (1 - k)) / mid, y: b, z: (t.lift.z * (1 - k)) / mid };
  return { x: t.lift.x, y: a + ((b - a) * (k - mid)) / (1 - 2 * mid), z: t.lift.z };
}
```

- [ ] **Step 6: Write the palette and report, with tests**

```ts
// src/company/office/palette.ts
/* The office's colours. Mesh colours from the approved look (style-a.webp), and the
 * text pairs on screen, which the test holds to WCAG AA. */
export const OFFICE = {
  sky: "#191d27", ground: "#23262e", shellOut: "#34363c", shellEdge: "#2a2c31", slabTop: "#3f3d3c",
  wallLit: "#b57f3a", wallDim: "#5b4630", wallDark: "#2b2a2c", floorLit: "#7a6248", floorDark: "#35322f",
  honey: "#f5b83d", glow: "#ffd98a", glass: "#8fb3c9", steel: "#5a6068",
  deskTop: "#9a6a3c", deskBody: "#2f3138", chair: "#24262c", screenOn: "#7cc4ff", screenOff: "#1b1e24", lampNeeds: "#f5b83d",
  bead: "#efc48a", plant: "#4f8f4a", pot: "#a8703c", paper: "#f2ece0",
} as const;

export const CHIP_BG = "#14161c";
export const PANEL_BG = "#15161a";

export const TEXT_PAIRS: Array<[string, string, string, number]> = [
  ["tag text on a chip", "#eceae4", CHIP_BG, 4.5],
  ["tag detail on a chip", "#b4b4bc", CHIP_BG, 4.5],
  ["needs tag on a chip", OFFICE.honey, CHIP_BG, 4.5],
  ["rail text on the panel", "#f4f4f5", PANEL_BG, 4.5],
  ["rail detail on the panel", "#b4b4bc", PANEL_BG, 4.5],
  ["breadcrumb on the sky", "#eceae4", OFFICE.sky, 4.5],
  ["bubble mark on honey", "#14161c", OFFICE.honey, 4.5],
  ["focus ring on the sky", OFFICE.honey, OFFICE.sky, 3],
];

const channel = (v: number) => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
const luminance = (hex: string) => {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * channel(n >> 16) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
};

export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
```

```ts
// src/company/office/palette.test.ts
import { describe, expect, it } from "vitest";
import { TEXT_PAIRS, contrast } from "./palette";

describe("contrast", () => {
  it("matches the known extremes", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrast("#777777", "#777777")).toBe(1);
  });
  it.each(TEXT_PAIRS)("%s passes", (_name, fg, bg, min) => {
    expect(contrast(fg, bg)).toBeGreaterThanOrEqual(min);
  });
});
```

```ts
// src/company/office/report.ts
/* The Report tab: what a person did, from the board and the Jobs. */
import type { Board } from "../../board";
import type { Job } from "../types";

export interface ReportCard {
  title: string;
  list: string;
  job: string | null;
  jobTitle: string | null;
  summary: string | null;
  files: string[];
  checklist: Array<{ text: string; done: boolean }>;
  stopped: boolean;
}
export interface DeskReport { name: string; branch: string | null; cards: ReportCard[] }

export function deskReport(p: { id: string; name: string; branch: string | null }, board: Board | null, jobs: Job[]): DeskReport {
  const cards = (board?.lists ?? []).flatMap((l) => l.cards
    .filter((c) => c.assigneeId === p.id || c.done?.by === p.name)
    .map((c): ReportCard => ({
      title: c.title, list: l.title, job: c.job ?? null, jobTitle: jobs.find((j) => j.id === c.job)?.title ?? null,
      summary: c.done?.summary ?? null, files: c.done?.files ?? [],
      checklist: c.checklist.map(({ text, done }) => ({ text, done })), stopped: !!c.stopped,
    })));
  return { name: p.name, branch: p.branch, cards };
}
```

```ts
// src/company/office/report.test.ts
import { describe, expect, it } from "vitest";
import type { Board } from "../../board";
import type { Job } from "../types";
import { deskReport } from "./report";

const card = (id: string, over: object) => ({ id, title: id, desc: "", labels: [], due: null, checklist: [], ...over });
const board: Board = { lists: [
  { id: "1", title: "Doing", cards: [card("mine", { assigneeId: "p1", job: "J-1", checklist: [{ id: "i", text: "Test", done: true }] }), card("other", { assigneeId: "p2" })] },
  { id: "2", title: "Done", cards: [card("done", { done: { repoRoot: "x", files: ["a.ts"], summary: "Fixed", at: 1, by: "Ana" } })] },
] } as Board;

describe("deskReport", () => {
  const me = { id: "p1", name: "Ana", branch: "bytehive/ana" };
  it("lists this person's cards, by id or by who finished them", () => {
    expect(deskReport(me, board, []).cards.map((c) => c.title)).toEqual(["mine", "done"]);
  });
  it("carries the evidence, the checklist and the Job", () => {
    const r = deskReport(me, board, [{ id: "J-1", title: "Add login" } as Job]);
    expect(r.cards[0]).toMatchObject({ list: "Doing", job: "J-1", jobTitle: "Add login", checklist: [{ text: "Test", done: true }] });
    expect(r.cards[1]).toMatchObject({ summary: "Fixed", files: ["a.ts"] });
  });
  it("is empty with no board", () => {
    expect(deskReport(me, null, [])).toEqual({ name: "Ana", branch: "bytehive/ana", cards: [] });
  });
});
```

- [ ] **Step 7: Run tests and the type check**

Run: `rtk proxy npx vitest run src/company/office && rtk npx tsc --noEmit`
Expected: PASS. If `contrast` fails a pair, change the colour in the pair, not the limit.

- [ ] **Step 8: Commit**

```bash
rtk git add src/company/types.ts src/company/office
rtk git commit -m "feat: office view model, navigation and layout"
```

### Task 5: Needs you rail and the office source

Starts with the `build-premium-website` skill. The rail is the one part of the office that must work with no WebGL, so it is plain DOM and knows nothing about the scene.

Moves everything out of the temporary panel except the people list. The dock panel keeps working on top of the new parts until Task 9 removes it, so the app stays usable at every step.

**Files:**
- Create: `src/company/office/source.ts`, `live.ts`, `rail.ts`, `rail.test.ts`, `office.css`
- Modify: `src/company/panel.ts`, `src/company/engine.ts`, `src/company/reduce.ts`. `office.css` is imported from `rail.ts`, as `panel.css` was from the panel.
- Test: `src/company/engine.test.ts`

**Interfaces:**
- Consumes: `officeModel`, `OfficeModel`, `BoardCounts`, `Flight` (Task 4), `rowTarget`, `Level` (Task 4), `onCompanyChange`, `companyNeeds` (Task 2), `typeable`, `floorContact` (Task 2).
- Produces, `source.ts`:

```ts
// src/company/office/source.ts
import type { OfficeModel } from "./model";
import type { DeskReport } from "./report";

export interface Snapshot {
  model: OfficeModel | null;
  note: string | null;   // why there is no model: "The Company is running in another ByteHive window."
  error: string | null;  // load failure, in words
}

/** All the office can see or do. The engine behind it in the app, fixtures in the demo. */
export interface OfficeSource {
  snapshot(now: number): Snapshot;
  onChange(fn: () => void): () => void;
  giveJob(ask: string): Promise<void>;
  messageFloor(floorId: string, text: string): Promise<void>;
  approve(job: string): Promise<void>;
  reject(job: string, note: string): Promise<void>;
  raiseCap(job: string, extra: number): Promise<void>;
  finishJob(job: string): Promise<void>;
  stopJob(job: string): Promise<void>;
  dismiss(key: string): void;
  /** Put a person's real pane on the stage. False when they have none. */
  showPerson(id: string): boolean;
  wakePerson(id: string): Promise<void>;
  report(personId: string): Promise<DeskReport | null>;
  /** Tell the source the office is on screen: it reads boards only then. */
  watching(on: boolean): void;
}
```

- Produces, `rail.ts`: `createRail(source: OfficeSource, go: (l: Level) => void): { mount(el: HTMLElement): void; draw(): void }`, `createJobBox(source: OfficeSource): { mount(el: HTMLElement): void; draw(): void }`.
- Produces, `engine.ts`: `messagePerson(id: string, text: string): Promise<void>`, `wakePerson(id: string): Promise<void>`, `companyFlights(now: number): Flight[]`.
- Produces, `live.ts`: `liveSource(): OfficeSource`.

- [ ] **Step 1: Write the failing tests**

Move `src/company/panel.test.ts` to `src/company/office/rail.test.ts` with `git mv`, and keep every case. Replace its engine mock with a fake source:

```ts
// src/company/office/rail.test.ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { NeedRow } from "../needs";
import type { OfficeModel } from "./model";
import { BUILDING } from "./nav";
import { createJobBox, createRail } from "./rail";
import type { OfficeSource, Snapshot } from "./source";

const planRow: NeedRow = {
  key: "plan-J-1", kind: "plan", jobId: "J-1", personId: null, title: "Approve plan: Add login", detail: "Build it 0 cards, 0 hires.",
  plan: { hires: [], floors: [], tokenCap: 100, newRoles: [] },
};
const model = (needs: NeedRow[] = [planRow], over: Partial<OfficeModel> = {}): OfficeModel => ({
  founder: null, floors: [], needs, jobs: [], flights: [], counts: { people: 0, working: 0, needs: needs.length }, ...over,
});
function fake(snap: Partial<Snapshot> = {}) {
  const listeners = new Set<() => void>();
  const s = {
    snap: { model: model(), note: null, error: null, ...snap } as Snapshot,
    snapshot: () => s.snap,
    onChange: (fn: () => void) => { listeners.add(fn); return () => listeners.delete(fn); },
    fire: () => listeners.forEach((fn) => fn()),
    giveJob: vi.fn(async () => {}), messageFloor: vi.fn(async () => {}), approve: vi.fn(async () => {}), reject: vi.fn(async () => {}),
    raiseCap: vi.fn(async () => {}), finishJob: vi.fn(async () => {}), stopJob: vi.fn(async () => {}), dismiss: vi.fn(),
    showPerson: vi.fn(() => true), wakePerson: vi.fn(async () => {}), report: vi.fn(async () => null), watching: vi.fn(),
  };
  return s as typeof s & OfficeSource;
}
const mountRail = (s: OfficeSource, go = vi.fn()) => {
  const el = document.createElement("div");
  document.body.appendChild(el);
  createRail(s, go).mount(el);
  return { el, go };
};
const click = (el: HTMLElement, sel: string) => el.querySelector<HTMLElement>(sel)!.click();

describe("Needs you rail", () => {
  beforeEach(() => { document.body.innerHTML = ""; });

  it("says it is starting, then shows a load failure", () => {
    const s = fake({ model: null });
    const { el } = mountRail(s);
    expect(el.textContent).toContain("Starting...");
    s.snap = { model: null, note: null, error: "bad file" };
    s.fire();
    expect(el.querySelector("[data-msg]")?.textContent).toContain("bad file");
  });
  it("says where the Company runs, instead of Starting", () => {
    const { el } = mountRail(fake({ model: null, note: "The Company is running in another ByteHive window." }));
    expect(el.textContent).toContain("another ByteHive window");
    expect(el.textContent).not.toContain("Starting...");
  });
  it("says so when nothing needs you", () => {
    expect(mountRail(fake({ model: model([]) })).el.textContent).toContain("Nothing needs you.");
  });
  it("approves a plan, once, while it works", async () => {
    let done = () => {};
    const s = fake();
    s.approve.mockImplementation(() => new Promise<void>((r) => { done = r; }));
    const { el } = mountRail(s);
    click(el, '[data-act="approve"]');
    click(el, '[data-act="approve"]');
    expect(s.approve).toHaveBeenCalledTimes(1);
    expect(el.querySelector('[data-act="approve"]')?.getAttribute("aria-disabled")).toBe("true");
    done();
  });
  it("sends a plan back with the note typed in its own row", async () => {
    const s = fake();
    const { el } = mountRail(s);
    el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value = "Fewer people";
    click(el, '[data-act="reject"]');
    expect(s.reject).toHaveBeenCalledWith("J-1", "Fewer people");
  });
  it("keeps a note while the rail redraws", () => {
    const s = fake();
    const { el } = mountRail(s);
    el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value = "half written";
    s.fire();
    expect(el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value).toBe("half written");
  });
  it("shows why an action failed", async () => {
    const s = fake();
    s.approve.mockRejectedValue(new Error("Codex could not load its settings."));
    const { el } = mountRail(s);
    click(el, '[data-act="approve"]');
    await new Promise((r) => setTimeout(r, 0));
    expect(el.querySelector("[data-msg]")?.textContent).toContain("Codex could not load");
  });
  it("flies to where a row points when the row is clicked", () => {
    const row: NeedRow = { key: "k", kind: "alert", jobId: null, personId: "founder", title: "Founder stuck", detail: "" };
    const s = fake({ model: model([row], { founder: { id: "founder", name: "Founder", floorId: null } as never }) });
    const { el, go } = mountRail(s);
    click(el, "[data-row]");
    expect(go).toHaveBeenCalledWith({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" });
  });
  it("does not fly when a button in the row was the target", () => {
    const { el, go } = mountRail(fake());
    click(el, '[data-act="approve"]');
    expect(go).not.toHaveBeenCalled();
  });
  it("stops a Job from the Jobs list", () => {
    const s = fake({ model: model([], { jobs: [{ id: "J-2", title: "T", status: "running", label: "Running", tokensUsed: 1, tokenCap: 2 }] }) });
    const { el } = mountRail(s);
    expect(el.textContent).toContain("Running");
    click(el, '[data-act="stop"]');
    expect(s.stopJob).toHaveBeenCalledWith("J-2");
  });
  it("keeps keyboard focus on the same button through a redraw", () => {
    const s = fake();
    const { el } = mountRail(s);
    el.querySelector<HTMLElement>('[data-act="approve"]')!.focus();
    s.fire();
    expect((document.activeElement as HTMLElement).dataset.act).toBe("approve");
  });
  it("escapes what an agent wrote", () => {
    const row: NeedRow = { key: "k", kind: "alert", jobId: null, personId: null, title: "<img src=x>", detail: "" };
    expect(mountRail(fake({ model: model([row]) })).el.querySelector("img")).toBeNull();
  });
  it("has BUILDING to go to for a row with nobody", () => {
    const row: NeedRow = { key: "k", kind: "paused", jobId: "J-1", personId: null, title: "Paused", detail: "" };
    const { el, go } = mountRail(fake({ model: model([row]) }));
    click(el, "[data-row]");
    expect(go).toHaveBeenCalledWith(BUILDING);
  });
});

describe("job box", () => {
  beforeEach(() => { document.body.innerHTML = ""; });
  const mountBox = (s: OfficeSource) => {
    const el = document.createElement("div");
    document.body.appendChild(el);
    createJobBox(s).mount(el);
    return el;
  };
  it("gives the Founder a job and clears the box", async () => {
    const s = fake();
    const el = mountBox(s);
    el.querySelector("textarea")!.value = "Add marketing to gcare";
    el.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true, bubbles: true }));
    await new Promise((r) => setTimeout(r, 0));
    expect(s.giveJob).toHaveBeenCalledWith("Add marketing to gcare");
    expect(el.querySelector("textarea")!.value).toBe("");
  });
  it("keeps the words when the job is refused, and says why", async () => {
    const s = fake();
    s.giveJob.mockRejectedValue(new Error("Open a project first."));
    const el = mountBox(s);
    el.querySelector("textarea")!.value = "Do it";
    el.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true, bubbles: true }));
    await new Promise((r) => setTimeout(r, 0));
    expect(el.querySelector("textarea")!.value).toBe("Do it");
    expect(el.textContent).toContain("Open a project first.");
  });
  it("sends nothing for an empty box", () => {
    const s = fake();
    const el = mountBox(s);
    el.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true, bubbles: true }));
    expect(s.giveJob).not.toHaveBeenCalled();
  });
});
```

Cases from the old panel test that are not above move across unchanged, with `eng.x` replaced by the fake's `s.x`.

Add to `src/company/engine.test.ts`:

```ts
describe("the owner talking to a person", () => {
  it("types the message into their terminal, marked as the owner's", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    await vi.advanceTimersByTimeAsync(9000); // past the boot wait
    io.sent.length = 0;
    await engine.messagePerson(member().spec.personId!, "Please\u001b[2J check the login page");
    expect(io.sent[io.sent.length - 1]).toEqual([member().id, "[From the owner] Please[2J check the login page"]);
  });
  it("sends nothing for an empty message", async () => {
    await engine.messagePerson("founder", "   ");
    expect(io.sent).toEqual([]);
  });
});

describe("waking a person", () => {
  it("starts someone with no pane", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    const id = member().spec.personId!;
    const ws = [...workspaces.values()].find((w) => [...w.panes.values()].some((p) => p.spec.personId === id))!;
    ws.panes.clear();
    io.spawned.length = 0;
    await engine.wakePerson(id);
    expect(io.spawned).toHaveLength(1);
  });
});

describe("flights", () => {
  it("records a message between people for a moment", async () => {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    const id = member().spec.personId!;
    await request({ type: "message", ts: 1, from: id, to: "Founder", message: "done" });
    expect(engine.companyFlights(Date.now())).toMatchObject([{ from: id, to: "founder" }]);
    expect(engine.companyFlights(Date.now() + 5000)).toEqual([]);
  });
  it("records nothing for a message that was held or refused", async () => {
    await request({ type: "message", ts: 1, from: "nobody", to: "Founder", message: "x" });
    expect(engine.companyFlights(Date.now())).toEqual([]);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/office/rail.test.ts src/company/engine.test.ts`
Expected: FAIL.

- [ ] **Step 3: Add the engine parts**

In `src/company/engine.ts`:

```ts
let flights: Flight[] = [];

export const companyFlights = (now: number): Flight[] => {
  flights = flights.filter((f) => now - f.at < FLIGHT_MS);
  return flights;
};

/** The owner types to someone from the office. Their word goes straight through: nothing holds it. */
export const messagePerson = (id: string, text: string): Promise<void> => enqueue(async () => {
  const words = typeable(text).trim();
  if (state && words) await deliver(id, `[From the owner] ${words}`);
}, "Sending a message");

/** Start someone who has no running pane, on the conversation and branch they had. */
export const wakePerson = (id: string): Promise<void> => enqueue(async () => {
  const p = state && personById(state.company, id);
  if (!p) return;
  await ensurePerson(p);
  const pane = paneOf(id)?.pane;
  if (pane && !pane.running && !pane.spawning) await pane.restart?.();
}, "Starting a person");
```

In `tick()`, after `await run(r.effects);`:

```ts
      noteFlight(req, r.effects);
```

```ts
/** A delivered message is a paper plane. A held one was bounced back to its sender. */
function noteFlight(req: CompanyRequest, effects: Effect[]): void {
  if (req.type !== "message") return;
  for (const e of effects) {
    if (e.kind === "deliver" && e.personId !== req.from) flights.push({ id: `${req.ts}-${flights.length}-${e.personId}`, from: req.from, to: e.personId, at: Date.now() });
  }
  if (effects.length) changed();
}
```

Import `type Flight` from `./office/model`, `FLIGHT_MS` from `./office/layout`, `typeable` from `./reduce`.

- [ ] **Step 4: Write the source**

`src/company/office/source.ts` as given in Interfaces. Then:

```ts
// src/company/office/live.ts
/* The office's view of the real company. */
import { readBoardFile } from "../../boardfile";
import { revealPane } from "../../agentbridge";
import { activeWs, workspaces } from "../../appstate";
import { onTasksChange, taskOf } from "../../tasks";
import * as engine from "../engine";
import { floorContact } from "../org";
import { officeModel, type BoardCounts, type OfficePane } from "./model";
import { deskReport } from "./report";
import type { OfficeSource } from "./source";

/** How often the wall boards are read while the office is on screen. */
const BOARDS_MS = 5000;

const panesNow = (): OfficePane[] => [...workspaces.values()].flatMap((ws) => [...ws.panes.values()].flatMap((p) =>
  (p.spec.personId ? [{ personId: p.spec.personId, wsId: ws.id, paneId: p.id, task: taskOf(p.id) ?? null }] : [])));

const count = (lists: Array<{ title: string; cards: unknown[] }>, title: string) =>
  lists.find((l) => l.title.trim().toLowerCase() === title)?.cards.length ?? 0;

export function liveSource(): OfficeSource {
  const boards: Record<string, BoardCounts> = {};
  let timer: number | null = null;

  const readBoards = async () => {
    const s = engine.companyView();
    for (const f of s?.company.floors.filter((x) => s.company.people.some((p) => p.floor === x.id)) ?? []) {
      const file = await readBoardFile(f.dir).catch(() => null);
      if (file) boards[f.id] = { todo: count(file.board.lists, "to do"), doing: count(file.board.lists, "doing"), done: count(file.board.lists, "done") };
    }
  };

  return {
    snapshot(now) {
      const s = engine.companyView();
      if (!s) return { model: null, note: engine.companyElsewhere(), error: engine.companyError() };
      const openDirs = [...workspaces.values()].flatMap((w) => (w.dir ? [w.dir] : []));
      return { model: officeModel(s.company, s.jobs, engine.companyAlerts(), panesNow(), { roles: s.roles, boards, openDirs, flights: engine.companyFlights(now) }), note: null, error: null };
    },
    onChange(fn) {
      const off = engine.onCompanyChange(fn);
      const offTasks = onTasksChange(fn);
      return () => { off(); offTasks(); };
    },
    giveJob: engine.giveJob,
    messageFloor: async (floorId, text) => {
      const s = engine.companyView();
      const who = s && floorContact(s.company, floorId);
      if (who) await engine.messagePerson(who.id, text);
    },
    approve: engine.approve,
    reject: engine.reject,
    raiseCap: engine.raiseCap,
    finishJob: engine.finishJob,
    stopJob: engine.stopJob,
    dismiss: engine.dismissAlert,
    showPerson(id) {
      for (const ws of workspaces.values()) for (const p of ws.panes.values()) if (p.spec.personId === id) return revealPane(ws.id, p.id);
      return false;
    },
    wakePerson: engine.wakePerson,
    async report(personId) {
      const s = engine.companyView();
      const p = s?.company.people.find((x) => x.id === personId);
      if (!s || !p) return null;
      const dir = s.company.floors.find((f) => f.id === p.floor)?.dir ?? activeWs?.dir ?? null;
      const file = dir ? await readBoardFile(dir).catch(() => null) : null;
      return deskReport({ id: p.id, name: p.name, branch: p.branch ?? null }, file?.board ?? null, s.jobs);
    },
    watching(on) {
      if (on && timer === null) {
        void readBoards();
        timer = window.setInterval(() => void readBoards(), BOARDS_MS);
      } else if (!on && timer !== null) {
        window.clearInterval(timer);
        timer = null;
      }
    },
  };
}
```

`onTasksChange` gives back an unsubscribe already: `inbox.ts` keeps it as `offTasks`.

- [ ] **Step 5: Write the rail**

Move `actButton`, `why`, `planHtml`, `rowHtml`, `jobHtml`, the busy set and the focus keeping from `src/company/panel.ts` into `src/company/office/rail.ts`, with these changes:

- `rowHtml` wraps the title in `<button type="button" class="co-go" data-row="${esc(r.key)}">` so a row is a real button. Click on `[data-row]` calls `go(rowTarget(row, model))`.
- A plan row gets a note box under its buttons: `<textarea class="co-note" data-note="${esc(job)}" rows="2" placeholder="Why send it back? Leave empty to cancel the job." aria-label="Why send it back"></textarea>`.
- A redraw replaces rows, so before writing `innerHTML` keep the value of every `[data-note]` in a `Map` by job id, write them back after, and restore caret focus to a note that held focus.
- Action buttons are looked up as `[data-act]` first, so a click on one never falls through to `[data-row]`.
- The Jobs list has one Stop button per Job, from `model.jobs`, with `label` and tokens in words as the panel has them.
- `createJobBox` owns the form, its own `role="alert"` line and its busy flag. Labels: `Give the Founder a job`, placeholder `Add marketing to gcare`, button `Send`.

`rail.ts` starts:

```ts
// src/company/office/rail.ts
/* Needs you: what the owner has to act on, in words, at every level of the office
 * and in the Inbox when there is no 3D. Knows nothing about the scene. */
import "./office.css";
import type { NeedRow, PlanDetail } from "../needs";
import type { OfficeJob } from "./model";
import { rowTarget, type Level } from "./nav";
import type { OfficeSource } from "./source";

const RAISE_BY = 1_000_000;
const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
```

`mount(el)` writes the skeleton once, in this order, and subscribes with `source.onChange(draw)`:

```ts
el.innerHTML =
  `<p class="co-error" role="alert" data-msg hidden></p>` +
  `<h3 id="coNeedsH">Needs you <span class="co-n" data-count></span></h3>` +
  `<ul class="co-needs" aria-labelledby="coNeedsH" data-needs></ul>` +
  `<h3 id="coJobsH">Jobs</h3><ul class="co-jobs" aria-labelledby="coJobsH" data-jobs></ul>`;
```

`src/company/office/office.css`: copy the `.co-*` rules from `src/company/panel.css`, add `.co-go` (full-width, left aligned, no chrome, `font: inherit`, `:focus-visible` honey ring), `.co-note`, `.co-n`. Colours through `--ib-panel`, `--ib-ink`, `--ib-ink-2`, `--ib-line`; honey only for a row that needs you.

- [ ] **Step 6: Point the panel at them**

`src/company/panel.ts` shrinks to: build `const source = liveSource()`, mount a job box, a rail, and the old people list, with `go` calling `source.showPerson` on a desk level. Its only remaining job is to keep Ctrl+Shift+O working until Task 9. Add a line at the top: `/* Goes in Task 9 of the phases 2 to 4 plan. */`.

- [ ] **Step 7: Run tests and the type check**

Run: `rtk proxy npx vitest run src/company && rtk npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 8: Check it by eye in the sandbox**

Start the dev app (Task 1 Step 6), press Ctrl+Shift+O, give a job on a scratch project. Expected: rail rows behave as before, Send back has its own box, a row click opens the person's pane.

- [ ] **Step 9: Commit**

```bash
rtk git add src/company/office src/company/engine.ts src/company/engine.test.ts src/company/reduce.ts src/company/panel.ts src/company/panel.css
rtk git commit -m "feat: Needs you rail and the office source"
```

### Task 6: The building in three.js, and the demo

Starts with the `build-premium-website` skill, Three.js section. This task and the next give structure, constants and recipes. The exact numbers get tuned by looking, in Task 11.

The demo is what makes that looking safe. It is a separate dev page with no Tauri, no IPC and no engine, so it cannot touch `~/.bytehive/company`.

**Files:**
- Create: `office-demo.html`, `src/company/office/fixtures.ts`, `fixtures.test.ts`, `demo.ts`, `scene.ts`, `build.ts`, `tools/office_shots.mjs`
- Modify: `package.json`, `package-lock.json` (three)
- Test: `src/company/office/scene.test.ts`

**Interfaces:**
- Consumes: `OfficeModel`, `sceneKey` (Task 4), `Level`, `frameDelay`, `Tower`, `towerLayout`, `shotFor`, `mixShots`, `OFFICE` (Task 4), `OfficeSource`, `Snapshot` (Task 5).
- Produces, `scene.ts`:

```ts
export interface SceneOptions {
  reducedMotion: boolean;
  dpr: number;
  onLost(): void;       // context lost
  onRestored(): void;   // context back, picture drawn
}
export interface Anchor { id: string; kind: "floor" | "person" | "top"; x: number; y: number; visible: boolean } // CSS pixels in the canvas
export interface SceneStats { calls: number; triangles: number; frames: number; lost: number; restored: number; lastRestoreMs: number | null; drawW: number; drawH: number }
export interface OfficeScene {
  setModel(m: OfficeModel): void;
  setLevel(l: Level, instant: boolean): void;
  resize(w: number, h: number): void;
  setVisible(on: boolean): void;
  pick(x: number, y: number): { personId?: string; floorId?: string } | null;
  anchors(): Anchor[];
  onMoved(fn: () => void): void;  // the camera came to rest: labels may be placed
  stats(): SceneStats;
  dispose(): void;
}
export function goodRenderer(name: string): boolean;
export function webglProbe(allowSoftware?: boolean): { ok: boolean; renderer: string };
export function createScene(canvas: HTMLCanvasElement, o: SceneOptions): OfficeScene;
```

- Produces, `fixtures.ts`: `demoModel(o?: { empty?: boolean; quiet?: boolean; flight?: boolean; floors?: number }): OfficeModel`, `demoSource(o?): OfficeSource`.

**Design rule for the fallback:** `createScene` reads nothing from the DOM except the canvas it is given. Size arrives through `resize`, picking through coordinates, and motion settings through options. If the gate in Task 10 sends the scene to a Worker with an OffscreenCanvas, only the file that calls `createScene` changes. Do not call `document`, `window` or `getComputedStyle` in `scene.ts`, `build.ts` or `people.ts`, apart from `requestAnimationFrame` and `performance.now`, which Workers have.

- [ ] **Step 1: Pin three**

```bash
rtk npm install --save-exact three@0.170.0
rtk npm install --save-dev --save-exact @types/three@0.170.0
```

Check `package.json` has `"three": "0.170.0"` with no caret. 0.170.0 is the version Phase 0 measured on this WebView2.

- [ ] **Step 2: Write the failing fixture test**

```ts
// src/company/office/fixtures.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { demoModel, demoSource } from "./fixtures";

describe("the demo company", () => {
  const m = demoModel();
  const people = [m.founder!, ...m.floors.flatMap((f) => f.people)];
  it("shows every state, so one screenshot judges them all", () => {
    expect(new Set(people.map((p) => p.state))).toEqual(new Set(["needs", "working", "review", "idle", "stopped"]));
  });
  it("has a Lead, a person alone on a floor, a closed floor and an empty open one", () => {
    expect(m.floors.some((f) => f.lead)).toBe(true);
    expect(m.floors.some((f) => f.people.length === 1 && !f.lead)).toBe(true);
    expect(m.floors.some((f) => !f.open && f.people.length)).toBe(true);
    expect(m.floors.some((f) => f.open && !f.people.length)).toBe(true);
  });
  it("has every kind of row in Needs you", () => {
    expect(new Set(m.needs.map((r) => r.kind))).toEqual(new Set(["plan", "paused", "alert", "merge"]));
  });
  it("carries every prop and more than one CLI", () => {
    expect(new Set(people.map((p) => p.prop))).toEqual(new Set(["none", "laptop", "clipboard", "megaphone", "magnifier", "book", "pen", "pencil"]));
    expect(new Set(people.map((p) => p.cli)).size).toBeGreaterThan(1);
  });
  it("can be empty, or quiet", () => {
    expect(demoModel({ empty: true }).floors).toEqual([]);
    expect(demoModel({ quiet: true }).needs).toEqual([]);
  });
  it("does its actions in memory", async () => {
    const s = demoSource();
    await s.giveJob("Anything");
    expect(s.snapshot(0).model?.jobs.map((j) => j.title)).toContain("Anything");
  });
  it("reaches nothing real", () => {
    for (const f of ["fixtures.ts", "demo.ts"]) {
      const text = readFileSync(`src/company/office/${f}`, "utf8");
      expect(text).not.toMatch(/from "\.\.\/engine"|from "\.\/live"|\/ipc"|@tauri-apps/);
    }
  });
});
```

```ts
// src/company/office/scene.test.ts
import { describe, expect, it } from "vitest";
import { goodRenderer } from "./scene";

describe("goodRenderer", () => {
  it.each([
    ["ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0)", true],
    ["ANGLE (NVIDIA, NVIDIA GeForce RTX 2070 Direct3D11)", true],
    ["Google SwiftShader", false],
    ["ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)))", false],
    ["ANGLE (Microsoft, Microsoft Basic Render Driver Direct3D11)", false],
    ["llvmpipe (LLVM 15.0.7)", false],
  ])("%s is %s", (name, good) => { expect(goodRenderer(name)).toBe(good); });
});
```

`scene.ts` imports three, which loads fine in happy-dom: only this pure function is called.

- [ ] **Step 3: Write the fixtures**

`src/company/office/fixtures.ts` builds a real `Company` and calls `officeModel` on it, so the demo cannot drift from the model. Company: floors `bytehive` (Lead Hai, Linh, An, Minh, open), `gcare` (Lead Mai, Tuan on `codex`, open), `vanta` (Sol alone on `opencode`, open), `oldsite` (Bao, closed), `scratch` (open, nobody). Panes give states: Linh working, An working, Minh review, Hai needs, Mai idle, Tuan working, Sol stopped, Founder working. Jobs: J-0142 `awaiting-plan` with a plan of 2 hires and 3 cards, J-0141 `running`, J-0139 `paused`, J-0138 `awaiting-merge`. Alerts: one `Stuck` row naming Tuan. Boards: `bytehive` 2, 2, 5. Roles: the eight bundled ones as plain objects (ids, titles, departments, looks from `src/company/roles/*/role.json`). Give props so all eight values appear.

`demoSource` holds this in memory, makes `approve`, `reject`, `stopJob`, `giveJob` change it and fire listeners, `showPerson` returns true, `report` returns `deskReport` over a fixture board, `watching` does nothing.

Then `office-demo.html` at the repo root:

```html
<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Office demo</title>
<link rel="stylesheet" href="/src/styles/index.css"></head>
<body class="inbox-ui"><div id="demo" style="position:fixed;inset:0"></div>
<script type="module" src="/src/company/office/demo.ts"></script></body>
</html>
```

`vite build` builds only `index.html`, so this page never ships. Confirm with `rtk proxy npx vite build` and `dist/office-demo.html` absent.

`src/company/office/demo.ts` reads the URL and mounts a canvas with the scene (Task 8 swaps this for the shell):

| Parameter | Effect |
|---|---|
| `?level=building`, `floor:bytehive`, `desk:<personId>` | Starts there, instantly |
| `?empty=1`, `?quiet=1`, `?floors=8` | Fixture variants |
| `?flight=1` | A plane in the air, frozen at the middle of its path for screenshots |
| `?still=1` | No ambient motion |
| `?software=1` | Allows software WebGL, which headless Chrome uses. Only this page reads it. |

It sets `window.__office = { scene, stats: () => scene.stats() }`.

- [ ] **Step 4: Write the scene**

`src/company/office/scene.ts`, in this order:

1. `goodRenderer(name)`: false when `name` matches `/swiftshader|llvmpipe|basic render|software/i`. `webglProbe(allowSoftware = false)`: a throwaway `canvas.getContext("webgl2")`, reads `WEBGL_debug_renderer_info`, then `WEBGL_lose_context.loseContext()` on the throwaway so it does not hold a context. `ok` means a context was made and, unless software is allowed, `goodRenderer` says yes. Only the demo page passes `true`.
2. Renderer: `new WebGLRenderer({ canvas, antialias: true, powerPreference: "low-power", preserveDrawingBuffer: false })`, `setPixelRatio(Math.min(o.dpr, 1.5))`, `outputColorSpace = SRGBColorSpace`, `toneMapping = ACESFilmicToneMapping`, `toneMappingExposure = 1.05`. Shadows: `PCFSoftShadowMap`, one `DirectionalLight` with a 1024 map, `shadowMap.autoUpdate = false`, `needsUpdate = true` only after `setModel` changes people. `low-power` is what Phase 0 measured.
3. Loop, the whole of the on-demand rule:

```ts
function schedule(): void {
  if (pending !== null) return;
  const delay = frameDelay({ visible, moving: moving(), ambient: anyoneWorking, reduced: o.reducedMotion });
  if (delay === null && !dirty) return;
  const wait = Math.max(0, (delay ?? 0) - (performance.now() - lastDraw));
  pending = setTimeout(() => requestAnimationFrame(frame), wait) as unknown as number;
}
function frame(t: number): void {
  pending = null;
  if (lost) return;
  advance(t);           // camera tween, plane, ambient pose: all from time, nothing stored that matters
  renderer.render(scene, camera);
  lastDraw = performance.now();
  frames += 1;
  dirty = false;
  schedule();
}
```

`invalidate()` sets `dirty = true` and calls `schedule()`. `setVisible(false)` clears the timer. Any call that changes the picture goes through `invalidate`.

4. `setModel(m)`: `const key = sceneKey(m); if (key === lastKey) return;`, then rebuild the people groups and floors that changed. No other state survives: dropping the whole scene and calling `setModel` again gives the same picture. That is what "no state of its own that matters" means here, and Task 8's test checks it.
5. `setLevel(l, instant)`: target shot from `shotFor(l, tower, aspect)`. `instant || o.reducedMotion` jumps to it. Else tween 900 ms with `easeInOutCubic` from the current shot. `moving()` is true while the tween runs. At its end call the `onMoved` listeners.
6. `resize(w, h)`: `renderer.setSize(w, h, false)`, `camera.aspect`, `updateProjectionMatrix`, recompute the shot, `invalidate`. Task 8 calls it debounced, so the drawing buffer is reallocated once per change, not every step.
7. Context loss: `canvas.addEventListener("webglcontextlost", (e) => { e.preventDefault(); lost = true; lostAt = performance.now(); counts; o.onLost(); })`. On `webglcontextrestored`: `lost = false`, `shadowMap.needsUpdate = true`, draw one frame, `lastRestoreMs = performance.now() - lostAt`, `o.onRestored()`. three re-uploads geometry and textures by itself: Task 10 checks the picture really comes back.
8. `pick(x, y)`: one `Raycaster` against invisible box proxies, one per person and one per floor, in a `Group` not rendered (`layers.set(1)`, raycaster on layer 1).
9. `anchors()`: project each floor's right edge and each person's head with `Vector3.project(camera)` to CSS pixels. `visible` is false when behind the camera or off the canvas.

`src/company/office/build.ts`, meshes as in `style-a.webp`:

| Part | Recipe |
|---|---|
| Ground plate | `roundedSlab(ROOM_W + LIFT_W + 3, ROOM_D + 3, 0.36, 0.5)`, `OFFICE.ground`, receives shadow, a soft amber patch in front from a radial `CanvasTexture` (as in style-a) |
| Floor slab | `roundedSlab(ROOM_W + LIFT_W + 0.5, ROOM_D + 0.5, SLAB, 0.28)`, `OFFICE.shellOut`, top face `OFFICE.slabTop` |
| Back and left walls | Two `RoundedBox`-like extrusions 0.34 thick, outside `OFFICE.shellOut`. Front and right stay open: the cutaway. |
| Inner wall light | A plane just inside each wall with a `CanvasTexture` vertical gradient from `OFFICE.wallLit` at the ceiling to `OFFICE.wallDim`, `MeshBasicMaterial` so it glows without a light. A dark floor uses `OFFICE.wallDark`. This replaces per-floor point lights: 0 lights per floor. |
| Room floor | Plane `OFFICE.floorLit` or `floorDark`, a warm radial gradient under the desks |
| Ceiling strip | Small emissive box under each slab, `OFFICE.glow`, off on a dark floor |
| Desk | Top `BoxGeometry(1.5, 0.08, 0.8)` `OFFICE.deskTop`, two side blocks `OFFICE.deskBody`, monitor `BoxGeometry(0.62, 0.4, 0.05)` with an emissive screen plane |
| Chair | Seat, back and a five-star base as one merged geometry, `OFFICE.chair` |
| Wall board | Frame `BoxGeometry(2.6, 1.3, 0.06)` `OFFICE.deskTop`, cork plane, up to 12 sticky planes: count from `floor.board`, yellow for To do, blue for Doing, green for Done |
| Plant | One per floor with a free corner: pot cylinder `OFFICE.pot`, 5 cone leaves `OFFICE.plant` |
| Lift | Steel frame from `GROUND` to `lift.top`: 4 posts `OFFICE.steel`, a glass box `MeshPhysicalMaterial`-free: `MeshStandardMaterial` with `transparent`, `opacity: 0.16`, `OFFICE.glass`. A car box parked at the ground. A small lamp at each floor. A mast above the roof. |
| Roof deck | Railing of glass panels on 3 sides, the Founder's desk at the front, a small penthouse box with one lit window and a lit door behind it, as in style-a |

Merge all static parts of a floor by material with `mergeGeometries` from `three/examples/jsm/utils/BufferGeometryUtils.js`. Share one geometry and one material per kind across floors. Target: under 300 draw calls with 20 people, read from `stats().calls`.

`roundedSlab(w, d, h, r)` is an `ExtrudeGeometry` of a rounded rectangle `Shape`, `bevelEnabled: true`, `bevelSize: 0.05`, `bevelThickness: 0.05`, `bevelSegments: 2`, rotated flat. The chunky soft edges are most of the toy look: keep them.

Lights: `HemisphereLight(0xfff2d8, 0x1d2029, 1.4)`, `DirectionalLight(0xffe2b0, 1.6)` from the front left with the shadow map. `scene.background = new Color(OFFICE.sky)`.

- [ ] **Step 5: Write the screenshot tool**

`tools/office_shots.mjs`: starts nothing. Takes a URL base and an output folder in the session scratchpad, launches headless Chrome (`--headless=new --remote-debugging-port=9555 --enable-unsafe-swiftshader`) on a temp profile, drives it over CDP with Node's global `WebSocket` (the pattern in the headless screenshot memory), loads each `?level=` with `&software=1&still=1`, waits for `window.__office.stats().frames > 0`, and saves `Page.captureScreenshot` PNGs at 1280x800 and 1920x1080. It closes only the Chrome it started.

Use a port of its own so it cannot fight the other agent:

```bash
rtk proxy npx vite --port 17631 --strictPort
rtk node tools/office_shots.mjs http://localhost:17631/office-demo.html <scratchpad>/shots
```

- [ ] **Step 6: Run the tests and look**

Run: `rtk proxy npx vitest run src/company/office && rtk npx tsc --noEmit`, then the shots above.

Expected: tests pass. Building shot shows a stacked cutaway tower with a glass lift on the right, warm walls, charcoal shell. Read the PNG. Compare it beside `style-a.webp`. People are not there yet.

- [ ] **Step 7: Check three stays out of the first load**

```bash
rtk proxy npx vite build
```

Expected: a separate chunk holds three, and `rtk proxy grep -l WebGLRenderer dist/assets/*.js` does not name the entry chunk listed in `dist/index.html`. Nothing in the app imports `scene.ts` yet: Task 9 repeats this check once it does.

- [ ] **Step 8: Commit**

```bash
rtk git add package.json package-lock.json office-demo.html src/company/office tools/office_shots.mjs
rtk git commit -m "feat: the office building in three.js, with a demo page"
```

### Task 7: Characters and motion

Starts with the `build-premium-website` skill.

**Files:**
- Create: `src/company/office/people.ts`, `src/company/office/textures.ts`
- Modify: `src/company/office/scene.ts`, `src/clibrand.ts`, `src/hivelogo.ts` (only if the logo path is not already exported)
- Test: `src/clibrand.test.ts`, `src/company/office/people.test.ts`

**Interfaces:**
- Consumes: `OfficePerson`, `OfficeState`, `Flight`, `flightPoint`, `FLIGHT_MS`, `OFFICE` (Task 4). The scene never imports the engine.
- Produces, `clibrand.ts`: `cliLogoUrl(a: BrandSource): string | null`.
- Produces, `people.ts`: `pose(state: OfficeState, t: number, seat: number): Pose`, `buildPerson(p: OfficePerson, kit: Kit): Object3D`, `applyPose(obj: Object3D, pose: Pose): void`.

```ts
export interface Pose {
  sit: number;        // 0 standing, 1 seated
  lean: number;       // chair back, radians
  armL: number; armR: number; // arm swing, radians
  screen: "work" | "off" | "dim";
  lamp: boolean;
  bubble: boolean;
  present: boolean;   // false: an empty chair
}
```

- [ ] **Step 1: Write the failing tests**

```ts
// src/company/office/people.test.ts
import { describe, expect, it } from "vitest";
import { pose } from "./people";

describe("pose", () => {
  it("has a bubble for needs and only for needs", () => {
    for (const s of ["working", "review", "idle", "stopped"] as const) expect(pose(s, 0, 0).bubble).toBe(false);
    expect(pose("needs", 0, 0).bubble).toBe(true);
  });
  it("stands up with the lamp on when it needs you", () => {
    expect(pose("needs", 0, 0)).toMatchObject({ sit: 0, lamp: true });
  });
  it("types at a lit screen when working, moving with time", () => {
    const a = pose("working", 0, 0);
    const b = pose("working", 125, 0);
    expect(a.screen).toBe("work");
    expect(a.armL).not.toBe(b.armL);
  });
  it("does not type in step with the next desk", () => {
    expect(pose("working", 300, 0).armL).not.toBe(pose("working", 300, 1).armL);
  });
  it("leans back when idle, and holds still", () => {
    expect(pose("idle", 0, 0).lean).toBeGreaterThan(0);
    expect(pose("idle", 0, 0)).toEqual(pose("idle", 5000, 0));
  });
  it("leaves the chair empty and the screen off when stopped", () => {
    expect(pose("stopped", 0, 0)).toMatchObject({ present: false, screen: "off", bubble: false, lamp: false });
  });
});
```

Add to `src/clibrand.test.ts`:

```ts
describe("cliLogoUrl", () => {
  it("gives a known CLI's logo file", () => {
    expect(cliLogoUrl({ badge: "claude" })).toMatch(/claude/);
  });
  it("gives nothing for a plain shell or an unknown CLI", () => {
    expect(cliLogoUrl({ badge: "shell" })).toBeNull();
    expect(cliLogoUrl({ badge: "nope" })).toBeNull();
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/office/people.test.ts src/clibrand.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement the pure parts**

In `src/clibrand.ts`:

```ts
/** A CLI's logo file, for drawing it somewhere other than HTML. */
export function cliLogoUrl(a: BrandSource): string | null {
  const b = brandOf(a);
  return b ? LOGOS[`./assets/clis/${b.key}.png`] ?? null : null;
}
```

In `src/company/office/people.ts`:

```ts
const STILL: Pose = { sit: 1, lean: 0, armL: 0, armR: 0, screen: "dim", lamp: false, bubble: false, present: true };

/** A character's whole picture from its state and the clock. Nothing is kept between frames. */
export function pose(state: OfficeState, t: number, seat: number): Pose {
  switch (state) {
    case "needs": return { ...STILL, sit: 0, lamp: true, bubble: true };
    case "working": {
      const phase = t / 125 + seat * 1.7;
      return { ...STILL, screen: "work", armL: Math.sin(phase) * 0.18, armR: Math.sin(phase + 2) * 0.18 };
    }
    case "review": return { ...STILL, armR: 0.5 };
    case "idle": return { ...STILL, lean: 0.32 };
    default: return { ...STILL, present: false, screen: "off" };
  }
}
```

- [ ] **Step 4: Build the characters**

`buildPerson` makes one `Group` from primitives, sized so a head reads at the Building shot, like the chunky bead heads in style-a:

| Part | Recipe |
|---|---|
| Head | `SphereGeometry(0.34, 24, 18)`, `OFFICE.bead`. Oversized on purpose. |
| Hair | Half sphere cap `SphereGeometry(0.355, 24, 12, 0, 2π, 0, π/2)`, `p.hair`, tilted back a little |
| Face | Two tiny dark spheres for eyes. No mouth at the Building shot: a small curve is enough at Desk. |
| Body | `CapsuleGeometry(0.24, 0.34, 6, 14)`, `p.shirt` |
| Arms | Two capsules 0.09 radius, `p.shirt`, pivots at the shoulders, driven by `armL`, `armR` |
| Legs | Two short capsules, dark, seated or standing by `sit` |
| Badge | `PlaneGeometry(0.17, 0.17)` on the chest with the CLI logo texture, on a small white card. No logo: a letter drawn on a 64px `CanvasTexture`. |
| Rank mark | A Lead gets a slim honey band on the left arm. The Founder has a honey shirt. |

Prop by `p.prop`, held in the right hand or set on the desk:

| Prop | Recipe | Where |
|---|---|---|
| `laptop` | Thin box and a tilted lid | On the desk |
| `clipboard` | Box 0.2 by 0.28, a paper plane on it, a small clip | Left hand |
| `megaphone` | `ConeGeometry` open end out, honey ring | Right hand |
| `magnifier` | `TorusGeometry` and a short cylinder handle | Right hand |
| `book` | Two boxes as an open book | On the desk |
| `pen`, `pencil` | Thin cylinder with a coloured tip | Right hand |
| `none` | Nothing | |

Bubble: a honey `roundedSlab`-like pill above the head with a dark `!` built from two meshes (a box and a sphere), and a small tail. Built only when `pose.bubble`. It is the only thing above a head in the whole scene.

Desk state: `screen: "work"` uses `OFFICE.screenOn` with emissive 1.1, `"dim"` a low emissive, `"off"` `OFFICE.screenOff`. `lamp: true` shows a small desk lamp, emissive `OFFICE.lampNeeds`.

Review papers: on the Lead's desk, or the person's own on a floor with no Lead, a stack of `floor.papers` thin boxes in `OFFICE.paper`.

`src/company/office/textures.ts`:

- `logoTexture()`: renders the honey cell from `design/logo.svg`, unchanged, into a 512px canvas through an `Image` with a data URL, into a `CanvasTexture` with `anisotropy: 4`. Copy the SVG source as a string constant: it stays byte for byte the logo. The roof sign is a thick hexagon plate with this on its front, on a short post at the front corner of the roof, in the place the mockup shows it. Lit by `MeshBasicMaterial` so it reads at night.
- `badgeTexture(cli)`: `TextureLoader` on `cliLogoUrl({ badge: cli })`, cached per CLI, drawn as loaded, which calls `invalidate`.

Paper plane: three triangles in one `BufferGeometry`, `OFFICE.paper`. `advance(t)` positions one per `model.flights` entry with `flightPoint(tower, floorOf(from), floorOf(to), (now - at) / FLIGHT_MS)`, nose pointing along the path. `moving()` is true while any flight is in the air. With `o.reducedMotion` the plane is drawn still at its destination until `FLIGHT_MS` passes. Same floor: no plane.

- [ ] **Step 5: Run the tests and look**

Run: `rtk proxy npx vitest run src/company/office src/clibrand.test.ts && rtk npx tsc --noEmit`, then the shots from Task 6 Step 5, plus `?level=floor:bytehive` and `?flight=1`.

Expected: at Building you can tell who needs you and who is working, without a label. At Floor, Hai stands with a bubble and an amber lamp, Linh and An type at lit screens, Minh has paper on the Lead's desk, Mai's chair leans back. Roof logo is the honey cell.

- [ ] **Step 6: Commit**

```bash
rtk git add src/clibrand.ts src/clibrand.test.ts src/company/office
rtk git commit -m "feat: office characters, props, bubble and paper plane"
```

### Task 8: Office shell and the three levels

Starts with the `build-premium-website` skill. This is the task that decides whether the office is usable, so judge it by using it.

**Files:**
- Create: `src/company/office/shell.ts`, `shell.test.ts`
- Modify: `src/company/office/office.css`, `src/company/office/demo.ts`, `src/inbox.ts`

**Interfaces:**
- Consumes: `OfficeSource`, `createRail`, `createJobBox` (Task 5), `Level`, `crumbs`, `navUp`, `clampLevel`, `escapeGoesUp`, `rowTarget` (Task 4), `OfficeScene`, `createScene` (Task 6).
- Produces, `shell.ts`:

```ts
export interface ShellHost {
  /** Put a person's real pane on the stage and open a tab there. */
  showDesk(personId: string, tab: DeskTab): boolean;
  /** Called when the level changes: the app shows or hides what sits under the layer. */
  levelChanged(l: Level): void;
  /** Leave the office for the Inbox: the owner asked to. */
  leave(): void;
  /** The scene cannot run: no context, a failed import, or a context that stayed lost. */
  failed(why: string): void;
  loadScene(): Promise<typeof import("./scene")>;
}
export function createShell(source: OfficeSource, host: ShellHost): {
  el: HTMLElement;
  show(l?: Level): void;
  hide(): void;
  level(): Level;
  go(l: Level): void;
  escape(ctx: { terminal: boolean; dialog: boolean; dock: boolean; typing: boolean }): boolean; // true when it went up
  sceneFailed(why: string): void;
  dispose(): void;
};
```

- Produces, `inbox.ts`: `stageAgent(paneId: string): boolean`, `openChanges(paneId: string): void`, `closeChanges(): void`, `changesOpenFor(): string | null`.

**Layout.** `#office` is a fixed layer. `z-index: 120`: above `.topbar` (100), under the hint (180), dock panel (290), modals (300) and window controls (300).

| Level | Layer | Content |
|---|---|---|
| Building | Full window | Bar on top (breadcrumb left, Inbox button right, `padding-right: 150px` for the window controls, `data-tauri-drag-region`). Canvas fills the layer. Rail on the right, 320px, panel surface. Job box bottom left, over the canvas, on a solid panel. Floor tags beside each floor. |
| Floor | Full window | Same bar and rail. Name tags over heads. Bottom left: message box `Message Hai` (the Lead), or `Message Sol` (one person alone), hidden on an empty floor where a line says `Nobody here yet.` Board chip beside the wall board: `To do 2, Doing 2, Done 5`, click opens the Board tool for that project. |
| Desk | Left column, `--office-desk-w: clamp(340px, 34vw, 520px)` | Bar, canvas in the top part framing the floor, rail below it. On the right is the app itself: `body.office-desk` sets `--ib-q` to the column width, hides `.inbox-queue`, `.inbox-dock` and `.inbox-head`, and sets `--ib-dock-h: 16px`. Tabs `Terminal`, `Changes`, `Report` go in `.tb-center`, in a tablist. |

Nothing moves between hosts. At Desk the stage is simply no longer covered.

Going from Floor to Desk: the layer's width animates 240ms with a CSS transition while the canvas stretches, and `scene.resize` runs once when it ends, through a `ResizeObserver` debounced 120ms. With reduced motion no transition. `focusPane` fits the terminal once the width has settled: call `term.fit` again from `transitionend`, or it fits the old width.

- [ ] **Step 1: Write the failing tests**

```ts
// src/company/office/shell.test.ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { demoSource } from "./fixtures";
import { BUILDING } from "./nav";
import { createShell, type ShellHost } from "./shell";

const scene = vi.hoisted(() => ({
  setModel: vi.fn(), setLevel: vi.fn(), resize: vi.fn(), setVisible: vi.fn(), pick: vi.fn(() => null),
  anchors: vi.fn(() => []), onMoved: vi.fn(), stats: vi.fn(), dispose: vi.fn(),
}));
const sceneModule = { createScene: vi.fn(() => scene), webglProbe: () => ({ ok: true, renderer: "test" }) };

function setup(over: Partial<ShellHost> = {}) {
  const host: ShellHost = {
    showDesk: vi.fn(() => true), levelChanged: vi.fn(), leave: vi.fn(), failed: vi.fn(),
    loadScene: vi.fn(async () => sceneModule as never), ...over,
  };
  const source = demoSource();
  const shell = createShell(source, host);
  document.body.appendChild(shell.el);
  return { host, source, shell };
}
const calm = { terminal: false, dialog: false, dock: false, typing: false };
const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => { document.body.innerHTML = ""; vi.clearAllMocks(); });

describe("the office shell", () => {
  it("does not load the scene until it is shown", async () => {
    const { host, shell } = setup();
    expect(host.loadScene).not.toHaveBeenCalled();
    shell.show();
    await tick();
    expect(host.loadScene).toHaveBeenCalledTimes(1);
  });
  it("hands the scene the model and the level", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    expect(scene.setModel).toHaveBeenCalled();
    expect(scene.setLevel).toHaveBeenLastCalledWith(BUILDING, true);
  });
  it("gives the scene the same model again after a redraw, so it holds nothing of its own", async () => {
    const { shell, source } = setup();
    shell.show();
    await tick();
    const first = scene.setModel.mock.lastCall![0];
    await source.approve("nothing");
    expect(JSON.stringify(scene.setModel.mock.lastCall![0])).toBe(JSON.stringify(first));
  });
  it("shows the job box at the building only, and the message box at a floor only", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    expect(shell.el.querySelector("[data-job-box]")?.hasAttribute("hidden")).toBe(false);
    expect(shell.el.querySelector("[data-msg-box]")?.hasAttribute("hidden")).toBe(true);
    shell.go({ kind: "floor", floorId: "bytehive" });
    expect(shell.el.querySelector("[data-job-box]")?.hasAttribute("hidden")).toBe(true);
    expect(shell.el.querySelector("[data-msg-box]")?.hasAttribute("hidden")).toBe(false);
    expect(shell.el.querySelector("[data-msg-box] label")?.textContent).toBe("Message Hai");
  });
  it("keeps Needs you at all three levels", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    for (const l of [BUILDING, { kind: "floor", floorId: "bytehive" } as const, { kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" } as const]) {
      shell.go(l);
      expect(shell.el.querySelector("[data-needs]")?.children.length).toBeGreaterThan(0);
    }
  });
  it("walks the breadcrumb back up", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    const labels = () => [...shell.el.querySelectorAll("[data-crumb]")].map((c) => c.textContent);
    expect(labels()).toEqual(["ByteHive", "bytehive", "Linh"]);
    shell.el.querySelector<HTMLElement>('[data-crumb="1"]')!.click();
    expect(shell.level()).toEqual({ kind: "floor", floorId: "bytehive" });
  });
  it("opens the person's real pane at Desk", async () => {
    const { shell, host } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    expect(host.showDesk).toHaveBeenCalledWith("linh", "terminal");
    expect(host.levelChanged).toHaveBeenLastCalledWith({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
  });
  it("offers to start someone with no pane", async () => {
    const { shell, source } = setup({ showDesk: vi.fn(() => false) });
    const wake = vi.spyOn(source, "wakePerson");
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "bao", floorId: "oldsite", tab: "terminal" });
    const b = shell.el.querySelector<HTMLElement>("[data-wake]")!;
    expect(b.textContent).toBe("Start Bao");
    b.click();
    expect(wake).toHaveBeenCalledWith("bao");
  });
  it("goes up one level on Esc, not past the building", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "floor", floorId: "bytehive" });
    expect(shell.escape(calm)).toBe(true);
    expect(shell.level()).toEqual(BUILDING);
    expect(shell.escape(calm)).toBe(false);
  });
  it("leaves Esc to a focused terminal", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    expect(shell.escape({ ...calm, terminal: true })).toBe(false);
    expect(shell.level().kind).toBe("desk");
  });
  it("uses a button for every person and floor, for the keyboard", async () => {
    scene.anchors.mockReturnValue([{ id: "bytehive", kind: "floor", x: 10, y: 10, visible: true }] as never);
    const { shell } = setup();
    shell.show();
    await tick();
    const tag = shell.el.querySelector<HTMLButtonElement>('button[data-tag="bytehive"]')!;
    expect(tag.textContent).toContain("bytehive");
    tag.click();
    expect(shell.level()).toEqual({ kind: "floor", floorId: "bytehive" });
  });
  it("falls back to its level when a person leaves", async () => {
    const { shell, source } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    vi.spyOn(source, "snapshot").mockReturnValue({ model: { ...source.snapshot(0).model!, floors: [] }, note: null, error: null });
    await source.approve("nothing");
    expect(shell.level()).toEqual(BUILDING);
  });
  it("reports a scene that cannot be made", async () => {
    sceneModule.createScene.mockImplementationOnce(() => { throw new Error("no context"); });
    const { shell, host } = setup();
    shell.show();
    await tick();
    expect(host.failed).toHaveBeenCalledWith("no context");
  });
  it("reports a three.js chunk that will not load", async () => {
    const { shell, host } = setup({ loadScene: vi.fn(async () => { throw new Error("offline"); }) });
    shell.show();
    await tick();
    expect(host.failed).toHaveBeenCalledWith("offline");
  });
  it("says where the Company runs when there is no model", async () => {
    const { shell, source } = setup();
    vi.spyOn(source, "snapshot").mockReturnValue({ model: null, note: "The Company is running in another ByteHive window.", error: null });
    shell.show();
    await tick();
    expect(shell.el.textContent).toContain("another ByteHive window");
  });
});
```

The fixture ids used here are lower case first names: `linh`, `bao`, `hai`. `demoSource().approve("nothing")` must fire its listeners even for an unknown Job, so these tests can force a redraw.

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/office/shell.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement the shell**

`createShell` builds this once:

```html
<div id="office" class="of" hidden>
  <header class="of-bar" data-tauri-drag-region>
    <nav class="of-crumbs" aria-label="Where you are" data-crumbs></nav>
    <button type="button" class="of-inbox" data-leave>Inbox</button>
  </header>
  <div class="of-stage" data-stage>
    <canvas class="of-canvas" tabindex="-1" aria-hidden="true"></canvas>
    <div class="of-tags" data-tags></div>
    <p class="of-note" data-note hidden></p>
  </div>
  <div class="of-job" data-job-box></div>
  <form class="of-msg" data-msg-box hidden>
    <label for="ofMsg"></label><textarea id="ofMsg" rows="2"></textarea><button type="submit">Send</button>
  </form>
  <aside class="of-rail" aria-label="Needs you" data-rail></aside>
</div>
```

Rules, each covered by a test above or a check in Step 5:

- `draw()`: `const snap = source.snapshot(Date.now())`. No model: show `note ?? error ?? "Starting..."` in `[data-note]`, and return. Else `level = clampLevel(level, model)`, `scene?.setModel(model)`, update crumbs, tags, boxes, rail. `source.onChange(draw)` plus a 1 s timer while visible, for plane expiry.
- Tags are real `<button>`s in `.of-tags`, absolutely placed from `scene.anchors()`, positioned on `onMoved` and after a resize, and hidden while the camera moves. Floor tag: `<b>bytehive</b> 4 people`, plus `2 need you` in honey when `floor.needs` is above 0, plus `closed` on a dark floor. Person tag at Floor: `Hai <span>Lead, needs you</span>`, one line, ellipsis over 18 characters. Top: `Founder`. At Desk no tags, only the person's own name in the crumb.
- A canvas click runs `scene.pick`. A person goes to their Desk, a floor to its Floor. The top floor goes straight to the Founder's Desk. Pointer cursor from a throttled `pick` on `pointermove`, at most every 80ms, only at rest.
- `go(l)`: `const was = level; level = l; scene?.setLevel(l, false); body.classList.toggle("office-desk", l.kind === "desk"); host.levelChanged(l); if (l.kind === "desk") host.showDesk(l.personId, l.tab)`, and move focus to the current crumb, except at Desk, where `focusPane` gives it to the terminal.
- No pane at Desk (`showDesk` false): a solid card in the stage area with `Linh is not running.` and a button `Start Linh`. On a dark floor the line is `gcare is closed. Starting Bao opens it again.` `source.wakePerson`, then `showDesk` again. Errors show in the card.
- Tabs live in `.tb-center`, built by `shell` while at Desk: `<div role="tablist" aria-label="Desk">` with three `role="tab"` buttons. `Terminal` closes Changes and Report. `Changes` calls `openChanges(paneId)`. `Report` shows `.of-report`, a scrolling panel over `.main`, from `source.report(personId)`: branch, then each card with Job id, list, summary, checklist with ticks, files. Empty: `Nothing to report yet.`. Left and Right arrows move between tabs.
- A `ResizeObserver` on `[data-stage]`, debounced 120ms, calls `scene.resize(w, h)`.
- `hide()`: `scene.setVisible(false)`, `source.watching(false)`, remove the `office-desk` class and the tabs. The scene stays in memory: reopening is instant.
- `sceneFailed(why)`: `scene?.dispose(); scene = null; host.failed(why)`. The same for a rejected `loadScene` and a throwing `createScene`, both caught in `show`.
- The scene gets `reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches`, followed on its `change` event, and `dpr: devicePixelRatio`. `document.addEventListener("visibilitychange", ...)` calls `scene.setVisible(visible && !document.hidden)`. A window that is only unfocused keeps drawing: the owner may be watching it on a second screen, and at rest it draws nothing anyway.

`office.css`: every colour from the tokens `--ib-*` or the values in `palette.ts`. Use the app's `--sans`, `--mono`. No gradients on UI surfaces, no glow on text. The scene is the picture; the chrome stays quiet around it, the same restraint as the Inbox. `:focus-visible` uses a 2px honey ring.

In `src/inbox.ts`:

```ts
/** Put an agent on the stage, full size. For the office's Desk. */
export function stageAgent(paneId: string): boolean {
  const t = taskOf(paneId);
  if (!t) return false;
  if (splitOn) setSplit(false);
  openTask(t);
  return true;
}

export function openChanges(paneId: string): void {
  if (stagePane()?.id !== paneId && !stageAgent(paneId)) return;
  if (reviewer?.paneId !== paneId) openReview();
}
export function closeChanges(): void { if (reviewer?.paneId) reviewer.close(); }
export const changesOpenFor = (): string | null => reviewer?.paneId ?? null;
```

The Changes tab follows the drawer: on each `draw`, if the tab says Changes and `changesOpenFor()` is null, the owner closed it, so the tab goes back to Terminal.

Update `demo.ts` to mount `createShell(demoSource(params), demoHost)`. `demoHost.showDesk` returns true and shows a fake terminal block on the right so Desk can be judged: dark panel, mono text, the same width as a real stage.

- [ ] **Step 4: Run tests and the type check**

Run: `rtk proxy npx vitest run src/company/office src/inbox.test.ts && rtk npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 5: Use it**

Open `http://localhost:17631/office-demo.html?software=1` in a browser through the Playwright tools. Click a floor, a person, a crumb, press Esc, Tab through tags, resize to 1280x720 and 2560x1440, turn on reduced motion with `browser_emulate_media`. Expected: nothing overlaps at 1280x720, the rail scrolls by itself, no text sits on the scene without a chip, reduced motion cuts the camera.

- [ ] **Step 6: Commit**

```bash
rtk git add src/company/office src/inbox.ts
rtk git commit -m "feat: the office shell: building, floor, desk"
```

### Task 9: Home screen, Ctrl+Shift+O and the fallback

Starts with the `build-premium-website` skill. Replaces the dock panel. This is where the constraint "people who never used the Company see no change" is won or lost.

**Files:**
- Create: `src/company/office/office.ts`, `office.test.ts`
- Modify: `src/main.ts`, `src/dock.ts`, `src/inboxhelp.ts`, `src/bridges.ts:335`, `src/inbox.ts`, `src/company/engine.ts`
- Delete: `src/company/panel.ts`, `src/company/panel.css`

**Interfaces:**
- Consumes: `createShell`, `ShellHost` (Task 8), `liveSource` (Task 5), `demoSource` (Task 6), `homeView`, `companyUsed` (Task 4), `stageAgent`, `openChanges`, `closeChanges` (Task 8).
- Produces, `office.ts`:

```ts
export interface OfficeDeps { showApp(): void; showHome(): void }
export function initOffice(deps: OfficeDeps): void;     // no DOM, no three.js
export function officeBoot(): void;                     // after the first showView
export function officeToggle(): void;                   // Ctrl+Shift+O
export function officeGoHome(): boolean;                // the house icon: true when the office took it
export function officeVisible(): boolean;
export function officeReveal(paneId: string): void;     // Ctrl+K or a notification picked an agent
export const OFFICE_3D: boolean;                        // false turns the scene off for everyone
```

**The rules, one by one:**

| Situation | What happens |
|---|---|
| No Job has ever existed, app starts | Exactly today: `showView()`. `#office` does not exist in the DOM, `office.ts` created nothing, three.js is not fetched. |
| No Job, Ctrl+Shift+O | The office opens at Building with the job box: that key opened the Company before and still does. It needs three.js the first time, so it is fetched then. |
| Job exists, app starts | Office at Building. The key `maestro.office.used` in localStorage, set once a Job has been seen, decides this before the engine has loaded, so there is no flash of the Inbox behind the splash. |
| Job exists, house icon | Office at Building |
| Office, Inbox button or Ctrl+Shift+O | Office closes to `showView()`: Inbox with projects open, Start with none. |
| Inbox, Ctrl+Shift+O | Opens the office at the level it was left |
| Ctrl+K picks a member from the office | That person's Desk |
| Ctrl+K picks a non-member from the office | Office closes, Inbox shows them |
| Notification click, same | Same |
| Detached window | No office. Ctrl+Shift+O shows `The Company runs in the main window.` through `topNote`. |
| Company runs in another app | `homeView` says Inbox. Ctrl+Shift+O opens the office layer with the note and the Inbox button. |
| WebGL missing, software only, three.js fails to load, or `createScene` throws | Fallback |
| Context lost, not back in 2 seconds | Fallback |
| Context lost, back within 2 seconds | Nothing changes, the picture returns |

**Fallback:** `body.office-fallback`, `topNote("The 3D office could not start, so you are in the Inbox.")` once per run, rail and job box mounted into `#app` as a fourth grid column:

```css
body.office-fallback.inbox-ui .app { grid-template-columns: var(--ib-q) 1fr var(--code-w, 380px) 300px; }
body.office-fallback .of-side { grid-column: 4; grid-row: 2; margin: 0 16px var(--ib-dock-h) 0; overflow-y: auto; }
```

Row clicks go to `stageAgent`. The fallback is only for people who used the Company: `homeView` already keeps it out of everyone else's way. Ctrl+Shift+O in the fallback tries the scene once more, so a restored context or a driver update can recover without a restart. With `OFFICE_3D` false it does not try.

- [ ] **Step 1: Write the failing tests**

```ts
// src/company/office/office.test.ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({
  used: false,
  probe: { ok: true, renderer: "ANGLE (Intel)" },
  sceneLoads: 0,
  sceneThrows: false,
  notes: [] as string[],
  elsewhere: null as string | null,
  pane: null as string | null,
}));
vi.mock("./live", () => ({ liveSource: () => mockSource() }));
vi.mock("./scene", () => {
  io.sceneLoads += 1;
  return {
    webglProbe: () => io.probe,
    createScene: () => {
      if (io.sceneThrows) throw new Error("no context");
      return { setModel() {}, setLevel() {}, resize() {}, setVisible() {}, pick: () => null, anchors: () => [], onMoved() {}, stats: () => ({}), dispose() {} };
    },
  };
});
vi.mock("../../hint", () => ({ topNote: (t: string) => io.notes.push(t) }));
vi.mock("../../inbox", () => ({ stageAgent: () => true, openChanges() {}, closeChanges() {}, changesOpenFor: () => null }));
vi.mock("../../dock", () => ({ dockIsOpen: () => false }));
vi.mock("../engine", () => ({ companyUsedNow: () => io.used, companyElsewhere: () => io.elsewhere, onCompanyChange: () => () => {} }));

function mockSource() {
  return {
    snapshot: () => ({ model: { founder: null, floors: [], needs: [], jobs: [], flights: [], counts: { people: 0, working: 0, needs: 0 } }, note: null, error: null }),
    onChange: () => () => {}, watching() {}, showPerson: () => true,
  };
}

async function fresh() {
  vi.resetModules();
  document.body.innerHTML = '<div id="app"></div>';
  localStorage.clear();
  const showApp = vi.fn();
  const showHome = vi.fn();
  const m = await import("./office");
  m.initOffice({ showApp, showHome });
  return { m, showApp, showHome };
}
/** Dynamic imports take a few turns of the loop, even mocked. */
const settle = async () => { for (let i = 0; i < 20; i += 1) await new Promise((r) => setTimeout(r, 0)); };

beforeEach(() => { io.used = false; io.probe = { ok: true, renderer: "ANGLE (Intel)" }; io.sceneThrows = false; io.notes = []; io.elsewhere = null; io.sceneLoads = 0; });

describe("someone who never used the Company", () => {
  it("gets no office element and no three.js at start", async () => {
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.getElementById("office")).toBeNull();
    expect(io.sceneLoads).toBe(0);
    expect(m.officeVisible()).toBe(false);
  });
  it("keeps the house icon as it is", async () => {
    const { m } = await fresh();
    expect(m.officeGoHome()).toBe(false);
  });
  it("sees no fallback note", async () => {
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(io.notes).toEqual([]);
    expect(document.body.classList.contains("office-fallback")).toBe(false);
  });
});

describe("once the Company has been used", () => {
  it("makes the office the home screen", async () => {
    io.used = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(true);
    expect(m.officeGoHome()).toBe(true);
  });
  it("knows that before the engine has loaded, from the last run", async () => {
    const first = await fresh();
    localStorage.setItem("maestro.office.used", "1");
    first.m.officeBoot();
    await settle();
    expect(first.m.officeVisible()).toBe(true);
  });
  it("toggles between the office and the Inbox", async () => {
    io.used = true;
    const { m, showApp } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    expect(m.officeVisible()).toBe(false);
    expect(showApp).toHaveBeenCalled();
    m.officeToggle();
    await settle();
    expect(m.officeVisible()).toBe(true);
  });
  it("falls back to the Inbox with Needs you when WebGL is missing", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
    expect(document.body.classList.contains("office-fallback")).toBe(true);
    expect(document.querySelector("#app .of-side [data-needs]")).not.toBeNull();
    expect(io.notes).toEqual(["The 3D office could not start, so you are in the Inbox."]);
  });
  it("treats a software renderer as missing", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "Google SwiftShader" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.body.classList.contains("office-fallback")).toBe(true);
  });
  it("falls back when the scene cannot be made", async () => {
    io.used = true;
    io.sceneThrows = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.body.classList.contains("office-fallback")).toBe(true);
  });
  it("tries the scene again on the key, from the fallback", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    io.probe = { ok: true, renderer: "ANGLE (Intel)" };
    m.officeToggle();
    await settle();
    expect(m.officeVisible()).toBe(true);
    expect(document.body.classList.contains("office-fallback")).toBe(false);
  });
});

describe("with the Company in another app", () => {
  it("stays on the Inbox at start", async () => {
    io.used = true;
    io.elsewhere = "The Company is running in another ByteHive window.";
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
  });
});
```

Add to `src/company/engine.ts`: `export const companyUsedNow = () => !!state && companyUsed(state.jobs);`.

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/office/office.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement `office.ts`**

```ts
// src/company/office/office.ts
/* The office in the app: when it shows, what it replaces, and what happens without WebGL. */
import { topNote } from "../../hint";
import { changesOpenFor, closeChanges, openChanges, stageAgent } from "../../inbox";
import { companyElsewhere, companyUsedNow } from "../engine";
import { homeView, type Level } from "./nav";
import { createJobBox, createRail } from "./rail";
import { createShell } from "./shell";
import type { OfficeSource } from "./source";

/** Turn the 3D scene off for everyone. Task 10's gate is what can set this false. */
export const OFFICE_3D = true;

const USED_KEY = "maestro.office.used";
/** How long a lost context gets before the Inbox takes over. */
const RESTORE_MS = 2000;

export interface OfficeDeps { showApp(): void; showHome(): void }

let deps: OfficeDeps;
let shell: ReturnType<typeof createShell> | null = null;
let source: OfficeSource | null = null;
let visible = false;
let fallback = false;
let last: Level | undefined;
```

The rest, in order:

- `sourceNow()`: `import.meta.env.DEV && import.meta.env.VITE_OFFICE_DEMO ? (await import("./fixtures")).demoSource() : (await import("./live")).liveSource()`. The `DEV &&` keeps fixtures out of a release: Vite replaces it at build time and drops the branch. Check that in Step 7.
- `used()`: `companyUsedNow() || safeGet(USED_KEY) === "1"`. Every read and write of localStorage is in `try`, as the rest of the app does. When `companyUsedNow()` turns true, `onCompanyChange` writes the key and `officeBoot` logic runs once: a first Job given from the panel turns the office on as the home screen the next time the house icon or the app is used, without yanking the screen now.
- `officeBoot()`: `if (!used()) return;` then `homeView({ used: true, webgl: true, elsewhere: !!companyElsewhere() })`, and `open()`.
- `open()`: `OFFICE_3D` false or probe bad: `toFallback()`. The probe comes from `await import("./scene")`, which is the first time three.js is fetched. Never before `used()` or the key. Catch a rejected import: `toFallback()`.
- `toFallback()`: only when `used()`. Mounts `.of-side` in `#app` with `createJobBox` and `createRail`, class on body, note once.
- `officeReveal(paneId)`: if not visible, return. If the pane's person exists in the model, `shell.go` to their Desk, else `close()`.
- At start the engine has not read the lock yet, so `companyElsewhere()` is null and the office may open. If it then turns true while the office was opened by `officeBoot` and not by the key, `close()`: the Inbox is where that app belongs.
- `ShellHost.failed(why)`: log it with `console.error`, then `toFallback()`.
- `ShellHost.showDesk(personId, tab)`: `source.showPerson(personId)` then `deps.showApp()`, and `tab === "changes"` opens Changes for that pane, else `closeChanges()`.
- `onLost`: `window.setTimeout(() => { if (lost) shell.sceneFailed("lost"); }, RESTORE_MS)`, cleared by `onRestored`.
- Keys, on `document` in the capture phase:

```ts
function onKey(e: KeyboardEvent): void {
  if (e.key !== "Escape" || !visible || !shell) return;
  const at = document.activeElement as HTMLElement | null;
  const went = shell.escape({
    terminal: !!at?.closest(".xterm"),
    dialog: !!document.querySelector(".backdrop.open, .inbox-modal-back, .inbox-pal"),
    dock: dockIsOpen(),
    typing: !!at?.closest("input, textarea, select"),
  });
  if (went) { e.preventDefault(); e.stopPropagation(); }
}
```

Esc in the job box or message box blurs it first, so a second Esc goes up.

- [ ] **Step 4: Wire it in**

`src/main.ts`: replace `dockToggle("company")` on line 418 with `officeToggle()`. After `initDock()`: `if (!isDetachedWindow) initOffice({ showApp: () => { showWorkspace(); }, showHome: showView });`. After the `showView();` on line 473: `if (!isDetachedWindow) officeBoot();`. On `btnHome`: `() => { if (!officeGoHome()) goHome(); }`. In a detached window `officeToggle` is a call to `topNote("The Company runs in the main window.")`.

`showDesk` only reaches `showApp` for a person with a pane, so a project is open and `#app` has something to show.

`src/dock.ts`: remove `createCompanyPanel`, `"company"` from `ToolId`, `TITLES`, `TOOL_IDS`, and `tools.company`.

`src/inboxhelp.ts`: `openTool("company")` calls `officeToggle()`, and line 103 loses `"company"`. Add to `HELP`, under Anywhere: `{ keys: [["Ctrl", "Shift", "O"]], what: "The office: your company, floor by floor. Press it again for the Inbox." }`, and change the Company tool's `what` to `"Give the Founder a job and see the office."`. Update `inboxhelp.test.ts` for the `dock.opened` expectation: `company` no longer goes to the dock.

`src/inbox.ts`: in the palette `agents` map, `run: () => { officeReveal(t.paneId); openTask(t); }`. In `startFinishWatch`, `open:` the same.

`inbox.ts` and `inboxhelp.ts` now import `office.ts`, which imports `inbox.ts` back. Both uses are at run time, inside functions, so the cycle is harmless, but their tests must not load the office: add `vi.mock("./company/office/office", () => ({ officeReveal() {}, officeToggle() {} }));` to `src/inbox.test.ts`, `src/inboxstage.test.ts` and `src/inboxhelp.test.ts`.

`src/bridges.ts:335`:

```ts
  // Main window only: a detached window would read the same requests and, not
  // seeing the people's panes, start everyone a second time. The office demo has
  // no company at all, so a dev app can show it without touching one.
  const officeDemo = import.meta.env.DEV && !!import.meta.env.VITE_OFFICE_DEMO;
  if (!new URLSearchParams(location.search).has("detach") && !officeDemo) void initCompany().catch(() => {});
```

Delete `src/company/panel.ts`, `src/company/panel.css`. Remove `.dp-company` rules if any remain elsewhere.

- [ ] **Step 5: Run everything**

Run: `rtk npm test && rtk npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 6: Prove the no-change rule**

Start the dev app on an empty sandbox: remove `~/.bytehive/company-dev` first (it is this plan's, not the owner's), then `rtk npm run tauri:dev`. In the app's console through CDP, or with a screenshot:

- `document.getElementById("office")` is `null`.
- `performance.getEntriesByType("resource")` lists no chunk containing three.
- The start screen and Inbox match a screenshot taken from the installed 0.14.1 with the same projects, by eye.

Then give one job on a scratch project, restart the dev app, and see the office come up as home.

- [ ] **Step 7: Check the release bundle**

```bash
rtk proxy npx vite build
```

Expected: `dist/assets` has no file containing `demoSource` or `Hai` from the fixtures, and the entry chunk has no `WebGLRenderer`.

- [ ] **Step 8: Commit**

```bash
rtk git add src/company src/main.ts src/dock.ts src/inboxhelp.ts src/inboxhelp.test.ts src/bridges.ts src/inbox.ts
rtk git commit -m "feat: the office is home once the Company is used, with the Inbox as fallback"
```

### Task 10: Gate on real WebView2

Phase 0 left five things unmeasured and one result in doubt. This task measures them on this machine, in the real app, before anyone polishes the picture. No product code changes unless a row fails.

**Files:**
- Create: `tools/office_gate.mjs`, `docs/superpowers/specs/2026-11-06-office-gate-results.md`

**Setup.** Close nothing of the owner's. Check the port with Task 1's preflight. Then:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9333"
$env:VITE_OFFICE_DEMO = "1"
rtk npm run tauri:dev
```

Demo source, so no company is read or written. Open two scratch folders as projects in the dev window and start 12 PowerShell agents through the real New agent form, six in each. Machine as in Phase 0: Intel UHD 630 through ANGLE, 12 logical cores, `low-power`.

`tools/office_gate.mjs` connects to port 9333 with Node's `WebSocket`, injects a collector, runs each row, and prints the table. The collector:

```js
window.__gate = (() => {
  const gaps = [], long = [];
  let last = performance.now(), on = true;
  const beat = (t) => { gaps.push(t - last); last = t; if (on) requestAnimationFrame(beat); };
  requestAnimationFrame(beat);
  new PerformanceObserver((l) => l.getEntries().forEach((e) => long.push(e.duration))).observe({ entryTypes: ["longtask"] });
  return {
    reset() { gaps.length = 0; long.length = 0; },
    stop() { on = false; },
    read: () => ({ maxGap: Math.max(0, ...gaps), over200: gaps.filter((g) => g > 200).length, longMax: Math.max(0, ...long), frames: gaps.length }),
  };
})();
```

CPU: sum `CPU` over the dev app's process tree (the `bytehive-dev` PID you started and its WebView2 children, found by parent PID) twice, 60 s apart, divided by `60 * 12` for all cores. Report both figures, all cores and one core, as Phase 0 did. Phase 0's note about which figure a pass rests on applies here too: state it.

**Gates.** Limits are chosen by this plan, from Phase 0's numbers: idle baseline 1.74 to 1.9% of all cores, `maxGap` 100 ms.

| # | Measure | How | Pass |
|---|---|---|---|
| G1 | Moving scene | `__office` orbits the camera at the 30 fps cap for 60 s, 12 idle shells | CPU at most 15% of all cores, `over200` 0, in 3 of 3 runs |
| G2 | Ambient scene | 6 people working, camera still, 60 s | CPU at most 5% of all cores, `over200` 0 |
| G3 | Still scene | Nobody working, 60 s, then the same with the office hidden | At most 5%, and within 1 point of the hidden run. `stats().frames` does not rise. |
| G4 | Resizing renderer | 20 native window resizes with `SetWindowPos`, 20 trips Floor to Desk to Floor | `over200` 0, `maxGap` under 200 ms. 500 ms after each change `stats().drawW` equals canvas CSS width times pixel ratio, within 1 px. No black frame in a screenshot after each 5th. |
| G5 | Streaming fleet | All 12 shells run a loop printing about 200 lines a second for 5 minutes, scene ambient, then 60 s of camera moves. Control: the same with the office hidden. | `over200` at most the control's plus 1, no gap over 400 ms. A marker line printed at the end of the loop shows on screen within 2 s. |
| G6 | The old freeze | Inject a `.crew-card` with `transition` and its `.on` glow into a `.backdrop.open` over the running scene, toggle it 50 times with real mouse events. Repeat with `box-shadow` animated on the same element. Assert `WEBGL_BUDGET` is still 0. | No gap over 200 ms. The freeze was 15 to 20 seconds, so any gap over 1 s is a fail whatever the rest says. |
| G7 | Modal after spawning | Start 12 shells, at once open and close New agent 10 times. 5 runs with the scene, 5 with the office hidden. This re-measures the 266 ms gap of Phase 0. | Gaps over 200 ms with the scene are no more than without it |
| G8 | Context restore | `WEBGL_lose_context.loseContext()` from `getContext`, then `restoreContext()` after 500 ms. 10 times. Then the window minimized 10 minutes and restored. | Picture back within 2 s of the restore call every time (`stats().lastRestoreMs`), with textures: logo and badges visible in a screenshot. `lost` equals `restored`. |
| G9 | Fallback for real | `loseContext()` and no restore | Inbox with Needs you within 3 s of the loss, rows still act |

**Real sleep is not run.** A forced suspend would stop the owner's machine and the other agent on `feat/remote`. G8 is the stand-in the owner asked for. Say so in the results, and add one line for the owner: if the screen stays black after a real sleep, the office falls back by G9's path.

**If a row fails:**

1. G1, G2 or G3: in this order, re-measuring after each: set `AMBIENT_MS` to 250, then ambient off; pixel ratio to 1; `antialias` off; shadows off. Each is one constant.
2. G4: remove the width transition, keep one `resize` at the end.
3. G5, G6, G7 or G8, or any row still failing: spec section 10's order. First OffscreenCanvas in a Worker: `createScene` is built for it, so change the caller, send model and level by `postMessage`, keep `pick` and `anchors` as messages. Then a second Tauri window. Re-run every row after a change.
4. If one still fails: set `OFFICE_3D = false`, which leaves the rail in the Inbox, the Desk reachable through the rail, and everything from Tasks 1 to 5 working. Task 11 is skipped, Task 12 releases that, and the final report says plainly that the 3D office is off and why. The third fallback in spec section 10, baked images, gets its own plan.

- [ ] **Step 1: Write the tool and record a baseline on the demo source**
- [ ] **Step 2: Run G1 to G9, three runs where the table says so**
- [ ] **Step 3: Apply the failure steps until each row passes, or `OFFICE_3D` is false**
- [ ] **Step 4: Write `2026-11-06-office-gate-results.md`**

One row per gate with the numbers and the verdict, in the shape of the Phase 0 results. Keep every failed run in the table, with its cause if known. Name what was not covered.

- [ ] **Step 5: Stop the dev app you started, by PID. Commit.**

```bash
rtk git add tools/office_gate.mjs docs/superpowers/specs/2026-11-06-office-gate-results.md
rtk git commit -m "docs: office measurements on real WebView2"
```

Add any constant changes from Step 3 in their own `perf:` commit first.

### Task 11: Looking and fixing pass

The owner chose this look from a picture. This task makes the running office match that picture, by looking at it. Invoke `build-premium-website` first, route `DASHBOARD_PRODUCT` with the Three.js and accessibility references, and run its gates on the shots.

**Files:**
- Modify: `src/company/office/build.ts`, `people.ts`, `office.css`, `palette.ts`, `scene.ts` constants only

**Method.** Real screenshots only. A claim that something looks right without a PNG read in this task does not count. Two sources: the demo in headless Chrome for speed, and the real WebView2 at the end, because colour and antialiasing differ.

- [ ] **Step 1: Take the first set**

`tools/office_shots.mjs` at 1280x800 and 1920x1080: Building, Floor (`bytehive`), Desk (`linh`), Founder's Desk, `?empty=1`, `?quiet=1`, `?floors=8`, `?flight=1`.

- [ ] **Step 2: Judge beside `style-a.webp`**

Read both images. For each row below write a pass or a fix in the task report. The rows come from the approved look.

| Row | Style A has | Fails when |
|---|---|---|
| Massing | A compact chunky tower, thick rounded slabs that overhang the walls | Thin or sharp edged, or the tower floats with no ground plate |
| Walls | Warm charcoal shell, honey glow inside, brighter at the top of each room | Flat brown, or walls lit from nowhere |
| Cutaway | Two walls, front and side open, floors stacked with a clear gap | You cannot see into a room at Building |
| Lift | Glass and steel column on the right, lamps at each floor, mast above the roof | Reads as a pole, or the glass is opaque |
| People | Round bead heads, big for their bodies, chairs and desks in scale, readable at Building | Heads the size of bodies' shoulders or smaller, capsule people |
| Desks | Pairs of desks with wood tops, dark bodies, blue-lit screens | Grey boxes |
| Roof | Founder at a desk behind glass rail, penthouse with lit window and door | Bare plate |
| Light | Soft and warm, a warm pool on the ground in front, dark slate background | Hard shadows, or a grey wash from too much hemisphere light |
| Logo | Honey cell, unchanged, on the roof | Recoloured, stretched or missing |
| State | Needs is the first thing the eye finds, and bubbles stay rare | Everything glows the same |

The picture was an AI image and the spec says the real one will be simpler in lighting. A simpler light that keeps the mood passes. Do not add post-processing to chase it: bloom costs too much on a machine that runs agents.

- [ ] **Step 3: Check the chrome**

| Check | Pass |
|---|---|
| Text | Every string on a solid chip or panel. Nothing under 12px. `palette.test.ts` passes for every pair actually used. No line wraps over 6 lines. |
| Contrast by eye | Read the tags on the brightest room in the shot, not only on the sky |
| 1280x800 | Tags do not collide. The rail and job box do not cover the tower's middle. |
| Hierarchy | At Building, in 5 seconds: what needs me. That is spec section 1. Show the shot to yourself cold and name it. |
| Copy | Plain, like the app. No marketing words. `Needs you`, `Inbox`, `Send`. |
| Motion | One easing, no overshoot. Reduced motion cuts. |
| Focus | Tab through all three levels on the demo, honey ring visible on every stop |
| AI slop | No gradient text, no glowing cards, no chrome competing with the scene |

- [ ] **Step 4: Fix, re-shoot, repeat**

At least two rounds, and until every row passes. Tune constants, not architecture. Keep draw calls under 300 at 20 people: read `stats().calls` with `?floors=8`.

- [ ] **Step 5: Confirm on real WebView2**

Start the dev app with `VITE_OFFICE_DEMO=1` as in Task 10, shoot Building, Floor and Desk with `Page.captureScreenshot` over port 9333, with a real terminal at Desk. Read them. Repeat G1 and G2 if anything heavier was added: the limits still hold.

- [ ] **Step 6: Commit**

```bash
rtk git add src/company/office
rtk git commit -m "style: the office matches the approved look"
```

### Task 12: End to end, then release 0.15.0

Uses real Claude Code agents and real quota, on the sandbox and scratch repos only. Hash check first: `rtk node tools/company_preflight.mjs --hash`.

**Files:**
- Modify: `CHANGELOG.md`, `README.md`, `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`

- [ ] **Step 1: Full test run**

`rtk npm test`, `rtk npx tsc --noEmit`, and from `mcp/`: `rtk proxy npx vitest run` and `rtk npm run build`.

- [ ] **Step 2: Scenario A, the office from nothing**

`rtk npm run build:mcp`, empty `~/.bytehive/company-dev`, `rtk npm run tauri:dev`. Open a scratch repo. Ctrl+Shift+O. Send `Add marketing to <scratch>`.

Expected, in order: the Founder appears on the roof, working. A plan row appears in the rail. Send it back with a note first: the Founder plans again (`Send back` was never exercised in Phase 1). Approve. A new person sits down on that floor with a marketing shirt and a megaphone, and a paper plane runs up the lift. Click them: Desk, real terminal, you can type in it. Report tab shows their card. Esc with the terminal focused goes to the terminal. Click the crumb: Floor.

- [ ] **Step 3: Scenario B, two floors and the leftovers from Phase 1**

Open a second scratch repo. Send one Job that needs 2 hires on one floor and 1 on the other. This exercises several hires in one approve, a Lead, QA's `test-report` skill and the Lead sending a card back, all unproven so far. Expected: approve once, merge once. The merge row opens Desk at Changes. Row `Open` works. The Report shows committed files, not `.claude/`, and ticked items.

- [ ] **Step 4: Scenario C, what went wrong in Phase 1**

Stop a running Job from the rail while someone works. Expected: their turn ends within seconds, cards show Stopped on the board, the Founder hears.

Set `stallMinutes` to 1 in the sandbox's `company.json` with the app closed. Give a Job and approve it, then stop the hired person's pane before they pick the card up. Expected: their boss is nudged about the To do card, then a row appears. The queue timeout has no real run: nothing here can hang `git worktree add` on demand, so its unit test stands for it.

- [ ] **Step 5: Scenario D, the no-change check and the second app**

Repeat Task 9 Step 6 on a clean sandbox. Then, with `rtk node tools/company_preflight.mjs --hold-lock <sandbox>` running, start the dev app: the office says the Company runs in another window, and `company.json` in the sandbox does not change.

Confirm the owner's data did not move: `--hash` gives the value from the start of this task, or only `engine.lock` changed if the installed app is running.

- [ ] **Step 6: Docs and version**

Add a `## [0.15.0] - <date>` section to `CHANGELOG.md`, for users: the office, how to get to the Inbox, Stop now ends a turn, cards show what was committed, checklists tick. Say that nothing changes for people who do not use the Company. Replace the Company section of `README.md`.

Set `0.15.0` in `package.json`, both places in `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the `bytehive` package in `src-tauri/Cargo.lock`, by hand, without `cargo fmt`.

```bash
rtk git add CHANGELOG.md README.md package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
rtk git commit -m "chore: version 0.15.0"
```

- [ ] **Step 7: Release**

Merge `feat/office` into `revert/maestro-v0.10.10`, push, tag `v0.15.0`, push the tag. Do not merge `feat/remote`. Watch the Windows job until green. Check `latest.json` on `tdat-dev/maestro-releases` says 0.15.0. Then go back to `feat/office` for Phase 3.

---

## Phase 3: Characters and more CLIs (0.16.0)

### Task 13: Character editing

Starts with the `build-premium-website` skill. A small editor at Desk, saved in `Person.look`, seen in the office at once. Swatches only, no free colour: every choice has been seen against the walls, and a role file's odd colour cannot end up in a style.

**Files:**
- Create: `src/company/look.ts`, `src/company/look.test.ts`, `src/company/office/lookeditor.ts`, `lookeditor.test.ts`
- Modify: `src/company/reduce.ts`, `src/company/engine.ts`, `src/company/office/source.ts`, `live.ts`, `fixtures.ts`, `shell.ts`, `office.css`, `model.ts` (`HAIRS` moves to `look.ts`)
- Test: `src/company/reduce.test.ts`, `src/company/engine.test.ts`

**Interfaces:**
- Produces, `look.ts`: `SHIRT_CHOICES: string[]`, `HAIRS: string[]`, `PROPS: Array<{ id: string; label: string }>`, `validLook(l: unknown): l is Pick<Look, "shirt" | "hair" | "prop">`.
- Produces, `reduce.ts`: `setLook(s: State, id: string, look: Pick<Look, "shirt" | "hair" | "prop"> | null): State`. `null` goes back to the role's look.
- Produces, `engine.ts`: `setPersonLook(id: string, look: Pick<Look, "shirt" | "hair" | "prop"> | null): Promise<void>`.
- Produces, `source.ts`: `setLook(id: string, look: Pick<Look, "shirt" | "hair" | "prop"> | null): Promise<void>`.
- Produces, `lookeditor.ts`: `createLookEditor(o: { source: OfficeSource; anchor: HTMLElement }): { open(p: OfficePerson): void; close(): void; isOpen(): boolean }`.
- Consumes: `lookOf`, `OfficePerson` (Task 4). `edited` on `Look` is there since Task 4.

- [ ] **Step 1: Write the failing tests**

```ts
// src/company/look.test.ts
import { describe, expect, it } from "vitest";
import { HAIRS, PROPS, SHIRT_CHOICES, validLook } from "./look";

describe("validLook", () => {
  const good = { shirt: SHIRT_CHOICES[0], hair: HAIRS[0], prop: PROPS[0].id };
  it("takes a look made of the choices", () => { expect(validLook(good)).toBe(true); });
  it.each([
    ["a shirt off the list", { ...good, shirt: "#000000" }],
    ["a style in a colour", { ...good, shirt: "red;background:url(x)" }],
    ["hair off the list", { ...good, hair: "#ffffff" }],
    ["an unknown prop", { ...good, prop: "sword" }],
    ["nothing", null],
  ])("refuses %s", (_name, look) => { expect(validLook(look)).toBe(false); });
});

describe("the choices", () => {
  it("cover every department's colour and every role's prop", () => {
    expect(SHIRT_CHOICES).toEqual(expect.arrayContaining(["#f5b83d", "#5ec2f0", "#5fd08a", "#ff6b6b", "#e0913a", "#b18cf0"]));
    expect(PROPS.map((p) => p.id)).toEqual(["none", "laptop", "clipboard", "megaphone", "magnifier", "book", "pen", "pencil"]);
  });
});
```

Add to `src/company/reduce.test.ts`:

```ts
describe("setLook", () => {
  const person = { id: "a", name: "A", role: "qa", rank: "staff", floor: "x", boss: "founder", cli: "claude", look: { shirt: "#b18cf0", hair: "#2a2e3a", prop: "magnifier" } } as const;
  const role = { id: "qa", title: "QA", department: "Engineering", rank: "staff", cli: "claude", denyTools: [], look: { shirt: "#b18cf0", hair: "#2a2e3a", prop: "magnifier" }, prompt: "", skills: [] } as RoleDef;
  const s: State = { company: { ...emptyCompany(), people: [{ ...person, look: { ...person.look } }] }, jobs: [], roles: [role] };
  const look = { shirt: "#ff6b6b", hair: "#4a3226", prop: "book" };
  it("saves the look as the owner's", () => {
    expect(setLook(s, "a", look).company.people[0].look).toEqual({ ...look, edited: true });
  });
  it("goes back to the role's look", () => {
    const edited = setLook(s, "a", look);
    expect(setLook(edited, "a", null).company.people[0].look).toEqual(role.look);
  });
  it("gives back the same state for an unknown person or a look off the list", () => {
    expect(setLook(s, "nobody", look)).toBe(s);
    expect(setLook(s, "a", { ...look, shirt: "#000000" })).toBe(s);
  });
});
```

Add to `src/company/engine.test.ts`:

```ts
describe("a person's look", () => {
  it("is saved in company.json", async () => {
    await engine.setPersonLook("founder", { shirt: "#ff6b6b", hair: "#4a3226", prop: "book" });
    const saved = JSON.parse(io.files.get(`${ROOT}\\company.json`)!);
    expect(saved.people[0].look).toEqual({ shirt: "#ff6b6b", hair: "#4a3226", prop: "book", edited: true });
  });
});
```

```ts
// src/company/office/lookeditor.test.ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { demoModel, demoSource } from "./fixtures";
import { createLookEditor } from "./lookeditor";

const anchor = () => document.body.appendChild(document.createElement("button"));
const linh = () => demoModel().floors.flatMap((f) => f.people).find((p) => p.id === "linh")!;

describe("the look editor", () => {
  beforeEach(() => { document.body.innerHTML = ""; });
  it("saves a choice at once and keeps the others", () => {
    const source = demoSource();
    const save = vi.spyOn(source, "setLook");
    const ed = createLookEditor({ source, anchor: anchor() });
    ed.open(linh());
    document.querySelector<HTMLElement>('[data-prop="book"]')!.click();
    expect(save).toHaveBeenCalledWith("linh", { shirt: linh().shirt, hair: linh().hair, prop: "book" });
  });
  it("has a Reset that goes back to the role", () => {
    const source = demoSource();
    const save = vi.spyOn(source, "setLook");
    const ed = createLookEditor({ source, anchor: anchor() });
    ed.open(linh());
    document.querySelector<HTMLElement>("[data-reset]")!.click();
    expect(save).toHaveBeenCalledWith("linh", null);
  });
  it("names every swatch for a screen reader", () => {
    const ed = createLookEditor({ source: demoSource(), anchor: anchor() });
    ed.open(linh());
    for (const b of document.querySelectorAll<HTMLElement>("[data-shirt], [data-hair]")) expect(b.getAttribute("aria-label")).toBeTruthy();
  });
  it("closes on Esc and gives focus back", () => {
    const a = anchor();
    const ed = createLookEditor({ source: demoSource(), anchor: a });
    ed.open(linh());
    document.querySelector(".of-look")!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(ed.isOpen()).toBe(false);
    expect(document.activeElement).toBe(a);
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/look.test.ts src/company/reduce.test.ts src/company/office/lookeditor.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
// src/company/look.ts
/* What a character can wear. Choices, not free colours: each one has been seen
 * against the walls, and a colour here ends up in a style attribute. */
import type { Look } from "./types";

export const SHIRT_CHOICES = ["#f5b83d", "#5ec2f0", "#5fd08a", "#ff6b6b", "#e0913a", "#b18cf0", "#7d8b99", "#e8e4da"];
export const HAIRS = ["#2a2e3a", "#4a3226", "#7a5230", "#b08a55", "#55585f", "#7a2f2a"];
export const PROPS = [
  { id: "none", label: "Nothing" }, { id: "laptop", label: "Laptop" }, { id: "clipboard", label: "Clipboard" }, { id: "megaphone", label: "Megaphone" },
  { id: "magnifier", label: "Magnifier" }, { id: "book", label: "Book" }, { id: "pen", label: "Pen" }, { id: "pencil", label: "Pencil" },
];

export function validLook(l: unknown): l is Pick<Look, "shirt" | "hair" | "prop"> {
  const x = l as Look | null;
  return !!x && SHIRT_CHOICES.includes(x.shirt) && HAIRS.includes(x.hair) && PROPS.some((p) => p.id === x.prop);
}
```

In `src/company/office/model.ts` remove `HAIRS` and import it from `../look`, so `hairFor` and the editor share one list.

In `src/company/reduce.ts`:

```ts
/** The owner dresses someone, or `null` hands them back to their role. */
export function setLook(s: State, id: string, look: Pick<Look, "shirt" | "hair" | "prop"> | null): State {
  const p = personById(s.company, id);
  if (!p || (look && !validLook(look))) return s;
  const role = s.roles.find((r) => r.id === p.role);
  const { edited: _old, ...own } = role?.look ?? p.look;
  const next: Look = look ? { ...look, edited: true } : own;
  return { ...s, company: { ...s.company, people: s.company.people.map((x) => (x.id === id ? { ...x, look: next } : x)) } };
}
```

In `src/company/engine.ts`:

```ts
export const setPersonLook = (id: string, look: Pick<Look, "shirt" | "hair" | "prop"> | null): Promise<void> => enqueue(async () => {
  if (!state) return;
  const next = setLook(state, id, look);
  if (next === state) return;
  state = next;
  await tryPersist([]);
}, "Saving a look");
```

Add `setLook` to `OfficeSource`, `live.ts` (`setLook: engine.setPersonLook`) and `demoSource` (in memory, fires listeners).

`lookeditor.ts`: a popover under a `Look` button that the shell puts in the Desk's tab row, right aligned, for every person including the Founder. Three `role="radiogroup"` rows: Shirt (8 swatches, `aria-label` a colour name from a small table in the file), Hair (6), Holds (8 text buttons). Current choice `aria-checked="true"`. Each click calls `source.setLook` at once, so the small floor beside it changes while you choose. `Reset` at the foot. Esc or a click outside closes and gives focus back. Saving failing shows one line in the popover.

The look shows in the small floor at Desk with no change to the scene: the Desk shot frames the person, and `sceneKey` already carries shirt, hair and prop.

`reduce.ts` imports `validLook` from `./look` and `type Look` from `./types`.

- [ ] **Step 4: Run tests and the type check**

Run: `rtk proxy npx vitest run src/company && rtk npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 5: Look at it**

Screenshot the demo at `?level=desk:linh` with the editor open, at 1280x800. Pick each prop once: each one reads at Desk size. Contrast of the Holds labels passes `palette.test.ts`: add the pair if it is new. Then in the sandbox dev app, change a look, restart the app, and see it kept in `company-dev/company.json`.

- [ ] **Step 6: Commit**

```bash
rtk git add src/company
rtk git commit -m "feat: dress a person from their desk"
```

### Task 14: Role launch for Codex and OpenCode

This task proves the flags, then writes the pure launch code. Task 15 wires it in and runs it for real. Spec section 7 sets the table. What was checked while writing this plan, on this machine, with no model call:

| Fact | Status |
|---|---|
| OpenCode 2.2.5 reads `OPENCODE_CONFIG_CONTENT` under `--standalone`, as its own config source | Seen with `opencode debug config --standalone` |
| It accepts `permission`, `mcp.<name>` with `environment`, `skills.paths`, `instructions`, and rewrites them to `permissions`, `mcp.servers`, `skills`, `instructions` | Seen |
| The owner's global OpenCode config allows everything and has its own `bytehive` server | Seen. Whether the role's deny and server win is not known. |
| `--standalone` means a private server, a child of the pane. Without it a background service would never see this process's variables. | From `opencode --help`. This is why the spec needs it. |
| `--auto` approves whatever is not denied. `--session <id>` continues or creates that session. | From `opencode --help` |
| Codex 0.165 has `-c`, `-s`, `-a`, `--add-dir`, `resume --last`, `-p` profiles | From `codex --help` |
| **Codex does not start on this machine today.** `~/.codex/config.toml:14` fails to parse (`notify = [...]`), and `codex mcp list` and `codex doctor` both report it | Seen. `--version` and `--help` work. A real `codex` session does not. |
| `developer_instructions`, `mcp_servers.*.env_vars` under `-c` | Not checked: Codex will not load |

The config error is the owner's file, written by their Codex app. This plan does not edit it. A Codex role refuses with that reason until it is fixed, and Task 15 runs the working path against a throwaway `CODEX_HOME`.

**Files:**
- Create: `src/company/cli.ts`, `src/company/cli.test.ts`, `docs/superpowers/specs/2026-11-06-phase3-cli-results.md`
- Modify: `src/company/launch.ts`, `src/company/launch.test.ts`, `src/company/types.ts`, `src/company/org.ts`, `src/company/org.test.ts`, `src/company/needs.ts`, `src/company/needs.test.ts`, `mcp/src/company.ts`

**Interfaces:**
- Produces, `types.ts`: `Hire.cli?: string`.
- Produces, `cli.ts`: `ROLE_CLIS`, `cliRefusal(cli: string): string | null`, `leaderRefusal(rank: Rank, cli: string): string | null`, `cliLabel(cli: string): string`, `readinessCheck(cli: string): { program: string; args: string[] } | null`, `readinessVerdict(cli: string, r: { code: number | null; stdout: string; stderr?: string } | { failed: string }): string | null`, `memberSession(spec: { badge: string; sessionId?: string }, newId: () => string): { before: string[]; after: string[]; sessionId: string } | null`, `interruptKeys(cli: string): string`.
- Produces, `launch.ts`: `interface RoleLaunch { args: string[]; env: Array<[string, string]>; files: Array<{ rel: string; text: string }> }`, `buildRoleLaunch(o: LaunchInput): RoleLaunch`, `withSkills(prompt: string, skills: Array<{ name: string; text: string }>): string`.

```ts
export interface LaunchInput {
  person: Person;
  role: RoleDef;
  promptFile: string;
  mcpConfig: string;     // Claude: its --mcp-config file
  mcpServer: string;     // the bundled server, for CLIs that take it inline
  pluginDir: string | null;
  skillsDir: string | null; // OpenCode: the folder holding <skill>/SKILL.md
  gitDir: string | null;    // Codex: the repo's .git, outside a worktree's sandbox
  settings?: string | null;
}
```

- [ ] **Step 1: Run the probes**

In the session scratchpad, with `rtk proxy`. Codex first: if `codex mcp list` still fails, make `$SCRATCH/codex-home`, copy only `~/.codex/auth.json` into it, write a two-line `config.toml` (`model` as the owner's), and use `CODEX_HOME=$SCRATCH/codex-home` for every Codex probe. Delete that folder at the end of this task. The copied file is a credential: it must not reach the repo or the results file.

Each probe spends a small amount of quota. Build a scratch git repo with a worktree, and a probe role file ending `End every reply with the word PROBE-ROLE.`

| # | Question | Command, shortened | If the answer is no |
|---|---|---|---|
| C1 | Does `developer_instructions` pointing at a file work? | `codex exec -c "developer_instructions='Your role is in the file <path>. Read it first and follow it.'" "Say hello."` | Send that sentence as the first message instead, through `deliverTask` |
| C2 | Does it survive `cmd /c`, with a space in the path? | Same through `cmd //c codex exec ...` | Write `$CODEX_HOME/bytehive-<id>.config.toml` and pass `-p bytehive-<id>` |
| C3 | Do `-c mcp_servers.bytehive.*` replace the user's server? | `-c "mcp_servers.bytehive.command='node'" -c "mcp_servers.bytehive.args=['<server>']"`, ask it to call `company_status` | Use a different server name, `bytehive-role`, and say so in the role prompt |
| C4 | Do `mcp_servers.bytehive.env_vars` pass `BYTEHIVE_AGENT_ID` to the server? | Same, with the variable set; the tool answers with the person, or refuses | `mcp_servers.bytehive.env={BYTEHIVE_AGENT_ID='p1'}` inline. Worktree-only values go through the same table at boot. |
| C5 | Can `-a never -s workspace-write --add-dir <repo>/.git` commit in a worktree? | Ask it to create and commit a file, in the worktree | `--dangerously-bypass-approvals-and-sandbox`, the same freedom as Claude's skip permissions. A role with `denyTools` then refuses on Codex: the sandbox was its only fence. |
| C6 | Does a new worktree open on a trust question? | Start interactive `codex` in a new worktree with the same flags, read the first screen | `-c "projects.'<path>'.trust_level='trusted'"`, built at boot. If that fails, the question shows in Needs you: an honest limit. |
| C7 | `codex resume --last` with the same `-c` flags, in the same folder? | Run after C1, ask what it said | The person starts a new conversation, with a Needs you line |
| O1 | Does the role's deny beat the global allow? | `OPENCODE_CONFIG_CONTENT=<json> opencode run --standalone --auto "Create x.txt"` with `permission.edit` denied | Put the deny under a custom agent in the config and pass `--agent`. If that fails too, `denyTools` roles refuse on OpenCode. |
| O2 | Does the role's `bytehive` server replace the global one? | `opencode mcp list --standalone` with the content set | Different server name, as C3 |
| O3 | Does `BYTEHIVE_AGENT_ID` reach the server? | `run` asking for `company_status` | Add every variable to `environment` |
| O4 | Does `instructions` apply? | Prompt file ending in PROBE-ROLE | Send the file as the first message |
| O5 | Are skills found at `skills.paths`? | A `brief-writer` skill that holds a probe word | Inline with `withSkills`, as Codex |
| O6 | What may `--session` be given? | `run --session ses_<uuid>` twice, see the second remember | Read the id from `opencode session list` after boot, and store that |
| O7 | Does the TUI paint in a pane? | In the dev app, start it from New agent with `--standalone` | The earlier blank pane: send a resize after spawn. Not found in `pane.ts`, so add it in Task 15 for this CLI |

Write every result into `docs/superpowers/specs/2026-11-06-phase3-cli-results.md`, one row each, shaped like the Phase 0 results: command, what happened, verdict. The code below is the shape for all-yes answers. Where a probe said no, apply its last column and change the matching test.

- [ ] **Step 2: Write the failing tests**

```ts
// src/company/cli.test.ts
import { describe, expect, it } from "vitest";
import { cliRefusal, interruptKeys, leaderRefusal, memberSession, readinessCheck, readinessVerdict } from "./cli";

describe("cliRefusal", () => {
  it.each(["claude", "codex", "opencode"])("takes %s", (cli) => { expect(cliRefusal(cli)).toBeNull(); });
  it("says plainly what it cannot run", () => {
    expect(cliRefusal("gemini")).toBe("ByteHive cannot run a Company role on gemini. Use Claude Code, Codex or OpenCode.");
  });
});

describe("leaderRefusal", () => {
  it("keeps Leads on Claude, and says why", () => {
    expect(leaderRefusal("lead", "codex")).toBe("A Lead runs on Claude Code: only there can file tools be denied.");
    expect(leaderRefusal("lead", "claude")).toBeNull();
  });
  it("leaves staff free", () => { expect(leaderRefusal("staff", "codex")).toBeNull(); });
});

describe("readiness", () => {
  it("asks nothing of Claude, and asks the others something that reads their settings", () => {
    expect(readinessCheck("claude")).toBeNull();
    expect(readinessCheck("codex")).toEqual({ program: "codex", args: ["mcp", "list"] });
    expect(readinessCheck("opencode")).toEqual({ program: "opencode", args: ["--version"] });
  });
  it("is fine when it worked", () => {
    expect(readinessVerdict("codex", { code: 0, stdout: "" })).toBeNull();
  });
  it("names the first line of a failure", () => {
    const r = { code: 1, stdout: "", stderr: "Error: failed to load bootstrap configuration\n\nCaused by: x" };
    expect(readinessVerdict("codex", r)).toBe("Codex cannot start: Error: failed to load bootstrap configuration. Run codex doctor.");
  });
  it("says when the CLI is not installed", () => {
    expect(readinessVerdict("opencode", { code: 1, stdout: "", stderr: "'opencode' is not recognized as an internal or external command" })).toBe("OpenCode is not installed, or is not on PATH.");
  });
  it("does not block on a check that itself failed", () => {
    expect(readinessVerdict("codex", { failed: "took longer than 30 s" })).toBeNull();
  });
});

describe("memberSession", () => {
  const id = () => "ses_new";
  it("leaves Claude to pane.ts", () => { expect(memberSession({ badge: "claude" }, id)).toBeNull(); });
  it("starts Codex fresh, then resumes the last one in its folder", () => {
    expect(memberSession({ badge: "codex" }, id)).toEqual({ before: [], after: [], sessionId: "codex" });
    expect(memberSession({ badge: "codex", sessionId: "codex" }, id)).toEqual({ before: ["resume", "--last"], after: [], sessionId: "codex" });
  });
  it("names OpenCode's session, the same one each time", () => {
    expect(memberSession({ badge: "opencode" }, id)).toEqual({ before: [], after: ["--session", "ses_new"], sessionId: "ses_new" });
    expect(memberSession({ badge: "opencode", sessionId: "ses_a" }, id)?.after).toEqual(["--session", "ses_a"]);
  });
});

describe("interruptKeys", () => {
  it("is Esc unless a probe found otherwise", () => { expect(interruptKeys("claude")).toBe("\u001b"); });
});
```

Add to `src/company/launch.test.ts`:

```ts
describe("buildRoleLaunch for each CLI", () => {
  const person = { id: "p1", name: "Ana", role: "developer", rank: "staff", floor: "a", boss: "founder", cli: "claude", look: { shirt: "#fff", hair: "#000", prop: "none" } } as Person;
  const role = { id: "developer", title: "Developer", department: "Engineering", rank: "staff", cli: "claude", denyTools: [], look: person.look, prompt: "p", skills: [] } as RoleDef;
  const base = { person, role, promptFile: "C:\\co\\people\\p1\\prompt.md", mcpConfig: "C:\\co\\people\\p1\\mcp.json", mcpServer: "C:\\Program Files\\ByteHive\\mcp.mjs", pluginDir: null, skillsDir: null, gitDir: null };

  it("keeps Claude's flags as they were", () => {
    const r = buildRoleLaunch(base);
    expect(r.args).toEqual(["--append-system-prompt-file", base.promptFile, "--mcp-config", base.mcpConfig]);
    expect(r.env).toEqual([["BYTEHIVE_AGENT_ID", "p1"]]);
    expect(r.files).toEqual([]);
  });

  describe("Codex", () => {
    const codex = { ...base, person: { ...person, cli: "codex" } };
    it("points developer_instructions at the prompt file, in forward slashes", () => {
      const r = buildRoleLaunch(codex);
      const at = r.args.findIndex((a) => a.startsWith("developer_instructions="));
      expect(r.args[at - 1]).toBe("-c");
      expect(r.args[at]).toContain("C:/co/people/p1/prompt.md");
      expect(r.args[at]).not.toContain("\\");
    });
    it("gives it the server the app carries, and passes identity through", () => {
      const r = buildRoleLaunch(codex);
      expect(r.args).toContain("mcp_servers.bytehive.command='node'");
      expect(r.args).toContain("mcp_servers.bytehive.args=['C:/Program Files/ByteHive/mcp.mjs']");
      expect(r.args.find((a) => a.startsWith("mcp_servers.bytehive.env_vars="))).toContain("BYTEHIVE_AGENT_ID");
    });
    it("writes in the workspace by default, and lets git reach the repo", () => {
      const r = buildRoleLaunch({ ...codex, gitDir: "D:/app/.git" });
      expect(r.args.join(" ")).toContain("-a never -s workspace-write --add-dir D:/app/.git");
    });
    it("is read-only for a role that denies tools", () => {
      const r = buildRoleLaunch({ ...codex, role: { ...role, denyTools: ["Edit"] } });
      expect(r.args.join(" ")).toContain("-s read-only");
    });
    it("refuses a path a TOML literal cannot hold", () => {
      expect(() => buildRoleLaunch({ ...codex, promptFile: "C:\\o'brien\\p.md" })).toThrow("a quote in its path");
    });
  });

  describe("OpenCode", () => {
    const open = { ...base, person: { ...person, cli: "opencode" }, skillsDir: "C:\\co\\people\\p1\\skills" };
    const config = (r: ReturnType<typeof buildRoleLaunch>) => JSON.parse(r.env.find(([k]) => k === "OPENCODE_CONFIG_CONTENT")![1]);
    it("runs on a private server, so the variables reach it", () => {
      expect(buildRoleLaunch(open).args).toEqual(["--standalone", "--auto"]);
    });
    it("carries the role in its config", () => {
      const c = config(buildRoleLaunch(open));
      expect(c.instructions).toEqual(["C:/co/people/p1/prompt.md"]);
      expect(c.skills).toEqual({ paths: ["C:/co/people/p1/skills"] });
      expect(c.mcp.bytehive).toEqual({ type: "local", command: ["node", "C:/Program Files/ByteHive/mcp.mjs"], environment: { BYTEHIVE_AGENT_ID: "p1" } });
    });
    it("turns denied tools into its own permissions", () => {
      const c = config(buildRoleLaunch({ ...open, role: { ...role, denyTools: ["Edit", "Write", "Bash", "PowerShell"] } }));
      expect(c.permission).toEqual({ edit: "deny", bash: "deny" });
    });
    it("denies nothing by default", () => {
      expect(config(buildRoleLaunch(open)).permission).toEqual({});
    });
  });

  it("refuses a CLI that cannot hold a role", () => {
    expect(() => buildRoleLaunch({ ...base, person: { ...person, cli: "gemini" } })).toThrow("cannot run a Company role on gemini");
  });
});

describe("withSkills", () => {
  const skill = { name: "brief", text: "---\nname: brief\ndescription: Use for briefs.\n---\nWrite the brief.\n" };
  it("leaves a prompt alone with no skills", () => { expect(withSkills("Role.", [])).toBe("Role."); });
  it("adds the skill's body, without its header, and says to use it", () => {
    const out = withSkills("Role.", [skill]);
    expect(out).toContain("## brief");
    expect(out).toContain("Write the brief.");
    expect(out).not.toContain("description:");
    expect(out).toMatch(/Use these when they apply/);
  });
});
```

Existing cases in this file call `buildRoleLaunch` with the old input: spread `base` into them.

Add to `src/company/org.test.ts`:

```ts
describe("a hire's CLI", () => {
  const dev = { id: "developer", title: "Developer", department: "Engineering", rank: "staff", cli: "claude", denyTools: [], look: { shirt: "#fff", hair: "#000", prop: "none" }, prompt: "", skills: [] } as RoleDef;
  const lead = { ...dev, id: "lead", rank: "lead" } as RoleDef;
  const c = { ...emptyCompany(), floors: [{ id: "a", dir: "D:\\a", name: "a" }] };
  const plan = (hires: Hire[]): JobPlan => ({ summary: "s", cards: [], hires });
  it("defaults to the role's, and takes the plan's", () => {
    const pick = { name: () => "Ana", id: () => "p" };
    expect(applyHires(c, plan([{ role: "developer", floor: "a" }]), [dev], pick).hired[0].cli).toBe("claude");
    expect(applyHires(c, plan([{ role: "developer", floor: "a", cli: "codex" }]), [dev], pick).hired[0].cli).toBe("codex");
  });
  it("refuses a CLI that cannot hold a role", () => {
    expect(validatePlan(c, plan([{ role: "developer", floor: "a", cli: "gemini" }]), [dev]).join(" ")).toContain("cannot run a Company role on gemini");
  });
  it("refuses a role whose own CLI cannot hold it", () => {
    expect(validatePlan(c, plan([{ role: "developer", floor: "a" }]), [{ ...dev, cli: "aider" }]).join(" ")).toContain("on aider");
  });
  it("keeps a Lead on Claude", () => {
    expect(validatePlan(c, plan([{ role: "lead", floor: "a", cli: "codex" }]), [lead]).join(" ")).toContain("A Lead runs on Claude Code");
  });
});
```

Add to `src/company/needs.test.ts`:

```ts
it("says which CLI a hire runs on when it is not Claude", () => {
  const plan = { summary: "s", cards: [], hires: [{ role: "qa", floor: "a", cli: "codex" }, { role: "qa", floor: "a" }] };
  const rows = needsYou([{ id: "J", title: "t", status: "awaiting-plan", plan, tokenCap: 9, people: [] } as unknown as Job], [], { floors: [{ id: "a", dir: "", name: "alpha" }], roles: [] });
  expect(rows[0].plan?.hires).toEqual(["qa on alpha, on Codex", "qa on alpha"]);
});
```

- [ ] **Step 3: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/cli.test.ts src/company/launch.test.ts src/company/org.test.ts src/company/needs.test.ts`
Expected: FAIL.

- [ ] **Step 4: Implement**

```ts
// src/company/cli.ts
/* Which CLIs can hold a role, and what each needs. Pure, and shared with the MCP
 * bundle through org.ts, so a plan is refused at the same line in both places. */
import type { Rank } from "./types";

export const ROLE_CLIS = ["claude", "codex", "opencode"];

const LABELS: Record<string, string> = { claude: "Claude Code", codex: "Codex", opencode: "OpenCode" };
export const cliLabel = (cli: string) => LABELS[cli] ?? cli;

export function cliRefusal(cli: string): string | null {
  return ROLE_CLIS.includes(cli) ? null : `ByteHive cannot run a Company role on ${cli}. Use Claude Code, Codex or OpenCode.`;
}

export function leaderRefusal(rank: Rank, cli: string): string | null {
  return rank !== "staff" && cli !== "claude" ? "A Lead runs on Claude Code: only there can file tools be denied." : null;
}

/** A command that fails when the CLI cannot start, without calling a model. */
export function readinessCheck(cli: string): { program: string; args: string[] } | null {
  if (cli === "codex") return { program: "codex", args: ["mcp", "list"] };
  if (cli === "opencode") return { program: "opencode", args: ["--version"] };
  return null;
}

/** Why a role cannot start on this CLI now, in words. A check that broke says nothing about the CLI. */
export function readinessVerdict(cli: string, r: { code: number | null; stdout: string; stderr?: string } | { failed: string }): string | null {
  if ("failed" in r || r.code === 0 || r.code === null) return null;
  const said = (r.stderr || r.stdout).trim();
  if (/not recognized|not found/i.test(said)) return `${cliLabel(cli)} is not installed, or is not on PATH.`;
  const first = said.split(/\r?\n/)[0].replace(/\.$/, "");
  return `${cliLabel(cli)} cannot start: ${first}. Run ${cli} doctor.`;
}

/** What a member's pane adds around its args to carry on a conversation. Claude has claudeSessionArgs. */
export function memberSession(spec: { badge: string; sessionId?: string }, newId: () => string): { before: string[]; after: string[]; sessionId: string } | null {
  if (spec.badge === "codex") return spec.sessionId ? { before: ["resume", "--last"], after: [], sessionId: spec.sessionId } : { before: [], after: [], sessionId: "codex" };
  if (spec.badge === "opencode") {
    const id = spec.sessionId ?? newId();
    return { before: [], after: ["--session", id], sessionId: id };
  }
  return null;
}

/** What ends a running turn. Add a CLI here only if a real run showed it differs. */
export const interruptKeys = (_cli: string): string => "\u001b";
```

`readinessVerdict` needs the check's stderr: `runCaptureStatus` gives `stdout` and `code`. Read `src/ipc.ts` and `run_capture_status` in the backend. If stderr is not returned, add it to the Rust result and the TS type, with a test on the Rust side if one exists for that command. If `opencode doctor` does not exist, the last sentence names `opencode debug config` instead: check with `opencode --help` and fix the string and test.

In `src/company/launch.ts`:

```ts
/** A prompt for a CLI with no way to attach skills: they ride in the text. */
export function withSkills(prompt: string, skills: Array<{ name: string; text: string }>): string {
  if (!skills.length) return prompt;
  const body = skills.map((s) => `## ${s.name}\n\n${s.text.replace(/^---[\s\S]*?---\r?\n/, "").trim()}`).join("\n\n");
  return `${prompt}\n\n# Skills\n\nUse these when they apply. They are part of your job.\n\n${body}`;
}

const fwd = (p: string) => p.replace(/\\/g, "/");

/** A TOML literal string: no escapes, so no quote can sit in one. */
function literal(text: string): string {
  if (text.includes("'")) throw new Error(`Codex cannot be started with a quote in its path: ${text}`);
  return `'${text}'`;
}

/** Every variable the MCP server reads to know who is calling. */
const IDENTITY = ["BYTEHIVE_AGENT", "BYTEHIVE_AGENT_ID", "BYTEHIVE_WORKSPACE", "BYTEHIVE_RUN_DIR", "BYTEHIVE_COMPANY_ROOT"];

function claudeLaunch(o: LaunchInput): RoleLaunch {
  const args = ["--append-system-prompt-file", o.promptFile, "--mcp-config", o.mcpConfig];
  if (o.pluginDir) args.push("--plugin-dir", o.pluginDir);
  if (o.role.denyTools.length) args.push("--disallowedTools", o.role.denyTools.join(","));
  if (o.settings) args.push("--settings", o.settings);
  return { args, env: [["BYTEHIVE_AGENT_ID", o.person.id]], files: [] };
}

/** Codex has no prompt file flag and the prompt must not cross cmd /c, so the
 *  instructions are one sentence that names the file. Skills are in that file. */
function codexLaunch(o: LaunchInput): RoleLaunch {
  const text = `Your role and the rules of this company are in the file ${fwd(o.promptFile)}. Read that file first, before anything else, and follow it for the whole session.`;
  const args = [
    "-c", `developer_instructions=${literal(text)}`,
    "-c", "mcp_servers.bytehive.command='node'",
    "-c", `mcp_servers.bytehive.args=[${literal(fwd(o.mcpServer))}]`,
    "-c", `mcp_servers.bytehive.env_vars=[${IDENTITY.map(literal).join(",")}]`,
    "-a", "never",
    "-s", o.role.denyTools.length ? "read-only" : "workspace-write",
  ];
  if (o.gitDir) args.push("--add-dir", o.gitDir);
  return { args, env: [["BYTEHIVE_AGENT_ID", o.person.id]], files: [] };
}

const OPENCODE_DENY: Record<string, string> = { Edit: "edit", Write: "edit", MultiEdit: "edit", NotebookEdit: "edit", Bash: "bash", PowerShell: "bash" };

/** --standalone: the default background service would not see this process's variables. */
function opencodeLaunch(o: LaunchInput): RoleLaunch {
  const config = {
    instructions: [fwd(o.promptFile)],
    skills: { paths: o.skillsDir ? [fwd(o.skillsDir)] : [] },
    permission: Object.fromEntries(o.role.denyTools.flatMap((t) => (OPENCODE_DENY[t] ? [[OPENCODE_DENY[t], "deny"]] : []))),
    mcp: { bytehive: { type: "local", command: ["node", fwd(o.mcpServer)], environment: { BYTEHIVE_AGENT_ID: o.person.id } } },
  };
  return { args: ["--standalone", "--auto"], env: [["BYTEHIVE_AGENT_ID", o.person.id], ["OPENCODE_CONFIG_CONTENT", JSON.stringify(config)]], files: [] };
}

/** No --strict-mcp-config for Claude: the owner's other servers keep working. */
export function buildRoleLaunch(o: LaunchInput): RoleLaunch {
  const refused = cliRefusal(o.person.cli);
  if (refused) throw new Error(refused);
  return o.person.cli === "codex" ? codexLaunch(o) : o.person.cli === "opencode" ? opencodeLaunch(o) : claudeLaunch(o);
}
```

Note `files` stays empty for now: `{ args, env, files }` is the spec's shape, and the engine already writes the prompt and plugin itself. Task 15 uses `files` for nothing either unless a probe needed a profile file: that is its one use. Leave it, and note this line in the commit.

In `src/company/types.ts`: `export interface Hire { role: string; floor: string; cli?: string }`.

In `src/company/org.ts`, inside `validatePlan`'s hires loop, after the role checks:

```ts
    const role = roleOf(h.role);
    if (role) {
      const cli = h.cli ?? role.cli;
      const no = cliRefusal(cli) ?? leaderRefusal(role.rank, cli);
      if (no) errors.push(no);
    }
```

In `applyHires`, `add` takes `cli` from the hire: `add(h.role, h.floor, boss, h.cli)` and `cli: cli ?? role.cli`.

In `src/company/needs.ts`, `planDetail`:

```ts
hires: plan.hires.map((h) => `${roleTitle(h.role)} on ${floorName(h.floor)}${h.cli && h.cli !== "claude" ? `, on ${cliLabel(h.cli)}` : ""}`),
```

In `mcp/src/company.ts`, add to the `job_plan` hires item: `cli: z.enum(["claude", "codex", "opencode"]).optional().describe("Leave out for the role's own CLI. Staff only.")`. Add one line to `FOUNDER_LAWS` in `src/laws.ts`: `A hire can name a cli when the owner asks for one. Leads always run on Claude.`

Update `src/company/engine.ts` callers of `buildRoleLaunch` for the new arguments, passing `mcpServer: await resolveResource(MCP_RESOURCE)`, `skillsDir: null`, `gitDir: null`, so Claude behaves as before. Task 15 fills them.

- [ ] **Step 5: Run tests and the type check**

Run: `rtk proxy npx vitest run src/company && rtk npx tsc --noEmit`, and from `mcp/`, `rtk proxy npx vitest run`
Expected: PASS.

- [ ] **Step 6: Clean up and commit**

Delete `$SCRATCH/codex-home` if you made it.

```bash
rtk git add src/company src/laws.ts mcp/src/company.ts docs/superpowers/specs/2026-11-06-phase3-cli-results.md
rtk git commit -m "feat: role launch for Codex and OpenCode, with plain refusals"
```

### Task 15: Run Codex and OpenCode roles for real

Uses real agents on scratch repos, in the sandbox. Each CLI has to pass the same list, or it comes off `ROLE_CLIS` and a role on it refuses, with the reason.

**Files:**
- Modify: `src/company/engine.ts`, `src/company/cli.ts`, `src/pane.ts:349-356`, `src/spawnmodal.ts` (only if Probe O7 needs a resize), `docs/superpowers/specs/2026-11-06-phase3-cli-results.md`
- Test: `src/company/engine.test.ts`, `src/pane.boot.test.ts`

**Interfaces:**
- Consumes: `buildRoleLaunch`, `withSkills`, `memberSession`, `readinessCheck`, `readinessVerdict`, `interruptKeys` (Task 14).
- Produces: `approve` rejects with the readiness reason before anything changes.

- [ ] **Step 1: Write the failing tests**

Add to `src/company/engine.test.ts`. Extend the `runCaptureStatus` mock so that a command ending `codex mcp list` answers from `io.cliCheck`, and one containing `rev-parse` gives `{ stdout: "D:\\a\\.git\n", code: 0 }`. Add `cliCheck: { stdout: "", code: 0 } as { stdout: string; code: number; stderr?: string }` to `io`, and reset it in `beforeEach`.

```ts
describe("hiring on another CLI", () => {
  const onCodex = (): JobPlan => ({ summary: "s", cards: [{ floor: "a", title: "Do it", desc: "d", checklist: [] }], hires: [{ role: "developer", floor: "a", cli: "codex" }] });

  it("starts them with Codex's flags and their skills in the prompt file", async () => {
    await waitingJob(onCodex());
    await engine.approve(job().id);
    const pane = member();
    expect(pane.spec.badge).toBe("codex");
    expect(pane.spec.args).toContain("-s");
    expect(pane.spec.args).not.toContain("--dangerously-skip-permissions");
    expect(pane.spec.args.join(" ")).toContain("developer_instructions=");
  });
  it("refuses to approve while the CLI cannot start, and changes nothing", async () => {
    await waitingJob(onCodex());
    io.cliCheck = { stdout: "", stderr: "Error: failed to load bootstrap configuration", code: 1 };
    await expect(engine.approve(job().id)).rejects.toThrow("Codex cannot start: Error: failed to load bootstrap configuration");
    expect(job().status).toBe("awaiting-plan");
    expect(io.spawned.filter((n) => n !== "Founder")).toEqual([]);
  });
  it("does not check a plan that only uses Claude", async () => {
    await waitingJob(planFor("a", 1));
    io.cliCheck = { stdout: "", stderr: "broken", code: 1 };
    await engine.approve(job().id);
    expect(job().status).toBe("running");
  });
  it("does not ask Claude's session store about a Codex conversation", async () => {
    await waitingJob(onCodex());
    await engine.approve(job().id);
    const p = engine.companyView()!.company.people.find((x) => x.cli === "codex")!;
    p.session = "codex";
    const ws = [...workspaces.values()].find((w) => [...w.panes.values()].some((x) => x.spec.personId === p.id))!;
    ws.panes.clear();
    await engine.wakePerson(p.id);
    expect(engine.companyAlerts().map((a) => a.key)).not.toContain(`lost-${p.id}`);
  });
});
```

Add to `src/pane.boot.test.ts`:

```ts
it("starts a Codex member fresh, then resumes it, with resume before its flags", async () => {
  const spec = memberSpec({ badge: "codex", program: "codex", args: ["-s", "workspace-write"] });
  await boot(spec);
  expect(spawned[spawned.length - 1].args).toEqual(["/c", "codex", "-s", "workspace-write"]);
  expect(spec.sessionId).toBe("codex");
  await boot(spec);
  expect(spawned[spawned.length - 1].args).toEqual(["/c", "codex", "resume", "--last", "-s", "workspace-write"]);
});
it("gives an OpenCode member the same session each time", async () => {
  const spec = memberSpec({ badge: "opencode", program: "opencode", args: ["--standalone", "--auto"] });
  await boot(spec);
  const first = spawned[spawned.length - 1].args.slice(-2);
  expect(first[0]).toBe("--session");
  await boot(spec);
  expect(spawned[spawned.length - 1].args.slice(-2)).toEqual(first);
});
it("leaves a plain Codex agent as it was", async () => {
  const spec = plainSpec({ badge: "codex", program: "codex", args: [] });
  await boot(spec);
  await boot(spec);
  expect(spawned[spawned.length - 1].args).toEqual(["/c", "codex"]);
});
```

`memberSpec`, `plainSpec`, `boot` and `spawned`: use the harness already in `src/pane.boot.test.ts`, under the names it has, with `personId` set on the member.

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/engine.test.ts src/pane.boot.test.ts`
Expected: FAIL.

- [ ] **Step 3: Wire it in**

`src/company/engine.ts`, `personSpec`:

```ts
async function personSpec(p: Person, role: RoleDef, dir: string): Promise<AgentSpec> {
  const base = `people\\${p.id}`;
  const skills = await roleSkills(root, role);
  // Only Claude and OpenCode can attach skills. Codex reads them in its prompt file.
  const prompt = composePrompt(p, role, state!.company);
  await writeNew(root, `${base}\\prompt.md`, p.cli === "codex" ? withSkills(prompt, skills) : prompt);
  await writeMcpConfig(p);
  const pluginDir = p.cli === "claude" ? await writePlugin(p, role) : null;
  const skillsDir = p.cli === "opencode" ? await writeSkills(p, skills) : null;
  const founder = p.rank === "founder";
  if (founder) await writeSettings(p);
  const launch = buildRoleLaunch({
    person: p, role, promptFile: `${root}\\${base}\\prompt.md`, mcpConfig: `${root}\\${base}\\mcp.json`,
    mcpServer: await resolveResource(MCP_RESOURCE), pluginDir, skillsDir, gitDir: await gitDirOf(dir),
    settings: founder ? `${root}\\${base}\\settings.json` : null,
  });
  const preset = CLI_PRESETS.find((x) => x.id === p.cli);
  if (!preset) throw new Error(`unknown CLI ${p.cli}`);
  return {
    program: preset.program,
    // Claude's skip permissions flag. The others carry their own policy in the launch.
    args: [...(p.cli === "claude" ? effectiveArgs(preset, true) : preset.args), ...launch.args],
```

The rest of the object stays. `refreshFiles` follows the same rule for `prompt.md`, plugin and skills.

```ts
/** A plain folder of skills, for a CLI that reads them from a path. Null with none. */
async function writeSkills(p: Person, skills: Array<{ name: string; text: string }>): Promise<string | null> {
  if (!skills.length) return null;
  const base = `people\\${p.id}\\skills`;
  for (const s of skills) await writeNew(root, `${base}\\${s.name}\\SKILL.md`, s.text);
  return `${root}\\${base}`;
}

/** The repo's own .git: a worktree's commits land there, outside the worktree. */
async function gitDirOf(dir: string): Promise<string | null> {
  const git = launchSpec("git", ["rev-parse", "--path-format=absolute", "--git-common-dir"]);
  const r = await runCaptureStatus(git.program, git.args, dir, 30_000).catch(() => null);
  return r?.code === 0 ? r.stdout.trim() : null;
}
```

`checkRestore`: `if (spec.badge === "claude" && spec.sessionId && !(await claudeSessionExists(...)))`.

In `approve`, before `approvePlan`:

```ts
/** Why the CLIs a plan hires on cannot start, if they can't. Asked before anything changes. */
async function cliProblem(job: Job): Promise<string | null> {
  const clis = new Set((job.plan?.hires ?? []).map((h) => h.cli ?? state?.roles.find((r) => r.id === h.role)?.cli ?? "claude"));
  for (const cli of clis) {
    const check = readinessCheck(cli);
    if (!check) continue;
    const run = launchSpec(check.program, check.args);
    const said = await runCaptureStatus(run.program, run.args, null, NODE_WAIT_MS).then((r) => readinessVerdict(cli, r), (e) => readinessVerdict(cli, { failed: reason(e) }));
    if (said) return said;
  }
  return null;
}
```

```ts
  const job0 = state.jobs.find((j) => j.id === id)!;
  const problem = await cliProblem(job0);
  if (problem) throw new Error(problem); // shown on the row: the plan stays to be approved or sent back
```

`interrupt` sends `interruptKeys(pane.spec.badge)`.

`src/pane.ts`, replacing the args block at lines 350-356:

```ts
      const own = spec.badge === "claude" ? await claudeSessionArgs(spec, cwd) : [];
      if (own.length) saveSession(); // History finds this run's transcript after a restart too
      // Another CLI's member keeps its conversation too, by that CLI's own flags.
      const kept = spec.personId ? memberSession(spec, () => `ses_${crypto.randomUUID().replace(/-/g, "")}`) : null;
      if (kept) { spec.sessionId = kept.sessionId; saveSession(); }
      // A member of the company brings its laws in its own args (company/launch.ts).
      const args =
        spec.badge !== "claude" ? [...(kept?.before ?? []), ...spec.args, ...(kept?.after ?? [])]
        : spec.personId ? [...spec.args, ...own]
        : [...spec.args, ...own, "--append-system-prompt", laws];
```

A plain Codex or OpenCode agent gets `kept` null, so nothing changes for it. `src/agentcost.ts` keys on `sessionId`, which is fine: it reads by folder for these CLIs.

Apply whatever the probes in Task 14 changed. If O7 showed a blank pane, send a resize shortly after spawn for `badge === "opencode"` members only.

- [ ] **Step 4: Run tests**

Run: `rtk proxy npx vitest run src/company src/pane && rtk npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 5: Run the refusal for real**

Sandbox dev app, scratch repo, as it stands on this machine. Give a Job, and in the rail see a plan. Have the Founder hire `a Codex developer`. If `~/.codex/config.toml` still does not parse: Approve shows `Codex cannot start: ...`, nobody starts, Send back works. This is the real run of the refusal. Write it in the results.

- [ ] **Step 6: Run each CLI through the list**

Codex with `CODEX_HOME` pointing at a throwaway home if the owner's still fails, set in the shell that starts `tauri:dev`, removed afterwards. OpenCode as it is. Quota is real, so keep the tasks small: `Add a VERSION file with 1.0.0`.

| # | Check | How |
|---|---|---|
| R1 | It starts in its own worktree, at a prompt, with no question left open | Read the pane |
| R2 | It knows its role | Type `What is your job, and who is your boss?` |
| R3 | It has its skills | Hire `marketing`. Ask for a brief. Codex: from the prompt. OpenCode: from the skills folder. |
| R4 | It has the app's MCP, with its own identity | `company_status` names it. `fleet_send` to its boss works. |
| R5 | The rules hold | Ask it to message the Founder directly when it has a Lead, and to `agent_spawn`: both refused |
| R6 | A card runs through | `card_move` to Doing, commit, `card_done`. Report shows the file, with no `.claude/`. |
| R7 | A denied role cannot write | A role with `denyTools` on this CLI tries to edit: refused or read-only. If it can write, that role refuses on this CLI. |
| R8 | It comes back | Stop the pane, send a message from the office: it restarts on the same conversation and remembers |
| R9 | Stop ends a turn | Start a long task, Stop the Job |
| R10 | Tokens count | `tokensUsed` rises (Task 16 makes it exact) |
| R11 | In the office | Badge shows its logo. State follows `working`, `idle`. A real question shows as `needs` with a bubble. |

Record each row per CLI in the results file: pass or fail, with what was seen. Spec section 7 calls OpenCode unproven, so the real end to end run decides. Failing R1, R2, R4, R5, R6 or R7 is a failure of the role on that CLI.

- [ ] **Step 7: Take a failed CLI off the list**

If a CLI fails a required row, change `src/company/cli.ts`:

```ts
export const ROLE_CLIS = ["claude", "codex"]; // whichever passed

/** A CLI that was tried for real and did not hold up, with what went wrong. */
const NOT_YET: Record<string, string> = { opencode: "OpenCode roles are off for now: <the row that failed, in one plain line>." };

export function cliRefusal(cli: string): string | null {
  if (ROLE_CLIS.includes(cli)) return null;
  return NOT_YET[cli] ?? `ByteHive cannot run a Company role on ${cli}. Use ${ROLE_CLIS.map(cliLabel).join(" or ")}.`;
}
```

Fix the `cliRefusal` and `buildRoleLaunch` tests to match, and the `job_plan` enum. Keep the launch code: it is tested and the next CLI version may pass.

- [ ] **Step 8: Clean up and commit**

Stop what you started, by PID. Delete any throwaway `CODEX_HOME`. Confirm the owner's hash.

```bash
rtk git add src/company src/pane.ts src/pane.boot.test.ts src/spawnmodal.ts docs/superpowers/specs/2026-11-06-phase3-cli-results.md
rtk git commit -m "feat: Company roles run on Codex and OpenCode"
```

If only one CLI passed, the commit message names that one.

### Task 16: A branch per Job and honest token counts

Backlog items closed here: a reused person gets a branch per Job, token counts. Both wait for Phase 3 because other CLIs change what a person's usage and session are.

**Files:**
- Create: `src/company/branch.ts`, `branch.test.ts`, `src/company/ledger.ts`, `ledger.test.ts`
- Modify: `src/company/engine.ts:555-580`, `src/company/engine.ts:610-645`, `src/company/engine.ts:428-446`, `src/company/types.ts`, `src-tauri/src/agentusage.rs`, `src/ipc.ts` (the `AgentUsage` type), `src/agentcost.ts:64-70`
- Test: `src/company/engine.test.ts`

**Interfaces:**
- Produces, `branch.ts`: `jobBranch(jobId: string, name: string): string`, `branchStep(i): BranchStep`.
- Produces, `ledger.ts`: `SEEN_BASE = -1`, `seedSeen(c: Company, jobs: Job[]): Record<string, number>`, `chargeJob(personId, jobs, doing): string | null`, `measureTokens(i): { jobs: Job[]; seen: Record<string, number> }`.
- Produces, `types.ts`: `Company.tokensSeen?: Record<string, number>`. `Job.tokenBase` stays in old files and is no longer read.

- [ ] **Step 1: Write the failing tests**

```ts
// src/company/branch.test.ts
import { describe, expect, it } from "vitest";
import type { Job, Person } from "./types";
import { branchStep, jobBranch } from "./branch";

const p = { id: "a", name: "Ana Maria", worktree: "D:\\wt\\a", branch: "bytehive/ana-1" } as Person;
const job = (id: string, status: Job["status"], people = ["a"]) => ({ id, status, people }) as Job;
const now = job("J-0007", "running");
const step = (jobs: Job[], over: object = {}) => branchStep({ person: p, job: now, jobs: [now, ...jobs], dirty: false, working: false, ...over });

describe("jobBranch", () => {
  it("names the Job and the person", () => { expect(jobBranch("J-0007", "Ana Maria")).toBe("bytehive/j-0007-ana-maria"); });
});

describe("branchStep", () => {
  it("stays for a person on their first Job: their hire's branch is that Job's", () => {
    expect(step([])).toEqual({ kind: "stay" });
  });
  it("stays when already on this Job's branch", () => {
    expect(branchStep({ person: { ...p, branch: "bytehive/j-0007-ana-maria" }, job: now, jobs: [now, job("J-1", "done")], dirty: false, working: false })).toEqual({ kind: "stay" });
  });
  it("stays for someone with no worktree: they get a new one at start", () => {
    expect(branchStep({ person: { ...p, worktree: undefined }, job: now, jobs: [now, job("J-1", "done")], dirty: false, working: false })).toEqual({ kind: "stay" });
  });
  it.each(["done", "stopped", "rejected"] as const)("switches after a Job that is %s", (status) => {
    expect(step([job("J-1", status)])).toEqual({ kind: "switch", branch: "bytehive/j-0007-ana-maria" });
  });
  it.each(["running", "paused", "awaiting-merge"] as const)("stacks, and says so, while an earlier Job is %s", (status) => {
    const s = step([job("J-1", status)]);
    expect(s).toEqual({ kind: "stack", why: `Ana Maria is still on J-1, so J-0007 goes on the same branch. Review it with J-1.` });
  });
  it("stacks with unfinished work in the folder", () => {
    expect(step([job("J-1", "done")], { dirty: true }).kind).toBe("stack");
  });
  it("stacks while they are in the middle of a turn", () => {
    expect(step([job("J-1", "done")], { working: true }).kind).toBe("stack");
  });
  it("ignores other people's Jobs", () => {
    expect(step([job("J-1", "running", ["b"])])).toEqual({ kind: "stay" });
  });
});
```

```ts
// src/company/ledger.test.ts
import { describe, expect, it } from "vitest";
import { emptyCompany } from "./org";
import type { Job } from "./types";
import { SEEN_BASE, chargeJob, measureTokens, seedSeen } from "./ledger";

const job = (id: string, status: Job["status"], people: string[], over: Partial<Job> = {}) =>
  ({ id, status, people, tokensUsed: 0, tokenBase: {}, ...over }) as Job;

describe("seedSeen", () => {
  it("keeps what the company already has", () => {
    const seen = { a: 5 };
    expect(seedSeen({ ...emptyCompany(), tokensSeen: seen }, [])).toBe(seen);
  });
  it("takes the first reading as a base for anyone from before the ledger", () => {
    expect(seedSeen(emptyCompany(), []).founder).toBe(SEEN_BASE);
  });
  it("carries over the highest figure the Jobs had", () => {
    const c = { ...emptyCompany(), people: [...emptyCompany().people, { id: "a" } as never] };
    expect(seedSeen(c, [job("J1", "done", [], { tokenBase: { a: 10 } }), job("J2", "done", [], { tokenBase: { a: 30 } })]).a).toBe(30);
  });
});

describe("chargeJob", () => {
  const jobs = [job("J1", "running", ["a"]), job("J2", "running", ["a"]), job("J3", "done", ["a"])];
  it("charges the Job whose card they hold", () => {
    expect(chargeJob("a", jobs, new Map([["a", "J1"]]))).toBe("J1");
  });
  it("else the newest Job going", () => {
    expect(chargeJob("a", jobs, new Map())).toBe("J2");
  });
  it("ignores a card of a Job that ended", () => {
    expect(chargeJob("a", jobs, new Map([["a", "J3"]]))).toBe("J2");
  });
  it("charges the Founder to the newest Job going, planning included", () => {
    expect(chargeJob("founder", [job("J1", "running", []), job("J2", "planning", [])], new Map())).toBe("J2");
  });
  it("charges nobody with no Job going", () => {
    expect(chargeJob("a", [job("J3", "done", ["a"])], new Map())).toBeNull();
  });
});

describe("measureTokens", () => {
  const run = (jobs: Job[], seen: Record<string, number>, usage: Array<[string, number | null]>) =>
    measureTokens({ jobs, seen, usage: new Map(usage), doing: new Map() });

  it("charges a new hire from zero, start-up included", () => {
    const r = run([job("J1", "running", ["a"])], {}, [["a", 65_636]]);
    expect(r.jobs[0].tokensUsed).toBe(65_636);
    expect(r.seen.a).toBe(65_636);
  });
  it("charges only what was added", () => {
    expect(run([job("J1", "running", ["a"])], { a: 100 }, [["a", 150]]).jobs[0].tokensUsed).toBe(50);
  });
  it("charges one Job, not both", () => {
    const r = run([job("J1", "running", ["a"]), job("J2", "running", ["a"])], { a: 100 }, [["a", 150]]);
    expect(r.jobs.map((j) => j.tokensUsed)).toEqual([0, 50]);
  });
  it("takes a base without charging it", () => {
    const r = run([job("J1", "running", ["founder"])], { founder: SEEN_BASE }, [["founder", 9000]]);
    expect(r.jobs[0].tokensUsed).toBe(0);
    expect(r.seen.founder).toBe(9000);
  });
  it("rebases a session that started over", () => {
    const r = run([job("J1", "running", ["a"])], { a: 900 }, [["a", 40]]);
    expect(r.jobs[0].tokensUsed).toBe(0);
    expect(r.seen.a).toBe(40);
  });
  it("skips a reading that came back empty", () => {
    const r = run([job("J1", "running", ["a"])], { a: 100 }, [["a", null]]);
    expect(r.seen.a).toBe(100);
  });
  it("keeps counting on a paused Job, for the turn it let finish", () => {
    expect(run([job("J1", "paused", ["a"])], { a: 100 }, [["a", 150]]).jobs[0].tokensUsed).toBe(50);
  });
  it("gives back the same Jobs when nothing moved, so an idle company writes nothing", () => {
    const jobs = [job("J1", "running", ["a"])];
    expect(run(jobs, { a: 100 }, [["a", 100]]).jobs).toBe(jobs);
  });
  it("drops tokens spent with no Job going, and notes them as seen", () => {
    const r = run([job("J1", "done", ["a"])], { a: 100 }, [["a", 150]]);
    expect(r.jobs[0].tokensUsed).toBe(0);
    expect(r.seen.a).toBe(150);
  });
});
```

Add to `src/company/engine.test.ts`. Extend the `runCaptureStatus` mock to record any command containing `git` in `io.git` and answer `{ stdout: "abc123\n", code: 0 }`, and add `gitChangedFiles: async () => io.dirty,` to the `../ipc` mock, with `git: [] as string[][]`, `dirty: [] as unknown[]`.

```ts
describe("a person used again", () => {
  async function reused(status: "done" | "running"): Promise<string> {
    await waitingJob(planFor("a", 1));
    await engine.approve(job().id);
    const id = member().spec.personId!;
    member().spec.worktree = "D:\\wt\\x";
    member().spec.branch = "bytehive/x-1";
    await vi.advanceTimersByTimeAsync(30_000); // the watch notes it on the Person
    job().status = status;
    const name = engine.companyView()!.company.people.find((p) => p.id === id)!.name;
    await engine.giveJob("next");
    const plan: JobPlan = { summary: "s", cards: [{ floor: "a", title: "More", desc: "d", checklist: [], assignee: name }], hires: [] };
    await request({ type: "plan", ts: 2, from: "founder", job: job(1).id, plan });
    io.git.length = 0;
    await engine.approve(job(1).id);
    return id;
  }
  it("moves them to a branch of this Job, from the project's HEAD", async () => {
    await reused("done");
    expect(io.git[io.git.length - 1].join(" ")).toContain("switch -c bytehive/j-0002-");
    expect(member().spec.branch).toMatch(/^bytehive\/j-0002-/);
  });
  it("tells the owner when it has to stack", async () => {
    const id = await reused("running");
    expect(io.git.flat().join(" ")).not.toContain("switch");
    expect(engine.companyAlerts().map((a) => a.key)).toContain(`stacked-${job(1).id}-${id}`);
  });
});
```

Existing engine tests on `measure` assumed the old first-reading rule, for example a person's first figure charged nothing: read each one and update it to the rules above, which are what the owner asked for. A test that wanted the old undercount was pinning the bug in `task-9-report.md` item 4.

- [ ] **Step 2: Run them to see them fail**

Run: `rtk proxy npx vitest run src/company/branch.test.ts src/company/ledger.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
// src/company/branch.ts
/* One Job, one branch, so that rejecting a Job's work does not mean picking it out of
 * another's. Review works on a person's one worktree, so the move has limits. */
import { slug } from "../worktree";
import type { Job, Person } from "./types";

export type BranchStep = { kind: "stay" } | { kind: "switch"; branch: string } | { kind: "stack"; why: string };

export const jobBranch = (jobId: string, name: string) => `bytehive/${slug(jobId)}-${slug(name) || "agent"}`;

const OPEN: Job["status"][] = ["running", "paused", "awaiting-merge"];

export function branchStep(i: { person: Person; job: Job; jobs: Job[]; dirty: boolean; working: boolean }): BranchStep {
  const { person: p, job } = i;
  const branch = jobBranch(job.id, p.name);
  if (!p.worktree || !p.branch || p.branch === branch) return { kind: "stay" };
  const earlier = i.jobs.filter((j) => j.id !== job.id && j.people.includes(p.id));
  if (!earlier.length) return { kind: "stay" };
  const open = earlier.find((j) => OPEN.includes(j.status));
  if (open) return { kind: "stack", why: `${p.name} is still on ${open.id}, so ${job.id} goes on the same branch. Review it with ${open.id}.` };
  if (i.dirty || i.working) return { kind: "stack", why: `${p.name} has unfinished work in their folder, so ${job.id} goes on the same branch.` };
  return { kind: "switch", branch };
}
```

```ts
// src/company/ledger.ts
/* Who spent what. One figure per person, last seen, in the company: each token is
 * charged once, to one Job. Fresh tokens only: cached re-reads grow fast in any long
 * session and say nothing about work (spec section 3). */
import { FOUNDER_ID, type Company, type Job } from "./types";

/** Take the next reading as the start: this person worked before anyone was counting. */
export const SEEN_BASE = -1;

const GOING: Job["status"][] = ["planning", "running", "paused"];

export function seedSeen(c: Company, jobs: Job[]): Record<string, number> {
  if (c.tokensSeen) return c.tokensSeen;
  const seen: Record<string, number> = {};
  for (const p of c.people) seen[p.id] = SEEN_BASE;
  for (const j of jobs) for (const [id, n] of Object.entries(j.tokenBase)) seen[id] = Math.max(seen[id] ?? SEEN_BASE, n);
  return seen;
}

/** The Job a person's new tokens belong to: the card they hold, else their newest Job going. */
export function chargeJob(personId: string, jobs: Job[], doing: Map<string, string>): string | null {
  const going = jobs.filter((j) => GOING.includes(j.status) && (personId === FOUNDER_ID || j.people.includes(personId)));
  const held = doing.get(personId);
  if (held && going.some((j) => j.id === held)) return held;
  return going[going.length - 1]?.id ?? null;
}

export function measureTokens(i: { jobs: Job[]; seen: Record<string, number>; usage: Map<string, number | null>; doing: Map<string, string> }): { jobs: Job[]; seen: Record<string, number> } {
  const seen = { ...i.seen };
  const added = new Map<string, number>();
  for (const [id, fresh] of i.usage) {
    if (fresh === null) continue; // a read that came back empty, not a restart
    const was = seen[id];
    seen[id] = fresh;
    // No figure: a new hire, and their start-up is theirs. The base and a session started over charge nothing.
    const spent = was === undefined ? fresh : was === SEEN_BASE || fresh <= was ? 0 : fresh - was;
    if (!spent) continue;
    const job = chargeJob(id, i.jobs, i.doing);
    if (job) added.set(job, (added.get(job) ?? 0) + spent);
  }
  if (!added.size) return { jobs: i.jobs, seen };
  return { jobs: i.jobs.map((j) => (added.has(j.id) ? { ...j, tokensUsed: j.tokensUsed + added.get(j.id)! } : j)), seen };
}
```

In `src/company/types.ts` add to `Company`: `tokensSeen?: Record<string, number>; // Person id to the fresh-token figure last seen`.

In `src/company/engine.ts`, delete `measure`. In `watch()`, replace `state = { ...state, jobs: state.jobs.map(measure) };` with:

```ts
  const doing = new Map(cards.filter((c) => c.list === "doing").map((c) => [c.assigneeId, c.job]));
  const usage = new Map<string, number | null>();
  for (const p of state.company.people) {
    const found = paneOf(p.id);
    const u = found ? agentUsage(found.pane) : null;
    if (u) usage.set(p.id, u.fresh);
  }
  const counted = measureTokens({ jobs: state.jobs, seen: seedSeen(state.company, state.jobs), usage, doing });
  state = { ...state, jobs: counted.jobs, company: { ...state.company, tokensSeen: counted.seen } };
```

Compare `tokensSeen` before saving, so an idle company still writes nothing: only set it when a figure moved.

Branch move, in `approve`, after `startHires`:

```ts
/** People used again leave the last Job's branch, when they can. */
async function moveBranches(job: Job, hired: Person[]): Promise<string[]> {
  const failed: string[] = [];
  for (const id of job.people) {
    const p = state && personById(state.company, id);
    const found = paneOf(id);
    if (!p || !found || hired.some((h) => h.id === id) || !p.worktree) continue;
    const { pane } = found;
    const dirty = (await gitChangedFiles(p.worktree).catch(() => [])).length > 0;
    const working = !!pane.running && Date.now() - pane.lastOutputAt <= WORKING_MS;
    const step = branchStep({ person: p, job, jobs: state!.jobs, dirty, working });
    if (step.kind === "stack") raiseAlert(`stacked-${job.id}-${id}`, step.why, id);
    if (step.kind !== "switch") continue;
    const floor = state!.company.floors.find((f) => f.id === p.floor);
    const git = async (cwd: string, ...args: string[]) => {
      const run = launchSpec("git", args);
      const r = await runCaptureStatus(run.program, run.args, cwd, NODE_WAIT_MS);
      if (r.code !== 0) throw new Error(`git ${args[0]} failed`);
      return r.stdout.trim();
    };
    try {
      const base = await git(floor?.dir ?? p.worktree, "rev-parse", "HEAD");
      await git(p.worktree, "switch", "-c", step.branch, base);
      pane.spec.branch = step.branch;
    } catch (e) {
      failed.push(`${p.name} stays on ${p.branch}: ${reason(e)}.`);
    }
  }
  if (remember()) await tryPersist([]);
  return failed;
}
```

Add `...await moveBranches(job, r.hired)` to the `failed.push` in `approve`, after `startHires`. The Founder is told the branch by the existing brief: add the person's branch to `reportFailures` only on failure. When someone moves, the next `deliver` carries on as before: the folder and session are the same, only the branch changed, so nothing restarts.

- [ ] **Step 4: Count fresh tokens for the other CLIs**

Today `fresh` is the same as `tokens` for Codex and OpenCode, and Codex's total includes cached input. In `src-tauri/src/agentusage.rs`, add `pub fresh: Option<u64>` to `AgentUsage` next to `tokens`. In `codex_usage_in`:

```rust
            out.tokens = info.pointer("/total_token_usage/total_tokens").and_then(Value::as_u64);
            let cached = info.pointer("/total_token_usage/cached_input_tokens").and_then(Value::as_u64).unwrap_or(0);
            out.fresh = out.tokens.map(|t| t.saturating_sub(cached));
```

Add a test next to `codex_reads_its_last_token_count_and_model` with `"cached_input_tokens":900` in the JSON, asserting `fresh`. In `opencode_usage_in`, read what its query sums and leave cache reads out of `fresh`, with a test beside the existing one. In `src/ipc.ts` add `fresh?: number | null`, and in `src/agentcost.ts` use `u.fresh = r.fresh ?? r.tokens ?? null;`. Do not run `cargo fmt`.

- [ ] **Step 5: Run tests and the type check**

Run: `rtk proxy npx vitest run src/company src/agentcost && rtk npx tsc --noEmit` and `rtk cargo test --manifest-path src-tauri/Cargo.toml agentusage`
Expected: PASS.

- [ ] **Step 6: Run it for real**

Sandbox, scratch repo. Job one: a hire adds a file. Merge or Mark done. Job two to the same person. Expected: their pane is on `bytehive/j-000N-<name>`, branched from the project's HEAD, with none of Job one's commits in it unless they were merged. `tokensUsed` on Job one includes the hire's start-up, and a person on one Job at a time adds to one Job.

- [ ] **Step 7: Commit**

```bash
rtk git add src/company src-tauri/src/agentusage.rs src/ipc.ts src/agentcost.ts
rtk git commit -m "fix: each job gets its own branch and tokens are charged once"
```

### Task 17: Release 0.16.0

**Files:**
- Modify: `CHANGELOG.md`, `README.md`, `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`

- [ ] **Step 1: Full test run**

`rtk npm test`, `rtk npx tsc --noEmit`, from `mcp/`: `rtk proxy npx vitest run` and `rtk npm run build`.

- [ ] **Step 2: Docs and version**

CHANGELOG section `## [0.16.0] - <date>`, for users: dress your people; hire on Codex or OpenCode, with exactly what passed in Task 15 and what did not, and what a refusal looks like; a new branch for each Job and when it has to stack; token counts now include a hire's start-up and charge one Job. If Codex is only refused on this machine because of `~/.codex/config.toml`, that is about the owner's machine: it goes in the final report, not the CHANGELOG.

Set `0.16.0` in `package.json`, both places in `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the `bytehive` package in `src-tauri/Cargo.lock`.

```bash
rtk git add CHANGELOG.md README.md package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
rtk git commit -m "chore: version 0.16.0"
```

- [ ] **Step 3: Release**

Merge `feat/office` into `revert/maestro-v0.10.10`, push, tag `v0.16.0`, push the tag. Watch the Windows job until green. Check `latest.json` on `tdat-dev/maestro-releases` says 0.16.0.

---

## Phase 4: Cleanup (0.17.0)

The one place where people who never used the Company see a change, because the owner asked for these removals and spec section 9 marks them. What they must keep is a working app.

**What goes and what stays.** This table is the contract for Tasks 18 and 19.

| Thing | Fate | Depends on it today | What a person who never used the Company notices |
|---|---|---|---|
| Conductor semi, auto, pipeline | Goes | `src/kanban.ts`, `src/conductor.ts`, `src/pipeline.ts`, `dock.css:68`, `isCompanyFloor` | The Conductor button leaves the board. Cards no longer leave To do by themselves. A note says so once. |
| Director | Goes | `spec.role === "conductor"` in `pane.ts`, `limitguard.ts:179`, `spawnmodal.ts`, `bridges.ts:217`, `agentbridge.ts:75`, `crew.ts`, `panetypes.ts`; `DIRECTOR_LAWS`; `directorFirst`; Settings toggle; the `[data-role="director"]` CSS | The Settings toggle goes. A saved agent named Director keeps its name and conversation and gets the worker rules at its next start. The Founder does this job now. |
| `spawnCrew`'s `conductor` argument | Goes | Nobody passes it | Nothing |
| `.crew*` CSS | Goes | No TypeScript builds these classes. Task 10 used them last. | Nothing |
| `orchestrator/`, `tests/`, `pyproject.toml` | Goes | Nothing in the app, CI or README. `tests/` holds only its Python tests. | Nothing |
| `src/assets/sprites/`, `tools/sprite_*.py` | Goes | Only those three tools write it. No TS or CSS reads it. | Nothing |
| Board, all `card_*` tools, To do, Doing, Done | Stays | | Same |
| Handing out a card by hand: send to agent, drag onto a terminal, assignee chip | Stays | `dispatchPrompt`, `assignCard`, `sendToAgentById`, pane targeting | Same, and now it is the way |
| Build, Test, Review lists on an old pipeline board | Stay as plain lists | | Their cards sit there until moved |
| `plan.json`, `done.json` polling | Stays | Agents' board writes | Same |
| `agent_spawn`, `fleet_send`, outbox, broadcast, Flow | Stay | Any agent can use them, Director or not | Same |
| New agent, race, Compare | Stays | | Same |
| Saved presets and Scheduled agents | Stay | `Template`, `loadTemplates`, `saveTemplate`, `spawnCrew`, `CrewState`, `expandCrew`, `loadCrew`, `scheduler.ts` | Same. This is D15: the spec's "Crew preset, replaced by roles" holds for the crew picker, which is already gone. A preset on a timer is something else. |
| `BYTEHIVE_LAWS` for plain agents | Stays | | Same, minus the word Director |
| Review, History, Sessions, memory, limit guard, hand-over, usage, Grid | Stay | | Same |
| `maestro.kanban.conductor.*` keys | Each removed when read, by the note | | Nothing |
| `isCompanyFloor` | Goes with the Conductor | Only `kanban.ts:393` reads it | Nothing |

### Task 18: Remove the Conductor modes and the Director

**Files:**
- Delete: `src/conductor.ts`, `src/conductor.test.ts`, `src/pipeline.ts`, `src/pipeline.test.ts`
- Modify: `src/kanban.ts`, `src/kanban.test.ts`, `src/company/engine.ts`, `src/company/engine.test.ts`, `src/pane.ts`, `src/panetypes.ts`, `src/crew.ts`, `src/agentbridge.ts`, `src/bridges.ts`, `src/limitguard.ts`, `src/limitguard.test.ts`, `src/spawnmodal.ts`, `src/spawnmodal.test.ts`, `src/laws.ts`, `src/laws.test.ts`, `src/prefs.ts`, `src/prefs.test.ts`, `src/prefsview.ts`, `src/flow.ts`, `src/fleetbridge.ts`, `index.html:171`, `src/styles/canvas.css`, `src/styles/inbox.css`, `src/styles/dock.css`, `mcp/src/server.ts:395`, `docs/superpowers/specs/2026-11-05-company-office-design.md`

**Interfaces:**
- Produces, `kanban.ts`: `conductorGoneNote(saved: unknown): string | null`.
- Produces, `spawnmodal.ts`: `spawnRequested` (was `spawnForConductor`).
- Removes: `AgentSpec.role`, `CliPreset.role`, `FleetPane.role`, `DIRECTOR_LAWS`, `Prefs.directorFirst`, `isCompanyFloor`.

- [ ] **Step 1: Check what still depends on them**

```bash
rtk proxy rg -n "conductor|Conductor|pipeline|Pipeline|Director|director|\.role\b|directorFirst|DIRECTOR_LAWS|isCompanyFloor" src mcp/src index.html --glob "!*.test.ts" --glob "!src/logs/**" --glob "!mcp/src/logs/**"
```

Compare with the Depends column above. Directory words (`directory`, `Directories`) are false hits. If anything new shows up that is not in the table, add it to this task or stop and say why it should stay. Do the same over the tests.

- [ ] **Step 2: Write the failing tests**

Add to `src/kanban.test.ts`:

```ts
describe("conductorGoneNote", () => {
  it.each(["semi", "auto", "pipeline"])("speaks to someone who had %s on", (mode) => {
    expect(conductorGoneNote(mode)).toBe("The Conductor is gone. Cards wait in To do until you hand them to an agent, or give the job to the Founder with Ctrl+Shift+O.");
  });
  it.each(["off", null, undefined, 3])("says nothing for %s", (saved) => {
    expect(conductorGoneNote(saved)).toBeNull();
  });
});
```

Add to `src/pane.boot.test.ts`:

```ts
it("gives a Director saved in an old session the worker's rules, under its own name", async () => {
  const spec = { ...plainSpec({ badge: "claude", program: "claude", args: [] }), name: "Director", role: "conductor" } as AgentSpec;
  await boot(spec);
  const args = spawned[spawned.length - 1].args;
  expect(args[args.indexOf("--append-system-prompt") + 1]).toBe(BYTEHIVE_LAWS);
});
```

Add to `src/limitguard.test.ts`:

```ts
it("offers an agent named Director like any other", () => {
  // the hand-over list used to skip it by role, and an old session may still carry that field
  const pane = paneNamed("Director", { role: "conductor" });
  expect(openTargets(other).map((t) => t.name)).toContain("Director");
});
```

Use the file's own helpers for `paneNamed` and `other`.

Add to `src/laws.test.ts`, replacing the two Director cases:

```ts
it("leaves the worker taking orders from the person first", () => {
  expect(BYTEHIVE_LAWS).toMatch(/person always outranks/);
  expect(BYTEHIVE_LAWS).not.toMatch(/Director/);
});
```

- [ ] **Step 3: Run them to see them fail**

Run: `rtk proxy npx vitest run src/kanban.test.ts src/limitguard.test.ts src/laws.test.ts`
Expected: FAIL.

- [ ] **Step 4: Remove the Conductor**

In `src/kanban.ts`:

- Remove the imports on lines 43, 55 and 57-61 that name the conductor and pipeline, keeping `dispatchPrompt` and anything else used elsewhere.
- Remove `conductorMode`, `assignedStage`, `setConductorMode` (lines 156-164), `conductorTick`, `pipelineTick` (390-455) and `lc` if nothing else uses it.
- Remove the button block (1266-1310) and `void conductorTick();` (1322). The watch timer keeps its three polls.
- Replace the restore at 1335-1336 with the note:

```ts
      if (next) {
        const note = conductorGoneNote(loadJSON<unknown>(conductorKey(next.key), null));
        if (note) { topNote(note); try { localStorage.removeItem(conductorKey(next.key)); } catch { /* storage blocked */ } }
      }
```

Keep `conductorKey`, and add at module level:

```ts
/** Said once, to someone whose board used to hand out cards by itself. */
export function conductorGoneNote(saved: unknown): string | null {
  return saved === "semi" || saved === "auto" || saved === "pipeline"
    ? "The Conductor is gone. Cards wait in To do until you hand them to an agent, or give the job to the Founder with Ctrl+Shift+O."
    : null;
}
```

`git rm src/conductor.ts src/conductor.test.ts src/pipeline.ts src/pipeline.test.ts`. Remove `.kb-conductor` and similar at `src/styles/dock.css:68`. In `src/company/engine.ts` remove `isCompanyFloor` and its `sameFolder` import if now unused, and from `engine.test.ts` the assertions on it. The fact they tested, that the Founder's project is not a floor with people, stays covered by `officeModel`'s test from Task 4.

- [ ] **Step 5: Remove the Director**

- `src/panetypes.ts`: remove `role`. `src/crew.ts`: remove `role` from `CliPreset`, with its comment. `src/agentbridge.ts:75`, `src/bridges.ts:217`: remove.
- `src/pane.ts`: remove the `role` parameter of `buildPaneEl`, lines 133-139, `DIRECTOR_LAWS`. Line 349 becomes the constant `BYTEHIVE_LAWS`.
- `src/limitguard.ts:179`: remove `p.spec.role !== "conductor" &&`, and `the Director or` from the comment at 169.
- `src/spawnmodal.ts`: remove the `conductor` argument and everything hanging off it in `spawnCrew`, so it is the plain loop. Remove `director` from `spawnAgents`. Rename `spawnForConductor` to `spawnRequested`, and in `src/bridges.ts:331`. Remove the `DIRECTOR_LAWS` import. Update the header comment.
- `src/laws.ts`: delete `DIRECTOR_LAWS`, and take the word Director out of `BYTEHIVE_LAWS` without changing what it tells a worker: the person outranks anyone who hands them work.
- `src/prefs.ts`, `src/prefsview.ts`, `src/prefs.test.ts`: remove `directorFirst`. `index.html:171`: remove the toggle.
- `src/flow.ts:198`: `No hand-offs yet.<br>When an agent sends work to another, it shows up here.`
- `src/fleetbridge.ts:89`: a comment, say `an agent` instead.
- `mcp/src/server.ts:395`: `for an agent that grows its own crew`.
- `src/styles/canvas.css:248-296`, `396`, `src/styles/inbox.css:228-229`, `234`, `257`, `264`, `274-275`: remove the `[data-role="director"]` rules. Check the selector lists that share a rule keep their other selectors.
- `src/spawnmodal.test.ts`: delete the `directorFirst` cases.
- Tests that use the name `Director` for a sample agent (`bridges.test.ts`, `fleetbridge.test.ts`, `flow.test.ts`, `typing.test.ts`) stay: it is only a name there.

A saved session may still hold `role: "conductor"` in its JSON. Nothing reads it, and it goes the next time the session is saved.

Add to spec section 9, under its table: `Update 2026-11-06: the crew picker is gone. Saved presets and Scheduled agents stay, because they run plain CLIs on a timer and roles do not replace that. See decision D15 in docs/superpowers/plans/2026-11-06-company-phases-2-4.md.`

- [ ] **Step 6: Run everything**

Run: `rtk npm test && rtk npx tsc --noEmit`, from `mcp/`: `rtk proxy npx vitest run`
Expected: PASS. Repeat the search from Step 1: only `conductorKey`, `conductorGoneNote` and test sample names remain.

- [ ] **Step 7: Check a person who never used the Company**

Sandbox empty, no Jobs. In the dev app, on a scratch repo with a board that has `maestro.kanban.conductor.<key>` set to `auto` in localStorage and a card in To do:

| Check | Expected |
|---|---|
| Open the board | One note about the Conductor, not on the next open. The card stays in To do. |
| New agent, a race of 2 | Works, Compare works |
| Hand the card to an agent | The agent gets it, card moves to Doing, `card_done` lands in Done |
| `agent_spawn` from an agent | A new agent starts |
| `fleet_send` broadcast | Reaches everyone |
| Settings | No Director toggle, nothing misaligned |
| A session saved with an agent named Director | Restores, resumes its conversation |
| Save a preset, schedule it a minute ahead | It starts |
| `#office` | Still absent |

- [ ] **Step 8: Commit**

```bash
rtk git add -u src index.html mcp/src docs/superpowers/specs/2026-11-05-company-office-design.md
rtk git commit -m "refactor: remove the Conductor modes and the Director"
```

`-u` stages tracked changes only, so nothing untracked comes along. Check `rtk git status` first.

### Task 19: Remove the orchestrator, the sprites and dead crew styles

**Files:**
- Delete: `orchestrator/`, `tests/`, `pyproject.toml`, `src/assets/sprites/`, `tools/sprite_pack.py`, `tools/sprite_preview.py`, `tools/sprite_slicer.py`
- Modify: `.gitignore:25-30`, `src/styles/modal.css:49-58`

- [ ] **Step 1: Check again that nothing uses them**

```bash
rtk proxy rg -n "orchestrator|sprites|sprite_" . --glob "!orchestrator/**" --glob "!tests/**" --glob "!pyproject.toml" --glob "!tools/sprite_*" --glob "!src/assets/sprites/**" --glob "!node_modules/**" --glob "!dist/**" --glob "!src-tauri/target/**" --glob "!docs/**" --glob "!design/**" --glob "!logs/**" --glob "!.superpowers/**" --glob "!.claude/**"
rtk proxy rg -n "crew-card|class=\"crew" src index.html
```

Expected, from when this plan was written: only `.gitignore`, and for the second search only `src/styles/modal.css`. `design/` mentions the word as prose. `.github/workflows` has no Python step. Anything else stops the task until it is explained.

- [ ] **Step 2: Remove them**

```bash
rtk git rm -r -q orchestrator tests pyproject.toml src/assets/sprites tools/sprite_pack.py tools/sprite_preview.py tools/sprite_slicer.py
```

In `.gitignore`, the heading `# Python orchestrator artifacts` becomes `# Python artifacts` and `uv.lock` goes. `tools/probe6.py` remains, so `__pycache__/` stays. Remove `.crew` and `.crew-card` rules from `src/styles/modal.css`.

- [ ] **Step 3: Build and test**

Run: `rtk npm test && rtk npx tsc --noEmit && rtk proxy npx vite build`
Expected: PASS. `dist/` has no `sprites`.

- [ ] **Step 4: Commit**

```bash
rtk git add .gitignore src/styles/modal.css
rtk git commit -m "chore: remove the Python orchestrator, the unused sprites and crew styles"
```

### Task 20: Release 0.17.0

**Files:**
- Modify: `CHANGELOG.md`, `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`

- [ ] **Step 1: Full test run and one last look**

`rtk npm test`, `rtk npx tsc --noEmit`, from `mcp/`: `rtk proxy npx vitest run` and `rtk npm run build`. Then a real Job through the office in the sandbox, start to merge, so the removals have seen the real path.

- [ ] **Step 2: Docs and version**

CHANGELOG section `## [0.17.0] - <date>`. Say plainly, first: the Conductor and the Director are gone, what to do instead, and that cards no longer move by themselves. Then that saved presets and Scheduled agents stay. Then the rest in one line.

Set `0.17.0` in `package.json`, both places in `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the `bytehive` package in `src-tauri/Cargo.lock`.

```bash
rtk git add CHANGELOG.md package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
rtk git commit -m "chore: version 0.17.0"
```

- [ ] **Step 3: Release**

Merge `feat/office` into `revert/maestro-v0.10.10`, push, tag `v0.17.0`, push the tag. Watch the Windows job until green. Check `latest.json` on `tdat-dev/maestro-releases` says 0.17.0.

---

## Backlog, where each item went

| Item | Task | Why there |
|---|---|---|
| Queue timeout | 2 | Everything the office does goes through the queue |
| Stop ends the running turn | 2 | The rail's Stop must mean it |
| A stopped Job's cards marked | 2 | The wall board shows them |
| Inbox header counts Company rows | 2 | It is the fallback's headline |
| To do cards watched | 2 | Silent loss was seen for real in Phase 1 |
| `card_done` reads committed work | 3 | The Report tab shows it |
| `card_update` can tick checklists | 3 | The Report tab shows it |
| A reused person gets a branch per Job | 16 | Needs people on other CLIs settled first |
| Token counts | 16 | The ledger has to cover what other CLIs report |
| `fleet_send` saying sent for a held message (report item 3) | 2 | One line, and agents were misled by it |

Nothing named in the backlog is left out.

## What this plan leaves out on purpose

- **A small token cap can skip the 80% row** (report item 5). The paused row that follows says more. The watch runs every 30 s, and caps are in the millions in real use.
- **`Permission deny rule "MultiEdit" matches no known tool`** (item 9). Harmless, and older Claude versions have that tool.
- **A person on a paused Job is held even for another Job's message** (item 10). Messages name no Job, and fixing that changes `fleet_send` for a rare case.
- **The Phase 1 minors in `progress.md`** that the office does not touch, such as `drainJsonl` losing a last request, and a running member keeping an old boss in its prompt. They are small, stated in that ledger, and none blocks these phases.
- **A real machine sleep in the gate.** It would stop the owner's machine and the other agent. G8 and G9 stand in, and the result says so.
- **Baked images**, spec section 10's last fallback, as work in this plan. If it comes to that, `OFFICE_3D` is false, the release says so, and it gets a plan of its own.
- **Moving a person to another floor from the office**, free walking, furniture, several companies: out of scope in spec section 1.
- **Fixing `~/.codex/config.toml`.** It is the owner's file, written by their Codex app.

## Self-review against the spec

| Spec | Requirement | Task |
|---|---|---|
| 2 | Floor is a project, top floor the Founder, lobby holds what waits for you | 4, 6, 8. The Needs you rail does the lobby's work: a room with no text in it would duplicate the rail, and section 3 says action stays in text. |
| 4 | Founder has `floor: null`, sits in a project, on the top floor in the office | 4, tested |
| 5 | Only the app writes `company.json`, scene is read only, second app stays out | 1, 4, 12 |
| 5 | Detached window: the Company runs in the main window | 9 |
| 6 | Stop, stuck, stall, To do | 2 |
| 7 | `buildRoleLaunch` returns `{ args, env, files }` and the three-CLI table, prompt through a file | 14, 15 |
| 7 | Resume re-passes role flags | 15: `spec.args` keeps them, `memberSession` wraps them |
| 8 | Three levels on one axis, breadcrumb, Esc rule, terminal passthrough | 4, 8, 9 |
| 8 | Needs you at all levels, a row flies to its place, Inbox stays as a list | 5, 8, 9 |
| 8 | `officeState` table, bubble only for needs, paper plane along the lift | 4, 7 |
| 8 | Honey cell on the roof, shirts by department, props by role, CLI logo on a badge | 7, D5 |
| 9 | Each row of the old mechanisms | 18, 19, with D15 as the one stated deviation |
| 10 | Dynamic import, draw on demand, 30 fps cap, stop when hidden, reduced motion, WebGL fallback, fallback order | 6, 9, 10 |
| 11 | Each phase ships on its own | 12, 17, 20 |
| 12 | Tests first for pure logic, real WebView2 for the UI, real CLIs | Every task; 10, 11, 12, 15 |

Types checked across tasks: `OfficeSource` (5) gains `setLook` in 13; `Level`, `DeskTab` (4) feed 5, 8, 9; `OfficeScene` (6) is what 8 consumes; `ShellHost` (8) is what 9 implements; `LaunchInput` (14) is what 15 fills; `WatchedCard.list` (2) is what 16's `doing` map reads. `Look.edited` is added in 4 and written first in 13.

One conflict in the brief, stated in Global Constraints: "people who never use the Company see no change" cannot hold for Phase 4, which removes things they can see. Phase 4 follows the brief's own narrower line, a working app, and the table at the head of Phase 4 lists every difference.
