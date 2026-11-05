# ByteHive Company, Phase 0 and 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove the two risky assumptions, then ship a working AI company without 3D: you tell a Founder what you want, approve one plan, and it hires role-based Claude Code agents, hands out cards, reports back, and stops at merge.

**Architecture:** A new `src/company/` module holds pure rules (`org.ts`, `reduce.ts`, `watch.ts`, `needs.ts`) and a thin engine that talks to panes and files. Only the app writes `~/.bytehive/company/company.json`; the MCP server queues requests in `requests.jsonl`, the same pattern as today's `spawn-requests.jsonl`. The MCP bundle imports the pure rules straight from `src/company/` (esbuild already bundles app code this way, see `mcp/src/context/stores.ts:13`). Phase 1 UI is a dock panel. The 3D office replaces it in Phase 2, planned after Phase 0 gives numbers.

**Tech Stack:** TypeScript, Vite 8, vitest, Tauri 2, xterm.js, `@modelcontextprotocol/sdk`, zod. No UI framework.

**Spec:** `docs/superpowers/specs/2026-11-05-company-office-design.md`

## Global Constraints

- Never write the character U+2014 (long dash) anywhere: code, comments, UI copy, commit messages.
- UI strings are English, like the rest of the app. Agent-to-agent messages are English.
- Commits carry no `Co-Authored-By: Claude` line. Conventional Commits, one commit per task.
- `fleet_send` from a non-member agent keeps working exactly as it does today.
- Never run `cargo fmt`. Keep each edited file's line endings (BOM-less CRLF or LF).
- Prefix shell commands with `rtk` (for example `rtk npm test`).
- Version must stay above 0.12.13. maestro.* localStorage keys stay.
- Default limits: 5 people per Job, 5 people under one Lead, 20 people in the company, 5,000,000 tokens per Job, stall after 10 minutes, 3 returns per card.
- Phase 1 runs Claude Code only. Codex and OpenCode roles are Phase 3.
- Branch: work on a new branch `feat/company` cut from `revert/maestro-v0.10.10`.
- Do not stage `.superpowers/`, `.playwright-mcp/` or `8bb623.rbf`.

## File Structure

| File | Responsibility |
|---|---|
| `src/company/types.ts` | All company types and `DEFAULT_LIMITS` |
| `src/company/org.ts` | Pure hierarchy rules, shared with the MCP bundle |
| `src/company/store.ts` | Read and write `company.json`, jobs, helper file writes |
| `src/company/roles.ts` | Default role catalog, seeding, loading |
| `src/company/roles/<id>/` | Bundled `role.json`, `prompt.md`, `skills/` |
| `src/company/launch.ts` | Prompt composition, `buildRoleLaunch`, plugin manifest |
| `src/company/reduce.ts` | Pure: request in, new state and effects out |
| `src/company/watch.ts` | Pure: stall and budget decisions |
| `src/company/needs.ts` | Pure: the Needs you rows |
| `src/company/engine.ts` | Runs effects, ticks, owns state |
| `src/company/panel.ts`, `src/company/panel.css` | Temporary dock panel |
| `mcp/src/company.ts` | MCP side: read company, queue requests, register tools |

---

## Phase 0: Risk checks

Phase 0 writes no product code. Results land in `docs/superpowers/specs/2026-11-05-phase0-results.md`. The machine has been short on RAM, so close other agents first.

### Task 0.1: Prove role launch on a real Claude agent

**Files:**
- Create: `docs/superpowers/specs/2026-11-05-phase0-results.md`

- [ ] **Step 1: Build a scratch role folder**

Run in the session scratchpad, not the repo:

```bash
mkdir -p probe/plugin/.claude-plugin probe/plugin/skills/brief-writer
printf '{"name":"probe-role","version":"0.0.1"}' > probe/plugin/.claude-plugin/plugin.json
cat > probe/plugin/skills/brief-writer/SKILL.md <<'EOF'
---
name: brief-writer
description: Use when asked for the probe word.
---
The probe word is HONEYCOMB.
EOF
printf 'You are a marketing specialist. End every reply with the word PROBE-ROLE.' > probe/prompt.md
```

- [ ] **Step 2: Check prompt file, skill and tool limits**

```bash
rtk proxy claude -p "What is the probe word? Then try to create a file named x.txt." \
  --append-system-prompt-file probe/prompt.md --plugin-dir probe/plugin \
  --disallowedTools "Edit,Write,MultiEdit,NotebookEdit" --output-format json
```

Expected: the reply contains HONEYCOMB and PROBE-ROLE, and no `x.txt` exists via the Write tool. Record the session id.

- [ ] **Step 3: Check that resume keeps the role**

```bash
rtk proxy claude -p "Say hello." --resume <session-id> --append-system-prompt-file probe/prompt.md --plugin-dir probe/plugin --output-format json
```

Expected: the reply still ends with PROBE-ROLE.

- [ ] **Step 4: Check the same flags through the app's launch path**

`launchSpec` runs npm CLIs through `cmd /c` (`src/crew.ts:105`). Run:

```bash
cmd //c claude -p "Say hello." --append-system-prompt-file "C:\Users\tvmar\.bytehive\probe prompt.md"
```

Use a copy of `prompt.md` at a path with a space. Expected: PROBE-ROLE in the reply. This proves quoting survives.

- [ ] **Step 5: Write the results and decide**

Create `docs/superpowers/specs/2026-11-05-phase0-results.md` with one row per check: command, result, pass or fail.

Decision rule used by Task 4: if `--append-system-prompt-file` failed, `buildRoleLaunch` uses `--append-system-prompt` with the text stripped of `& | < > % ! ^ ( ) " '` and newlines, capped at 6000 characters. If `--plugin-dir` failed, skills are appended into the prompt file.

- [ ] **Step 6: Commit**

```bash
rtk git add docs/superpowers/specs/2026-11-05-phase0-results.md
rtk git commit -m "docs: phase 0 role launch results"
```

### Task 0.2: Measure WebGL next to terminals in WebView2

**Files (throwaway branch `spike/office-webgl`, never merged):**
- Create: `src/spikeoffice.ts`
- Modify: `src/main.ts` (one import)

- [ ] **Step 1: Branch and install three**

```bash
rtk git switch -c spike/office-webgl
rtk npm install three@0.170.0 @types/three@0.170.0
```

- [ ] **Step 2: Add the spike scene**

```ts
// src/spikeoffice.ts
import * as THREE from "three";

const gaps: number[] = [];
const long: number[] = [];
let lost = 0;
let restored = 0;

export function startSpike(): void {
  const c = document.createElement("canvas");
  c.style.cssText = "position:fixed;left:0;top:0;width:45vw;height:60vh;z-index:40;pointer-events:none";
  document.body.appendChild(c);
  c.addEventListener("webglcontextlost", (e) => { e.preventDefault(); lost += 1; });
  c.addEventListener("webglcontextrestored", () => { restored += 1; });
  const r = new THREE.WebGLRenderer({ canvas: c, antialias: true, powerPreference: "low-power" });
  r.setSize(c.clientWidth, c.clientHeight, false);
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x14161c);
  const cam = new THREE.PerspectiveCamera(35, c.clientWidth / c.clientHeight, 0.1, 100);
  cam.position.set(9, 8, 12);
  cam.lookAt(0, 3, 0);
  scene.add(new THREE.HemisphereLight(0xfff2d8, 0x1d2029, 2));
  const slab = new THREE.MeshStandardMaterial({ color: 0x2a2e3a });
  const desk = new THREE.MeshStandardMaterial({ color: 0x8a6a3a });
  const body = new THREE.MeshStandardMaterial({ color: 0x5fd08a });
  for (let f = 0; f < 5; f += 1) {
    const s = new THREE.Mesh(new THREE.BoxGeometry(8, 0.3, 6), slab);
    s.position.y = f * 2;
    scene.add(s);
    for (let i = 0; i < 6; i += 1) {
      const d = new THREE.Mesh(new THREE.BoxGeometry(0.9, 0.4, 0.6), desk);
      d.position.set(-3 + i * 1.2, f * 2 + 0.4, 0);
      const p = new THREE.Mesh(new THREE.CapsuleGeometry(0.2, 0.3, 4, 8), body);
      p.position.set(-3 + i * 1.2, f * 2 + 0.7, -0.6);
      scene.add(d, p);
    }
  }
  let last = performance.now();
  let moving = true;
  const frame = (t: number) => {
    gaps.push(t - last);
    last = t;
    if (gaps.length > 20000) gaps.shift();
    if (moving) {
      cam.position.x = 9 * Math.cos(t / 4000);
      cam.position.z = 12 * Math.sin(t / 4000);
      cam.lookAt(0, 3, 0);
      r.render(scene, cam);
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  new PerformanceObserver((l) => l.getEntries().forEach((e) => long.push(e.duration)))
    .observe({ entryTypes: ["longtask"] });
  (window as unknown as Record<string, unknown>).__spike = {
    stats: () => ({
      maxGap: Math.max(...gaps),
      over200: gaps.filter((g) => g > 200).length,
      longMax: Math.max(0, ...long),
      lost,
      restored,
    }),
    still: (v: boolean) => { moving = !v; if (v) r.render(scene, cam); },
    reset: () => { gaps.length = 0; long.length = 0; },
  };
}
```

Add at the end of `src/main.ts`:

```ts
import("./spikeoffice").then((m) => m.startSpike());
```

- [ ] **Step 3: Run the real app and load it**

Run `rtk npm run tauri:dev`. Open 12 Shell agents across two projects. The dev UI exposes CDP on port 9333, so `window.__spike.stats()` can be read there.

- [ ] **Step 4: Measure the three gates**

1. Call `__spike.reset()`. Open and close the New agent modal 10 times, switch project tabs 10 times, resize the window 10 times. Read `stats()`. Pass: `over200` is 0.
2. Call `__spike.still(true)` and `__spike.reset()`. Leave the app idle for 1 hour. Pass: `lost` equals 0, or `restored` equals `lost`, and the picture is back within 2 seconds.
3. With the scene still, read CPU for the `ByteHive` process tree in Task Manager for 1 minute. Pass: under 5%.

- [ ] **Step 5: Record, and try the fallback if needed**

Append the numbers to `docs/superpowers/specs/2026-11-05-phase0-results.md` on `feat/company`. If a gate fails, repeat with an OffscreenCanvas in a Worker, then with a second Tauri window, in that order. Record which one passed, or that only the baked Blender route remains.

- [ ] **Step 6: Drop the spike**

```bash
rtk git switch revert/maestro-v0.10.10
rtk git branch -D spike/office-webgl
```

Then commit the results file on `feat/company` with `docs: phase 0 WebGL measurements`.

---

## Phase 1: The company, no 3D

### Task 1: Agents in a worktree must reach the project's board

Today `BYTEHIVE_WORKSPACE` is set to the run folder (`src/pane.ts:349`), which is the worktree for an isolated agent. The MCP server then uses `<worktree>/.bytehive`, while the app polls `<project>/.bytehive`. Every company worker runs in a worktree, so this must be fixed first.

**Files:**
- Modify: `src/workspaces.ts`, `src/pane.ts:347-350`, `src/panetypes.ts:8-24`
- Test: `src/workspaces.test.ts`

**Interfaces:**
- Produces: `agentEnv(spec: Pick<AgentSpec, "name" | "cwd" | "env">, ranIn: string | null): Array<[string, string]>`
- Produces: `AgentSpec.env?: Array<[string, string]>`, `AgentSpec.personId?: string`

- [ ] **Step 1: Write the failing test**

Add to `src/workspaces.test.ts` (create the file if missing, with `import { describe, it, expect } from "vitest";`):

```ts
import { agentEnv } from "./workspaces";

describe("agentEnv", () => {
  it("pins the workspace to the project even when the agent runs in a worktree", () => {
    const env = agentEnv({ name: "Ana", cwd: "D:\\app" }, "D:\\.bytehive-worktrees\\app-1\\ana");
    expect(env).toContainEqual(["BYTEHIVE_WORKSPACE", "D:\\app"]);
    expect(env).toContainEqual(["BYTEHIVE_AGENT", "Ana"]);
  });

  it("falls back to the run folder for an agent with no project", () => {
    expect(agentEnv({ name: "Ana", cwd: null }, "C:\\Users\\me")).toContainEqual(["BYTEHIVE_WORKSPACE", "C:\\Users\\me"]);
  });

  it("adds the spec's own variables", () => {
    const env = agentEnv({ name: "Ana", cwd: "D:\\app", env: [["BYTEHIVE_AGENT_ID", "p1"]] }, null);
    expect(env).toContainEqual(["BYTEHIVE_AGENT_ID", "p1"]);
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `rtk npx vitest run src/workspaces.test.ts`
Expected: FAIL, `agentEnv` is not exported.

- [ ] **Step 3: Implement**

In `src/panetypes.ts`, add to `AgentSpec` after `ranIn`:

```ts
  env?: Array<[string, string]>; // extra variables for this agent's process
  personId?: string; // set for a member of the company: its Person id
```

In `src/workspaces.ts` add:

```ts
import type { AgentSpec } from "./panetypes";

/** The variables every agent process gets. BYTEHIVE_WORKSPACE is the project,
 *  never the worktree: the app reads the board and the outbox in the project. */
export function agentEnv(spec: Pick<AgentSpec, "name" | "cwd" | "env">, ranIn: string | null): Array<[string, string]> {
  return [
    ["BYTEHIVE_AGENT", spec.name],
    ["BYTEHIVE_WORKSPACE", spec.cwd ?? ranIn ?? ""],
    ...(spec.env ?? []),
  ];
}
```

In `src/pane.ts` replace lines 347-350 with:

```ts
      const envPairs = agentEnv(spec, cwd);
```

and extend the import on line 31 to `import { basename, runDir, agentEnv } from "./workspaces";`.

- [ ] **Step 4: Run the tests**

Run: `rtk npx vitest run src/workspaces.test.ts src/pane`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src/workspaces.ts src/workspaces.test.ts src/pane.ts src/panetypes.ts
rtk git commit -m "fix: agents in a worktree reach the project's board"
```

### Task 2: Company types and hierarchy rules

**Files:**
- Create: `src/company/types.ts`, `src/company/org.ts`
- Test: `src/company/org.test.ts`

**Interfaces:**
- Produces, from `types.ts`: `FOUNDER_ID`, `Rank`, `Look`, `Floor`, `Person`, `Limits`, `DEFAULT_LIMITS`, `Company`, `RoleDef`, `Hire`, `PlanCard`, `JobPlan`, `JobStatus`, `Job`, `CompanyRequest`.
- Produces, from `org.ts`: `emptyCompany(): Company`, `personById(c, id)`, `personByName(c, name)`, `reportsOf(c, id): Person[]`, `canMessage(c, fromId, toId): boolean`, `resolveTargets(c, fromId, to: string | null): { people: Person[] } | { error: string }`, `validatePlan(c, plan, roles): string[]`, `applyHires(c, plan, roles, pick): { company: Company; hired: Person[] }`.

- [ ] **Step 1: Write the types**

```ts
// src/company/types.ts
export const FOUNDER_ID = "founder";

export type Rank = "founder" | "lead" | "staff";

export interface Look { shirt: string; hair: string; prop: string }

export interface Floor { id: string; dir: string; name: string }

export interface Person {
  id: string;
  name: string;
  role: string;
  rank: Rank;
  floor: string | null; // null for the Founder
  boss: string | null; // Person id, null for the Founder
  cli: string;
  look: Look;
  session?: string;
}

export interface Limits {
  peoplePerJob: number;
  peoplePerLead: number;
  peopleTotal: number;
  tokensPerJob: number;
  stallMinutes: number;
  maxReturns: number;
}

export const DEFAULT_LIMITS: Limits = {
  peoplePerJob: 5,
  peoplePerLead: 5,
  peopleTotal: 20,
  tokensPerJob: 5_000_000,
  stallMinutes: 10,
  maxReturns: 3,
};

export interface Company {
  version: 1;
  floors: Floor[];
  people: Person[];
  limits: Limits;
  nextJob: number;
}

export interface RoleDef {
  id: string;
  title: string;
  department: string;
  rank: Rank;
  cli: string;
  denyTools: string[];
  look: Look;
  prompt: string;
  skills: string[]; // folder names under the role's skills/
}

export interface Hire { role: string; floor: string }

export interface PlanCard {
  floor: string;
  title: string;
  desc: string;
  checklist: string[];
  assignee?: string; // Person name, an existing person or left for the Lead
}

export interface JobPlan {
  summary: string;
  cards: PlanCard[];
  hires: Hire[];
  tokenCap?: number;
}

export type JobStatus = "planning" | "awaiting-plan" | "running" | "awaiting-merge" | "paused" | "done" | "rejected";

export interface Job {
  id: string;
  title: string;
  ask: string;
  status: JobStatus;
  plan: JobPlan | null;
  tokenCap: number;
  tokensUsed: number;
  tokenBase: Record<string, number>; // Person id to their fresh tokens when the Job began
  people: string[]; // Person ids working this Job
  summary?: string;
  createdAt: number;
}

export type CompanyRequest =
  | { type: "message"; ts: number; from: string; to: string | null; message: string }
  | { type: "plan"; ts: number; from: string; job: string; plan: JobPlan }
  | { type: "role"; ts: number; from: string; role: RoleDef; job: string }
  | { type: "job-done"; ts: number; from: string; job: string; summary: string };
```

- [ ] **Step 2: Write the failing tests**

```ts
// src/company/org.test.ts
import { describe, it, expect } from "vitest";
import { applyHires, canMessage, emptyCompany, personByName, reportsOf, resolveTargets, validatePlan } from "./org";
import { DEFAULT_LIMITS, FOUNDER_ID, type Company, type JobPlan, type Person, type RoleDef } from "./types";

const role = (id: string, rank: "lead" | "staff"): RoleDef => ({
  id, title: id, department: "x", rank, cli: "claude", denyTools: [], look: { shirt: "#fff", hair: "#000", prop: "none" }, prompt: "p", skills: [],
});
const roles = [role("lead", "lead"), role("developer", "staff"), role("marketing", "staff")];
const person = (id: string, rank: Person["rank"], floor: string | null, boss: string | null): Person => ({
  id, name: id.toUpperCase(), role: "developer", rank, floor, boss, cli: "claude", look: role("x", "staff").look,
});
const company = (people: Person[]): Company => ({
  ...emptyCompany(),
  floors: [{ id: "a", dir: "D:\\a", name: "a" }, { id: "b", dir: "D:\\b", name: "b" }],
  people: [...emptyCompany().people, ...people],
});
const plan = (over: Partial<JobPlan>): JobPlan => ({ summary: "s", cards: [], hires: [], ...over });
let n = 0;
const pick = { name: () => `N${(n += 1)}`, id: () => `id${n}` };

describe("canMessage", () => {
  const c = company([person("l1", "lead", "a", FOUNDER_ID), person("d1", "staff", "a", "l1"), person("d2", "staff", "a", "l1"), person("l2", "lead", "b", FOUNDER_ID)]);
  it("lets a person reach their boss and their reports", () => {
    expect(canMessage(c, "d1", "l1")).toBe(true);
    expect(canMessage(c, "l1", "d1")).toBe(true);
    expect(canMessage(c, FOUNDER_ID, "l1")).toBe(true);
  });
  it("stops peers, skips and other floors", () => {
    expect(canMessage(c, "d1", "d2")).toBe(false);
    expect(canMessage(c, "d1", FOUNDER_ID)).toBe(false);
    expect(canMessage(c, "l1", "l2")).toBe(false);
  });
});

describe("resolveTargets", () => {
  const c = company([person("l1", "lead", "a", FOUNDER_ID), person("d1", "staff", "a", "l1")]);
  it("sends a message with no target to the boss, or the reports of a person with none above", () => {
    expect(resolveTargets(c, "d1", null)).toEqual({ people: [personByName(c, "L1")] });
    expect(resolveTargets(c, FOUNDER_ID, null)).toEqual({ people: reportsOf(c, FOUNDER_ID) });
  });
  it("explains a refusal in words the agent can act on", () => {
    const r = resolveTargets(c, "d1", "Founder");
    expect(r).toEqual({ error: expect.stringContaining("L1") });
  });
  it("reports an unknown name", () => {
    expect(resolveTargets(c, "d1", "Ghost")).toEqual({ error: expect.stringContaining("Ghost") });
  });
});

describe("validatePlan", () => {
  it("accepts a one person hire on an empty floor", () => {
    expect(validatePlan(company([]), plan({ hires: [{ role: "marketing", floor: "a" }] }), roles)).toEqual([]);
  });
  it("needs a Lead once a floor reaches two people", () => {
    const errors = validatePlan(company([]), plan({ hires: [{ role: "developer", floor: "a" }, { role: "developer", floor: "a" }] }), roles);
    expect(errors.join()).toContain("Lead");
  });
  it("rejects an unknown floor, an unknown role, and too many hires", () => {
    const five = Array.from({ length: 6 }, () => ({ role: "developer", floor: "a" }));
    const errors = validatePlan(company([]), plan({ hires: [...five, { role: "lead", floor: "a" }, { role: "ghost", floor: "z" }] }), roles);
    expect(errors.join("|")).toMatch(/floor/);
    expect(errors.join("|")).toMatch(/role/);
    expect(errors.join("|")).toContain(String(DEFAULT_LIMITS.peoplePerJob));
  });
  it("rejects a card on an unknown floor", () => {
    expect(validatePlan(company([]), plan({ cards: [{ floor: "z", title: "t", desc: "", checklist: [] }] }), roles).join()).toMatch(/floor/);
  });
});

describe("applyHires", () => {
  it("reports a lone hire to the Founder", () => {
    const { hired } = applyHires(company([]), plan({ hires: [{ role: "marketing", floor: "a" }] }), roles, pick);
    expect(hired[0].boss).toBe(FOUNDER_ID);
    expect(hired[0].rank).toBe("staff");
  });
  it("puts staff under the floor's Lead and moves a lone worker under a new Lead", () => {
    const c = company([person("d1", "staff", "a", FOUNDER_ID)]);
    const { company: next, hired } = applyHires(c, plan({ hires: [{ role: "lead", floor: "a" }, { role: "developer", floor: "a" }] }), roles, pick);
    const lead = hired.find((p) => p.rank === "lead")!;
    expect(lead.boss).toBe(FOUNDER_ID);
    expect(next.people.find((p) => p.id === "d1")!.boss).toBe(lead.id);
    expect(hired.find((p) => p.rank === "staff")!.boss).toBe(lead.id);
  });
});
```

- [ ] **Step 3: Run to see it fail**

Run: `rtk npx vitest run src/company/org.test.ts`
Expected: FAIL, cannot resolve `./org`.

- [ ] **Step 4: Implement**

```ts
// src/company/org.ts
/* The company's rules, with no DOM and no IPC: the app and the MCP bundle
 * both import this, so what an agent is told and what the app enforces
 * cannot drift apart. */
import { DEFAULT_LIMITS, FOUNDER_ID, type Company, type JobPlan, type Person, type RoleDef } from "./types";

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

export function emptyCompany(): Company {
  const founder: Person = {
    id: FOUNDER_ID, name: "Founder", role: "founder", rank: "founder", floor: null, boss: null,
    cli: "claude", look: { shirt: "#f5b83d", hair: "#2a2e3a", prop: "none" },
  };
  return { version: 1, floors: [], people: [founder], limits: { ...DEFAULT_LIMITS }, nextJob: 1 };
}

export const personById = (c: Company, id: string) => c.people.find((p) => p.id === id);
export const personByName = (c: Company, name: string) => c.people.find((p) => same(p.name, name));
export const reportsOf = (c: Company, id: string) => c.people.filter((p) => p.boss === id);

/** A person talks to their boss and to their own reports, nobody else. */
export function canMessage(c: Company, fromId: string, toId: string): boolean {
  const from = personById(c, fromId);
  const to = personById(c, toId);
  if (!from || !to || from.id === to.id) return false;
  return from.boss === to.id || to.boss === from.id;
}

export function resolveTargets(c: Company, fromId: string, to: string | null): { people: Person[] } | { error: string } {
  const from = personById(c, fromId);
  if (!from) return { error: `You are not on the company roster (${fromId}).` };
  if (to === null) {
    const boss = from.boss ? personById(c, from.boss) : undefined;
    return { people: boss ? [boss] : reportsOf(c, from.id) };
  }
  const target = personByName(c, to);
  if (!target) return { error: `Nobody named ${to} works here. See company_status.` };
  if (canMessage(c, from.id, target.id)) return { people: [target] };
  const boss = from.boss ? personById(c, from.boss) : undefined;
  return { error: `You cannot message ${target.name} directly. Send it to ${boss?.name ?? "your reports"} and they will pass it on.` };
}

const floorPeople = (c: Company, floor: string) => c.people.filter((p) => p.floor === floor);

export function validatePlan(c: Company, plan: JobPlan, roles: RoleDef[]): string[] {
  const errors: string[] = [];
  const known = new Set(c.floors.map((f) => f.id));
  const roleOf = (id: string) => roles.find((r) => r.id === id);
  if (plan.hires.length > c.limits.peoplePerJob) errors.push(`A plan may hire at most ${c.limits.peoplePerJob} people.`);
  if (c.people.length + plan.hires.length > c.limits.peopleTotal) errors.push(`The company is limited to ${c.limits.peopleTotal} people.`);
  for (const h of plan.hires) {
    if (!known.has(h.floor)) errors.push(`Unknown floor ${h.floor}.`);
    if (!roleOf(h.role)) errors.push(`Unknown role ${h.role}. See role_list or propose one with role_propose.`);
  }
  for (const card of plan.cards) if (!known.has(card.floor)) errors.push(`Card "${card.title}" is on unknown floor ${card.floor}.`);
  for (const floor of known) {
    const hires = plan.hires.filter((h) => h.floor === floor);
    if (!hires.length) continue;
    const now = floorPeople(c, floor);
    const leads = now.filter((p) => p.rank === "lead").length + hires.filter((h) => roleOf(h.role)?.rank === "lead").length;
    const total = now.length + hires.length;
    if (total >= 2 && leads === 0) errors.push(`Floor ${floor} will have ${total} people and needs a Lead: add a hire with role lead.`);
    if (leads > 1) errors.push(`Floor ${floor} can only have one Lead.`);
    if (total - leads > c.limits.peoplePerLead) errors.push(`A Lead can have at most ${c.limits.peoplePerLead} people.`);
  }
  return errors;
}

export function applyHires(
  c: Company,
  plan: JobPlan,
  roles: RoleDef[],
  pick: { name: (taken: string[]) => string; id: () => string },
): { company: Company; hired: Person[] } {
  const people = c.people.map((p) => ({ ...p }));
  const hired: Person[] = [];
  const add = (roleId: string, floor: string, boss: string) => {
    const role = roles.find((r) => r.id === roleId)!;
    const person: Person = {
      id: pick.id(), name: pick.name(people.map((p) => p.name)), role: role.id,
      rank: role.rank === "lead" ? "lead" : "staff", floor, boss, cli: role.cli, look: role.look,
    };
    people.push(person);
    hired.push(person);
    return person;
  };
  const isLead = (roleId: string) => roles.find((r) => r.id === roleId)?.rank === "lead";
  for (const h of plan.hires.filter((x) => isLead(x.role))) {
    const lead = add(h.role, h.floor, FOUNDER_ID);
    for (const p of people) if (p.floor === h.floor && p.id !== lead.id) p.boss = lead.id;
  }
  for (const h of plan.hires.filter((x) => !isLead(x.role))) {
    const lead = people.find((p) => p.floor === h.floor && p.rank === "lead");
    add(h.role, h.floor, lead?.id ?? FOUNDER_ID);
  }
  return { company: { ...c, people }, hired };
}
```

- [ ] **Step 5: Run the tests**

Run: `rtk npx vitest run src/company/org.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/company/types.ts src/company/org.ts src/company/org.test.ts
rtk git commit -m "feat: company types and hierarchy rules"
```

### Task 3: Store and role catalog

**Files:**
- Create: `src/company/store.ts`, `src/company/roles.ts`, `src/company/roles/<id>/role.json`, `src/company/roles/<id>/prompt.md`, `src/company/roles/<id>/skills/<skill>/SKILL.md`
- Test: `src/company/store.test.ts`, `src/company/roles.test.ts`

**Interfaces:**
- Consumes: types from Task 2.
- Produces: `companyRoot(home: string): string`, `ensureDirs(root, rel)`, `writeNew(root, rel, content)`, `loadCompany(root): Promise<Company>`, `saveCompany(root, c)`, `loadJobs(root): Promise<Job[]>`, `saveJob(root, job)`, `parseCompany(text): Company`.
- Produces: `DEFAULT_ROLES: RoleDef[]`, `seedRoles(root)`, `loadRoles(root): Promise<RoleDef[]>`, `skillFiles(role): Array<{ name: string; text: string }>`.

Role ids to create: `founder`, `lead`, `developer`, `qa`, `designer`, `marketing`, `copywriter`, `researcher`. `founder` and `lead` set `"denyTools": ["Edit", "Write", "MultiEdit", "NotebookEdit"]`.

- [ ] **Step 1: Write the failing tests**

```ts
// src/company/store.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";

const files = new Map<string, string>();
const key = (root: string, rel: string) => `${root}|${rel}`;
vi.mock("../ipc", () => ({
  fsReadFile: async (root: string, rel: string) => {
    const content = files.get(key(root, rel));
    if (content === undefined) throw { Failed: "no such path: gone" };
    return { content, mtime: 1 };
  },
  fsWriteFile: async (root: string, rel: string, content: string) => { files.set(key(root, rel), content); return { mtime: 2 }; },
  fsCreateFile: async (root: string, rel: string) => { if (!files.has(key(root, rel))) files.set(key(root, rel), ""); },
  fsCreateDir: async () => {},
  fsReadDir: async (root: string) => [...files.keys()]
    .filter((k) => k.startsWith(`${root}|jobs\\`))
    .map((k) => ({ name: k.split("\\").pop()!, is_dir: false })),
}));

import { companyRoot, loadCompany, loadJobs, parseCompany, saveCompany, saveJob } from "./store";
import { emptyCompany } from "./org";
import type { Job } from "./types";

beforeEach(() => files.clear());

describe("store", () => {
  it("puts the company under the home folder", () => {
    expect(companyRoot("C:\\Users\\me\\")).toBe("C:\\Users\\me\\.bytehive\\company");
  });
  it("starts an empty company when there is no file", async () => {
    expect((await loadCompany("R")).people[0].name).toBe("Founder");
  });
  it("round trips the company", async () => {
    const c = { ...emptyCompany(), nextJob: 7 };
    await saveCompany("R", c);
    expect((await loadCompany("R")).nextJob).toBe(7);
  });
  it("refuses a corrupt file instead of starting over", () => {
    expect(() => parseCompany("{oops")).toThrow();
  });
  it("round trips a job", async () => {
    const job: Job = { id: "J-0001", title: "t", ask: "a", status: "planning", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 1 };
    await saveJob("R", job);
    expect((await loadJobs("R"))[0].id).toBe("J-0001");
  });
});
```

Check the real `FsEntry` field names in `src/ipc.ts` and match the mock and `loadJobs` to them.

```ts
// src/company/roles.test.ts
import { describe, it, expect } from "vitest";
import { DEFAULT_ROLES, skillFiles } from "./roles";

describe("default roles", () => {
  it("covers the starting catalog", () => {
    expect(DEFAULT_ROLES.map((r) => r.id).sort()).toEqual(
      ["copywriter", "designer", "developer", "founder", "lead", "marketing", "qa", "researcher"],
    );
  });
  it("gives every role a prompt", () => {
    for (const r of DEFAULT_ROLES) expect(r.prompt.length).toBeGreaterThan(80);
  });
  it("forbids the Founder and the Lead from editing files", () => {
    for (const id of ["founder", "lead"]) expect(DEFAULT_ROLES.find((r) => r.id === id)!.denyTools).toContain("Edit");
  });
  it("ships a real skill for marketing", () => {
    const m = DEFAULT_ROLES.find((r) => r.id === "marketing")!;
    expect(skillFiles(m).map((s) => s.name)).toContain("campaign-brief");
    expect(skillFiles(m)[0].text).toMatch(/^---\nname: /);
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `rtk npx vitest run src/company/store.test.ts src/company/roles.test.ts`
Expected: FAIL, modules not found.

- [ ] **Step 3: Implement the store**

```ts
// src/company/store.ts
/* company.json and the Job files. Only the app writes these; the MCP server
 * sends requests instead, so two processes never write the same file. */
import { fsCreateDir, fsCreateFile, fsReadDir, fsReadFile, fsWriteFile } from "../ipc";
import { emptyCompany } from "./org";
import type { Company, Job } from "./types";

const COMPANY_REL = "company.json";
const missing = (e: unknown) => /no such path/i.test(typeof e === "string" ? e : String((e as { Failed?: string })?.Failed ?? ""));

export function companyRoot(home: string): string {
  return `${home.replace(/[\\/]+$/, "")}\\.bytehive\\company`;
}

/** Create each folder of `rel` under `root`; a folder already there is fine. */
export async function ensureDirs(root: string, rel: string): Promise<void> {
  let at = "";
  for (const part of rel.split(/[\\/]/).filter(Boolean)) {
    at = at ? `${at}\\${part}` : part;
    await fsCreateDir(root, at).catch(() => {});
  }
}

/** Write a file, making it and its folders first. fs_write_file needs it to exist. */
export async function writeNew(root: string, rel: string, content: string): Promise<void> {
  const cut = rel.lastIndexOf("\\");
  if (cut > 0) await ensureDirs(root, rel.slice(0, cut));
  await fsCreateFile(root, rel).catch(() => {});
  await fsWriteFile(root, rel, content, null);
}

export function parseCompany(text: string): Company {
  const c = JSON.parse(text) as Company;
  if (c?.version !== 1 || !Array.isArray(c.people) || !Array.isArray(c.floors)) throw new Error("company.json has the wrong shape");
  return c;
}

export async function loadCompany(root: string): Promise<Company> {
  try {
    return parseCompany((await fsReadFile(root, COMPANY_REL)).content);
  } catch (e) {
    if (missing(e)) return emptyCompany();
    throw e;
  }
}

export const saveCompany = (root: string, c: Company) => writeNew(root, COMPANY_REL, JSON.stringify(c, null, 2));
export const saveJob = (root: string, job: Job) => writeNew(root, `jobs\\${job.id}.json`, JSON.stringify(job, null, 2));

export async function loadJobs(root: string): Promise<Job[]> {
  await ensureDirs(root, "jobs");
  const entries = await fsReadDir(root, "jobs");
  const out: Job[] = [];
  for (const e of entries) {
    if (!e.name.endsWith(".json")) continue;
    try {
      out.push(JSON.parse((await fsReadFile(root, `jobs\\${e.name}`)).content) as Job);
    } catch { /* a half-written Job is read again next start */ }
  }
  return out.sort((a, b) => a.createdAt - b.createdAt);
}
```

The root folder `~/.bytehive/company` must exist before these calls. The engine creates it with `fsCreateDir(home, ".bytehive")` and `fsCreateDir(home, ".bytehive\\company")` (Task 6).

- [ ] **Step 4: Write the role files**

Every `role.json` has this shape (values change per role):

```json
{
  "id": "marketing",
  "title": "Marketing",
  "department": "Marketing",
  "rank": "staff",
  "cli": "claude",
  "denyTools": [],
  "look": { "shirt": "#e0913a", "hair": "#2a2e3a", "prop": "megaphone" },
  "skills": ["campaign-brief"]
}
```

`src/company/roles/marketing/prompt.md`:

```markdown
You are the marketer of this company. You turn a product into words and plans that reach real buyers.

Start from who the buyer is and what they are trying to get done. Read the product's README, landing page and past copy before you write anything. Give concrete output: positioning, a campaign brief, channel plan, headlines, posts. Say what you would measure. Never invent customers, numbers or quotes. Write like a person who runs a shop, not like an ad.
```

`src/company/roles/marketing/skills/campaign-brief/SKILL.md`:

```markdown
---
name: campaign-brief
description: Use when asked to plan a launch, campaign or content push. Produces a one page brief.
---

Write a one page brief with these parts, in order:

1. Who it is for: one specific buyer.
2. What they want, in their words.
3. The one thing to say.
4. Why they should believe it: real facts from the product only.
5. Where it runs and what to make for each place.
6. What to measure and when to check.

If a fact is missing, list it as a question instead of guessing.
```

Write the other seven the same way, each prompt at least 80 characters, with these skills:

| Role | Rank | denyTools | prop | shirt | Skill | Prompt focus |
|---|---|---|---|---|---|---|
| founder | founder | Edit, Write, MultiEdit, NotebookEdit | none | #f5b83d | none | Plans, hires, delegates, never codes |
| lead | lead | Edit, Write, MultiEdit, NotebookEdit | clipboard | #5ec2f0 | none | Splits cards, one writer per file area, reviews, reports |
| developer | staff | none | laptop | #5fd08a | none | Works one card on its branch, tests before done |
| qa | staff | none | magnifier | #b18cf0 | `test-report` | Reads the change fresh, runs it, reports pass or fail with evidence |
| designer | staff | none | pencil | #ff6b6b | none | Hierarchy, spacing, restraint, checks in the real UI |
| copywriter | staff | none | pen | #e6c84a | none | Plain words, no AI tone |
| researcher | staff | none | book | #3ad29f | none | Finds sources, separates fact from guess |

`src/company/roles/qa/skills/test-report/SKILL.md`:

```markdown
---
name: test-report
description: Use when checking finished work. Produces a pass or fail report with evidence.
---

Report in this order: what you ran, what you expected, what happened, verdict.

Run the change yourself. Never pass work you only read. Quote the command and its real output. A failure names the file and the smallest way to reproduce it.
```

- [ ] **Step 5: Implement the catalog**

```ts
// src/company/roles.ts
/* The roles ByteHive ships with. A copy lands in ~/.bytehive/company/roles the
 * first time, and that copy wins, so a role you edit stays edited. */
import { fsReadDir, fsReadFile } from "../ipc";
import { writeNew } from "./store";
import type { RoleDef } from "./types";

const jsons = import.meta.glob("./roles/*/role.json", { eager: true, import: "default" }) as Record<string, Omit<RoleDef, "prompt">>;
const prompts = import.meta.glob("./roles/*/prompt.md", { eager: true, query: "?raw", import: "default" }) as Record<string, string>;
const skills = import.meta.glob("./roles/*/skills/*/SKILL.md", { eager: true, query: "?raw", import: "default" }) as Record<string, string>;

export const DEFAULT_ROLES: RoleDef[] = Object.entries(jsons).map(([path, def]) => ({
  ...def,
  prompt: prompts[path.replace("role.json", "prompt.md")],
}));

export function skillFiles(role: RoleDef): Array<{ name: string; text: string }> {
  return role.skills.flatMap((name) => {
    const text = skills[`./roles/${role.id}/skills/${name}/SKILL.md`];
    return text ? [{ name, text }] : [];
  });
}

export async function seedRoles(root: string): Promise<void> {
  for (const role of DEFAULT_ROLES) {
    const exists = await fsReadFile(root, `roles\\${role.id}\\role.json`).then(() => true, () => false);
    if (exists) continue;
    const { prompt, ...def } = role;
    await writeNew(root, `roles\\${role.id}\\role.json`, JSON.stringify(def, null, 2));
    await writeNew(root, `roles\\${role.id}\\prompt.md`, prompt);
    for (const s of skillFiles(role)) await writeNew(root, `roles\\${role.id}\\skills\\${s.name}\\SKILL.md`, s.text);
  }
}

export async function loadRoles(root: string): Promise<RoleDef[]> {
  const out: RoleDef[] = [];
  for (const e of await fsReadDir(root, "roles")) {
    try {
      const def = JSON.parse((await fsReadFile(root, `roles\\${e.name}\\role.json`)).content) as Omit<RoleDef, "prompt">;
      out.push({ ...def, prompt: (await fsReadFile(root, `roles\\${e.name}\\prompt.md`)).content });
    } catch { /* a role folder missing a file is not offered */ }
  }
  return out;
}
```

If `import.meta.glob` keys or JSON default imports differ under vitest, adjust the glob options until the test passes. Keep the exports the same.

- [ ] **Step 6: Run the tests**

Run: `rtk npx vitest run src/company`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add src/company
rtk git commit -m "feat: company store and role catalog"
```

### Task 4: Laws by rank and role launch

**Files:**
- Create: `src/company/launch.ts`
- Modify: `src/laws.ts`, `src/pane.ts:337-341`, `src/spawnmodal.ts:248-250`
- Test: `src/company/launch.test.ts`

**Interfaces:**
- Consumes: `Person`, `RoleDef`, `Company` (Task 2), `AgentSpec.env`, `AgentSpec.personId` (Task 1), the Task 0.1 decision.
- Produces: `FOUNDER_LAWS`, `LEAD_LAWS`, `STAFF_LAWS` in `src/laws.ts`.
- Produces: `composePrompt(person, role, c): string`, `buildRoleLaunch(o: { person: Person; role: RoleDef; promptFile: string; pluginDir: string | null }): { args: string[]; env: Array<[string, string]> }`, `pluginManifest(person): string`.
- Produces: `prepareIsolation(ws, force = false)`.

- [ ] **Step 1: Write the failing test**

```ts
// src/company/launch.test.ts
import { describe, it, expect } from "vitest";
import { buildRoleLaunch, composePrompt, pluginManifest } from "./launch";
import { emptyCompany } from "./org";
import { FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS } from "../laws";
import { FOUNDER_ID, type Person, type RoleDef } from "./types";

const look = { shirt: "#fff", hair: "#000", prop: "none" };
const role: RoleDef = { id: "marketing", title: "Marketing", department: "Marketing", rank: "staff", cli: "claude", denyTools: [], look, prompt: "You are the marketer.", skills: ["campaign-brief"] };
const person: Person = { id: "p1", name: "Vy", role: "marketing", rank: "staff", floor: "gcare", boss: FOUNDER_ID, cli: "claude", look };
const c = { ...emptyCompany(), floors: [{ id: "gcare", dir: "D:\\gcare", name: "gcare" }], people: [...emptyCompany().people, person] };

describe("composePrompt", () => {
  it("puts the rank's laws, the role and the person's place in one prompt", () => {
    const text = composePrompt(person, role, c);
    expect(text).toContain(STAFF_LAWS);
    expect(text).toContain("You are the marketer.");
    expect(text).toContain("Vy");
    expect(text).toContain("Founder");
    expect(text).toContain("gcare");
  });
  it("picks the laws by rank", () => {
    expect(composePrompt({ ...person, rank: "lead" }, role, c)).toContain(LEAD_LAWS);
    expect(composePrompt({ ...person, rank: "founder", floor: null, boss: null }, role, c)).toContain(FOUNDER_LAWS);
  });
  it("does not tell a member to wait for the user's approval", () => {
    expect(STAFF_LAWS).not.toMatch(/wait for the user/i);
  });
});

describe("buildRoleLaunch", () => {
  it("passes the prompt file, the plugin and the identity", () => {
    const l = buildRoleLaunch({ person, role, promptFile: "C:\\p\\prompt.md", pluginDir: "C:\\p\\plugin" });
    expect(l.args).toEqual(["--append-system-prompt-file", "C:\\p\\prompt.md", "--plugin-dir", "C:\\p\\plugin"]);
    expect(l.env).toContainEqual(["BYTEHIVE_AGENT_ID", "p1"]);
  });
  it("adds denied tools only when the role has some, and skips an empty plugin", () => {
    const l = buildRoleLaunch({ person, role: { ...role, denyTools: ["Edit", "Write"] }, promptFile: "f", pluginDir: null });
    expect(l.args).toEqual(["--append-system-prompt-file", "f", "--disallowedTools", "Edit,Write"]);
  });
});

describe("pluginManifest", () => {
  it("is valid JSON with a safe name", () => {
    expect(JSON.parse(pluginManifest({ ...person, id: "p 1!" })).name).toBe("bytehive-role-p-1");
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `rtk npx vitest run src/company/launch.test.ts`
Expected: FAIL.

- [ ] **Step 3: Add the laws**

Append to `src/laws.ts`. These go through a file, so the cmd.exe rule above does not apply to them. Add a comment saying so.

```ts
// Company laws reach the agent through --append-system-prompt-file, so the
// one line rule above does not apply to them.

const COMPANY_COMMON =
  "You work at a company inside ByteHive, with a boss and maybe reports. Use fleet_send to talk: omit `to` to reach your boss, or name one of your reports. You cannot message anyone else; that is on purpose, pass it through your boss. Write agent messages in English. The person who owns the company outranks everyone: if they type into your terminal, do that first and say in one line what you paused. Read memory_get before you start. Keep the board true: card_move your card to Doing when you start and card_done with a one line summary when it is finished.";

export const FOUNDER_LAWS = `${COMPANY_COMMON}

You are the FOUNDER. You never edit files. A new job arrives as a line starting with JOB and an id. For each job: call company_status and role_list, then decide who does it. Prefer people who are already idle; hire only when nobody fits. Call job_plan once with a short summary, the cards per floor, the hires, and a tokenCap. The person approves or rejects it; do nothing else until you are told it was approved. If they ask to add someone, such as add marketing, that is a job with one hire and no cards. If no role fits, write one with role_propose and then call job_plan. After approval, talk only to Leads and to people working alone on a floor. When the Lead reports that a job is finished, check the cards with board_get and floor, then call job_finish with a summary. Never start work on a floor directly.`;

export const LEAD_LAWS = `${COMPANY_COMMON}

You are the LEAD of this floor. You never edit files; you split, assign, review and report. When a job reaches you, read the board, give each card to one of your people with fleet_send, one card each, one file area each so nobody writes the same files. Include everything they need in the message. When someone reports done, read their diff with git, send it back with exact reasons if it is wrong, and have QA check it if you have QA. Do not send a card back more than 3 times; tell your boss it is stuck instead. When every card of the job is done, report to your boss in a few lines.`;

export const STAFF_LAWS = `${COMPANY_COMMON}

You are STAFF. Your boss gives you cards, and you do not need anyone's approval to start one. Do only your card, in your own branch, in the file area you were given. Test your work before you call it done. When finished, call card_done and tell your boss what changed and how you checked it. You cannot hire or start other agents. If you are blocked, tell your boss right away instead of waiting.`;
```

- [ ] **Step 4: Implement launch**

```ts
// src/company/launch.ts
/* What a member of the company is launched with. Claude only for now; Codex
 * and OpenCode have different flags and come later. */
import { FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS } from "../laws";
import { personById } from "./org";
import type { Company, Person, RoleDef } from "./types";

const LAWS = { founder: FOUNDER_LAWS, lead: LEAD_LAWS, staff: STAFF_LAWS };

export function composePrompt(person: Person, role: RoleDef, c: Company): string {
  const boss = person.boss ? personById(c, person.boss)?.name : null;
  const floor = c.floors.find((f) => f.id === person.floor)?.name;
  const place = [
    `Your name is ${person.name}.`,
    boss ? `Your boss is ${boss}.` : "You answer only to the person who owns the company.",
    floor ? `You work on the ${floor} floor.` : "",
  ].filter(Boolean).join(" ");
  return `${LAWS[person.rank]}\n\n${place}\n\n${role.prompt}`;
}

export function buildRoleLaunch(o: { person: Person; role: RoleDef; promptFile: string; pluginDir: string | null }): {
  args: string[];
  env: Array<[string, string]>;
} {
  const args = ["--append-system-prompt-file", o.promptFile];
  if (o.pluginDir) args.push("--plugin-dir", o.pluginDir);
  if (o.role.denyTools.length) args.push("--disallowedTools", o.role.denyTools.join(","));
  return { args, env: [["BYTEHIVE_AGENT_ID", o.person.id]] };
}

export function pluginManifest(person: Person): string {
  const safe = person.id.replace(/[^a-z0-9]+/gi, "-").replace(/^-+|-+$/g, "").toLowerCase();
  return JSON.stringify({ name: `bytehive-role-${safe}`, version: "1.0.0" }, null, 2);
}
```

If Task 0.1 failed for a flag, apply its decision rule here and change the test to match.

- [ ] **Step 5: Stop the inline laws for members, keep worktrees**

In `src/pane.ts`, replace lines 337-341:

```ts
      const laws = spec.role === "conductor" ? DIRECTOR_LAWS : BYTEHIVE_LAWS;
      const own = spec.badge === "claude" ? await claudeSessionArgs(spec, cwd) : [];
      if (own.length) saveSession(); // History finds this run's transcript after a restart too
      // A member of the company brings its laws in its own args (company/launch.ts).
      const args =
        spec.badge !== "claude" ? spec.args
        : spec.personId ? [...spec.args, ...own]
        : [...spec.args, ...own, "--append-system-prompt", laws];
```

Change line 315 so a member keeps its worktree when the global pref is off:

```ts
        if (!getPref("worktree") && !spec.personId) {
```

In `src/spawnmodal.ts:248`:

```ts
export async function prepareIsolation(ws: Workspace, force = false): Promise<void> {
  if (!force && !getPref("worktree")) { ws.isolated = false; return; }
```

- [ ] **Step 6: Run the tests**

Run: `rtk npx vitest run src/company src/laws`
Expected: PASS. `src/laws.test.ts` enforces the one line rule on the old laws only; if it scans every export, limit it to `BYTEHIVE_LAWS` and `DIRECTOR_LAWS`.

- [ ] **Step 7: Commit**

```bash
rtk git add src/company src/laws.ts src/laws.test.ts src/pane.ts src/spawnmodal.ts
rtk git commit -m "feat: laws by rank and role launch"
```

### Task 5: MCP tools for the company

**Files:**
- Create: `mcp/src/company.ts`
- Modify: `mcp/src/server.ts:21,100-114,245-268,374-393`, `mcp/src/board.ts:62-79`, `src/board.ts:19-28,69-78`
- Test: `mcp/src/company.test.ts`

**Interfaces:**
- Consumes: `resolveTargets`, `personById`, `validatePlan`, types (imported as `../../src/company/org` and `../../src/company/types`, no `.js` suffix, as `mcp/src/context/stores.ts:13` does).
- Produces: env `BYTEHIVE_AGENT_ID`, file `~/.bytehive/company/requests.jsonl`.
- Produces: `companyRoot(home)`, `readCompany(root)`, `readRoles(root)`, `queueRequest(root, req)`, `sendAsMember(root, fromId, to, message, now)`, `planAsFounder(root, fromId, job, plan, now)`, `statusFor(root, fromId)`, `floorDir(root, fromId, floor)`, `registerCompanyTools(server, personId)`.
- Produces: `Card.job?: string`, `Card.assigneeId?: string`, kept in both `normalizeCard` functions.

- [ ] **Step 1: Write the failing test**

```ts
// mcp/src/company.test.ts
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { floorDir, planAsFounder, queueRequest, readCompany, sendAsMember, statusFor } from "./company.js";

let root: string;
const look = { shirt: "#fff", hair: "#000", prop: "none" };
const people = [
  { id: "founder", name: "Founder", role: "founder", rank: "founder", floor: null, boss: null, cli: "claude", look },
  { id: "l1", name: "Hai", role: "lead", rank: "lead", floor: "a", boss: "founder", cli: "claude", look },
  { id: "d1", name: "Linh", role: "developer", rank: "staff", floor: "a", boss: "l1", cli: "claude", look },
];
const limits = { peoplePerJob: 5, peoplePerLead: 5, peopleTotal: 20, tokensPerJob: 5000000, stallMinutes: 10, maxReturns: 3 };

beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "bytehive-company-"));
  fs.writeFileSync(path.join(root, "company.json"), JSON.stringify({ version: 1, floors: [{ id: "a", dir: "D:\\a", name: "a" }], people, limits, nextJob: 1 }));
  const r = path.join(root, "roles", "developer");
  fs.mkdirSync(r, { recursive: true });
  fs.writeFileSync(path.join(r, "role.json"), JSON.stringify({ id: "developer", title: "Developer", department: "Eng", rank: "staff", cli: "claude", denyTools: [], look, skills: [] }));
  fs.writeFileSync(path.join(r, "prompt.md"), "p");
});
afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

const lines = () => fs.readFileSync(path.join(root, "requests.jsonl"), "utf8").trim().split("\n").map((l) => JSON.parse(l));

describe("company MCP", () => {
  it("reads the roster", () => {
    expect(readCompany(root)?.people.length).toBe(3);
  });
  it("queues a message to the boss when no target is given", () => {
    expect(sendAsMember(root, "d1", undefined, "done", 5)).toBe("sent to Hai");
    expect(lines()[0]).toMatchObject({ type: "message", from: "d1", to: "Hai", message: "done", ts: 5 });
  });
  it("refuses a skip level message and queues nothing", () => {
    expect(() => sendAsMember(root, "d1", "Founder", "hi", 5)).toThrow(/Hai/);
    expect(fs.existsSync(path.join(root, "requests.jsonl"))).toBe(false);
  });
  it("only lets the Founder plan, and validates the plan", () => {
    const plan = { summary: "s", cards: [], hires: [{ role: "developer", floor: "a" }] };
    expect(() => planAsFounder(root, "l1", "J-0001", plan, 5)).toThrow(/Founder/);
    expect(() => planAsFounder(root, "founder", "J-0001", { ...plan, hires: [{ role: "ghost", floor: "a" }] }, 5)).toThrow(/ghost/);
    planAsFounder(root, "founder", "J-0001", plan, 5);
    expect(lines()[0]).toMatchObject({ type: "plan", job: "J-0001" });
  });
  it("lets the Founder read any floor and others only their own", () => {
    expect(floorDir(root, "founder", "a")).toBe("D:\\a");
    expect(() => floorDir(root, "d1", "a")).toThrow(/Founder/);
  });
  it("shows the Founder the floors and people", () => {
    expect(statusFor(root, "founder")).toMatchObject({ floors: [{ id: "a" }], people: expect.any(Array) });
  });
  it("appends rather than overwrites", () => {
    queueRequest(root, { type: "job-done", ts: 1, from: "founder", job: "J-1", summary: "x" });
    queueRequest(root, { type: "job-done", ts: 2, from: "founder", job: "J-2", summary: "y" });
    expect(lines().length).toBe(2);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run from `mcp/`: `rtk npx vitest run src/company.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
// mcp/src/company.ts
/* The company seen from an agent. This side only reads company.json and queues
 * requests; the app owns the file and checks everything again on delivery. */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { personById, resolveTargets, validatePlan } from "../../src/company/org";
import type { Company, CompanyRequest, JobPlan, RoleDef } from "../../src/company/types";
import { appendLine } from "./fleet.js";

export const companyRoot = (home = os.homedir()) => path.join(home, ".bytehive", "company");

export function readCompany(root: string): Company | null {
  try {
    return JSON.parse(fs.readFileSync(path.join(root, "company.json"), "utf8")) as Company;
  } catch {
    return null;
  }
}

export function readRoles(root: string): RoleDef[] {
  const dir = path.join(root, "roles");
  if (!fs.existsSync(dir)) return [];
  const out: RoleDef[] = [];
  for (const id of fs.readdirSync(dir)) {
    try {
      const def = JSON.parse(fs.readFileSync(path.join(dir, id, "role.json"), "utf8")) as Omit<RoleDef, "prompt">;
      out.push({ ...def, prompt: fs.readFileSync(path.join(dir, id, "prompt.md"), "utf8") });
    } catch { /* an incomplete role is not offered */ }
  }
  return out;
}

export function queueRequest(root: string, req: CompanyRequest): void {
  fs.mkdirSync(root, { recursive: true });
  appendLine(path.join(root, "requests.jsonl"), JSON.stringify(req));
}

const company = (root: string): Company => {
  const c = readCompany(root);
  if (!c) throw new Error("The company has not been set up in ByteHive yet.");
  return c;
};

function founderOnly(c: Company, id: string): void {
  if (personById(c, id)?.rank !== "founder") throw new Error("Only the Founder can do this. Ask your boss.");
}

export function sendAsMember(root: string, from: string, to: string | undefined, message: string, now: number): string {
  const text = message.trim();
  if (!text) throw new Error("message must not be empty");
  const r = resolveTargets(company(root), from, to?.trim() || null);
  if ("error" in r) throw new Error(r.error);
  for (const p of r.people) queueRequest(root, { type: "message", ts: now, from, to: p.name, message: text });
  return `sent to ${r.people.map((p) => p.name).join(", ")}`;
}

export function planAsFounder(root: string, from: string, job: string, plan: JobPlan, now: number): void {
  const c = company(root);
  founderOnly(c, from);
  const errors = validatePlan(c, plan, readRoles(root));
  if (errors.length) throw new Error(errors.join(" "));
  queueRequest(root, { type: "plan", ts: now, from, job, plan });
}

export function statusFor(root: string, id: string): unknown {
  const c = company(root);
  const me = personById(c, id);
  const people = c.people
    .filter((p) => me?.rank === "founder" || p.floor === me?.floor || p.id === me?.boss)
    .map(({ id: pid, name, role, rank, floor, boss }) => ({ id: pid, name, role, rank, floor, boss }));
  return { floors: me?.rank === "founder" ? c.floors : c.floors.filter((f) => f.id === me?.floor), people, limits: c.limits };
}

export function floorDir(root: string, id: string, floor: string): string {
  const c = company(root);
  founderOnly(c, id);
  const f = c.floors.find((x) => x.id === floor);
  if (!f) throw new Error(`Unknown floor ${floor}. See company_status.`);
  return f.dir;
}

const planSchema = {
  job: z.string().describe("The job id, like J-0142"),
  summary: z.string().describe("What will be done, in two lines, for the owner to read"),
  cards: z.array(z.object({
    floor: z.string(),
    title: z.string(),
    desc: z.string(),
    checklist: z.array(z.string()),
    assignee: z.string().optional().describe("Name of an existing person, or omit for the Lead to choose"),
  })),
  hires: z.array(z.object({ role: z.string(), floor: z.string() })),
  tokenCap: z.number().int().positive().optional(),
};

type Reply = { content: { type: "text"; text: string }[]; isError?: boolean };
const reply = (fn: () => unknown): Reply => {
  try {
    const out = fn();
    return { content: [{ type: "text", text: typeof out === "string" ? out : JSON.stringify(out, null, 2) }] };
  } catch (e) {
    return { content: [{ type: "text", text: e instanceof Error ? e.message : String(e) }], isError: true };
  }
};

/** Tools only members of the company get. */
export function registerCompanyTools(server: McpServer, personId: string, root = companyRoot()): void {
  server.registerTool("company_status", {
    description: "The floors and people you can see in the company, and its limits.",
    inputSchema: {},
  }, async () => reply(() => statusFor(root, personId)));

  server.registerTool("role_list", {
    description: "Roles the Founder can hire.",
    inputSchema: {},
  }, async () => reply(() => readRoles(root).map(({ id, title, department, skills }) => ({ id, title, department, skills }))));

  server.registerTool("job_plan", {
    description: "Founder only: send a plan for the owner to approve. Hires happen only after approval.",
    inputSchema: planSchema,
  }, async ({ job, summary, cards, hires, tokenCap }) =>
    reply(() => { planAsFounder(root, personId, job, { summary, cards, hires, tokenCap }, Date.now()); return "plan sent, wait for the answer"; }));

  server.registerTool("role_propose", {
    description: "Founder only: draft a new role when none fits. The owner sees it with the plan.",
    inputSchema: {
      job: z.string(),
      id: z.string().describe("Short lowercase id, like data-analyst"),
      title: z.string(),
      department: z.string(),
      prompt: z.string().describe("What this person does and how, at least three sentences"),
    },
  }, async ({ job, id, title, department, prompt }) => reply(() => {
    founderOnly(company(root), personId);
    const role: RoleDef = { id, title, department, rank: "staff", cli: "claude", denyTools: [], look: { shirt: "#7d8b99", hair: "#2a2e3a", prop: "none" }, prompt, skills: [] };
    queueRequest(root, { type: "role", ts: Date.now(), from: personId, role, job });
    return "role sent, it appears in role_list once saved";
  }));

  server.registerTool("job_finish", {
    description: "Founder only: report a job finished so the owner can review and merge it.",
    inputSchema: { job: z.string(), summary: z.string() },
  }, async ({ job, summary }) => reply(() => {
    founderOnly(company(root), personId);
    queueRequest(root, { type: "job-done", ts: Date.now(), from: personId, job, summary });
    return "reported to the owner";
  }));
}
```

- [ ] **Step 4: Wire the server**

In `mcp/src/server.ts`:

Add the import after line 21:

```ts
import { companyRoot, floorDir, registerCompanyTools, sendAsMember } from "./company.js";
```

After line 86 add:

```ts
  // Set only for members of the company; everyone else keeps the flat fleet.
  const personId = process.env.BYTEHIVE_AGENT_ID?.trim() || undefined;
```

Replace the `board_get` registration (lines 100-114):

```ts
  server.registerTool(
    "board_get",
    {
      description:
        "Read the ByteHive kanban board for this workspace (.bytehive/board.json): all lists and their cards, including ids to use with the other tools. The Founder can pass floor to read another project.",
      inputSchema: { floor: z.string().optional().describe("Founder only: a floor id from company_status") },
    },
    async ({ floor }) => {
      try {
        if (floor && !personId) return fail("Only members of the company have floors.");
        return ok(loadBoard(floor ? floorDir(companyRoot(), personId!, floor) : dir));
      } catch (e) {
        return fail(e);
      }
    },
  );
```

In the `fleet_send` handler, first line inside `try`:

```ts
        if (personId) return ok(sendAsMember(companyRoot(), personId, to, message, Date.now()));
```

In the `agent_spawn` handler, first line inside `try`:

```ts
        if (personId) return fail("Members of the company do not start agents. Hiring goes through the Founder's plan.");
```

Before `registerBrowserTools` add:

```ts
  if (personId) registerCompanyTools(server, personId);
```

- [ ] **Step 5: Keep job fields on cards**

In `src/board.ts` and `mcp/src/board.ts`, add to `Card` after `assignee`:

```ts
  job?: string; // the company Job this card belongs to
  assigneeId?: string; // the Person working it; survives a rename
```

In each `normalizeCard`, after the `assignee` line:

```ts
  if (typeof c.job === "string" && c.job) card.job = c.job;
  if (typeof c.assigneeId === "string" && c.assigneeId) card.assigneeId = c.assigneeId;
```

- [ ] **Step 6: Run the tests and build**

Run from `mcp/`: `rtk npx vitest run && rtk npm run build`
Expected: PASS, and the bundle builds with `src/company/org` inlined.

Run from the root: `rtk npx vitest run src/board`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add mcp/src src/board.ts
rtk git commit -m "feat: MCP tools for company members"
```

### Task 6: Engine, plans and hiring

**Files:**
- Create: `src/company/reduce.ts`, `src/company/engine.ts`
- Modify: `src/spawnmodal.ts` (add `spawnPerson`), `src/bridges.ts:261`, `src/kanban.ts:391-392`
- Test: `src/company/reduce.test.ts`

**Interfaces:**
- Consumes: everything above; `drainJsonl` (`src/fleetbridge.ts:235`), `sendMessage` (`src/ipc.ts:82`), `deliverTask`, `createWorkspace`, `readBoardFile`, `writeBoardFile`.
- Produces, `reduce.ts`: `State = { company: Company; jobs: Job[]; roles: RoleDef[] }`, `Effect`, `newJob(s, ask, now): { state, job, effects }`, `applyRequest(s, req): { state, effects }`, `approvePlan(s, jobId, pick): { state, hired, effects }`, `rejectPlan(s, jobId, note): { state, effects }`.
- Produces, `engine.ts`: `initCompany(): Promise<void>`, `giveJob(ask: string)`, `approve(jobId)`, `reject(jobId, note)`, `companyView(): { company, jobs, alerts }`, `onCompanyChange(fn)`, `isCompanyFloor(dir): boolean`, `openPerson(id)`.
- Produces, `spawnmodal.ts`: `spawnPerson(dir: string, spec: AgentSpec, boot: boolean): Promise<void>`.

```ts
type Effect =
  | { kind: "deliver"; personId: string; text: string }
  | { kind: "save-role"; role: RoleDef };
```

- [ ] **Step 1: Write the failing test**

```ts
// src/company/reduce.test.ts
import { describe, it, expect } from "vitest";
import { applyRequest, approvePlan, newJob, rejectPlan, type State } from "./reduce";
import { emptyCompany } from "./org";
import { FOUNDER_ID, type JobPlan, type Person, type RoleDef } from "./types";

const look = { shirt: "#fff", hair: "#000", prop: "none" };
const role: RoleDef = { id: "marketing", title: "M", department: "M", rank: "staff", cli: "claude", denyTools: [], look, prompt: "p", skills: [] };
const lead: Person = { id: "l1", name: "Hai", role: "lead", rank: "lead", floor: "a", boss: FOUNDER_ID, cli: "claude", look };
const dev: Person = { id: "d1", name: "Linh", role: "developer", rank: "staff", floor: "a", boss: "l1", cli: "claude", look };
const state = (): State => ({
  company: { ...emptyCompany(), floors: [{ id: "a", dir: "D:\\a", name: "a" }, { id: "b", dir: "D:\\b", name: "b" }], people: [...emptyCompany().people, lead, dev] },
  jobs: [],
  roles: [role],
});
const plan: JobPlan = { summary: "Add marketing", cards: [], hires: [{ role: "marketing", floor: "b" }] };
const pick = { name: () => "Vy", id: () => "p9" };

describe("newJob", () => {
  it("numbers jobs and wakes the Founder", () => {
    const { state: s, job, effects } = newJob(state(), "add marketing to b", 100);
    expect(job.id).toBe("J-0001");
    expect(s.company.nextJob).toBe(2);
    expect(job.status).toBe("planning");
    expect(job.tokenCap).toBe(5_000_000);
    expect(effects).toEqual([{ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("JOB J-0001") }]);
  });
});

describe("applyRequest", () => {
  it("delivers a message between boss and report with who sent it", () => {
    const { effects } = applyRequest(state(), { type: "message", ts: 1, from: "d1", to: "Hai", message: "done" });
    expect(effects).toEqual([{ kind: "deliver", personId: "l1", text: "[From Linh] done" }]);
  });
  it("drops a forged message that breaks the hierarchy", () => {
    expect(applyRequest(state(), { type: "message", ts: 1, from: "d1", to: "Founder", message: "hi" }).effects).toEqual([]);
  });
  it("puts a valid plan up for approval", () => {
    const s = newJob(state(), "x", 1).state;
    const r = applyRequest(s, { type: "plan", ts: 2, from: FOUNDER_ID, job: "J-0001", plan });
    expect(r.state.jobs[0].status).toBe("awaiting-plan");
    expect(r.state.jobs[0].plan).toEqual(plan);
  });
  it("refuses a plan from anyone but the Founder", () => {
    const s = newJob(state(), "x", 1).state;
    expect(applyRequest(s, { type: "plan", ts: 2, from: "l1", job: "J-0001", plan }).state.jobs[0].status).toBe("planning");
  });
  it("tells the Founder when a plan fails on arrival", () => {
    const s = newJob(state(), "x", 1).state;
    const bad = { ...plan, hires: [{ role: "ghost", floor: "b" }] };
    const r = applyRequest(s, { type: "plan", ts: 2, from: FOUNDER_ID, job: "J-0001", plan: bad });
    expect(r.state.jobs[0].status).toBe("planning");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("ghost") });
  });
  it("marks a finished job for merge", () => {
    const s = newJob(state(), "x", 1).state;
    expect(applyRequest(s, { type: "job-done", ts: 2, from: FOUNDER_ID, job: "J-0001", summary: "ok" }).state.jobs[0]).toMatchObject({ status: "awaiting-merge", summary: "ok" });
  });
  it("saves a proposed role and adds it to the list", () => {
    const r = applyRequest(state(), { type: "role", ts: 1, from: FOUNDER_ID, role: { ...role, id: "analyst" }, job: "J-0001" });
    expect(r.state.roles.map((x) => x.id)).toContain("analyst");
    expect(r.effects).toEqual([{ kind: "save-role", role: expect.objectContaining({ id: "analyst" }) }]);
  });
});

describe("approvePlan and rejectPlan", () => {
  const waiting = () => applyRequest(newJob(state(), "x", 1).state, { type: "plan", ts: 2, from: FOUNDER_ID, job: "J-0001", plan }).state;
  it("hires, starts the job and tells the Founder", () => {
    const r = approvePlan(waiting(), "J-0001", pick);
    expect(r.hired.map((p) => p.name)).toEqual(["Vy"]);
    expect(r.state.jobs[0].status).toBe("running");
    expect(r.state.jobs[0].people).toContain("p9");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("approved") });
  });
  it("uses the plan's token cap when it has one", () => {
    const s = waiting();
    s.jobs[0].plan = { ...plan, tokenCap: 900 };
    expect(approvePlan(s, "J-0001", pick).state.jobs[0].tokenCap).toBe(900);
  });
  it("sends a rejection with its reason back to the Founder and replans", () => {
    const r = rejectPlan(waiting(), "J-0001", "too many people");
    expect(r.state.jobs[0].status).toBe("planning");
    expect(r.effects[0]).toMatchObject({ kind: "deliver", personId: FOUNDER_ID, text: expect.stringContaining("too many people") });
  });
  it("closes the job when rejected with no reason", () => {
    expect(rejectPlan(waiting(), "J-0001", "").state.jobs[0].status).toBe("rejected");
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `rtk npx vitest run src/company/reduce.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement the reducer**

```ts
// src/company/reduce.ts
/* Everything that changes the company, as plain functions: state in, state and
 * effects out. engine.ts runs the effects. The app re-checks every rule here,
 * because a request is only a line in a file. */
import { applyHires, canMessage, personById, personByName, validatePlan } from "./org";
import { FOUNDER_ID, type Company, type CompanyRequest, type Job, type Person, type RoleDef } from "./types";

export interface State { company: Company; jobs: Job[]; roles: RoleDef[] }

export type Effect =
  | { kind: "deliver"; personId: string; text: string }
  | { kind: "save-role"; role: RoleDef };

const jobId = (n: number) => `J-${String(n).padStart(4, "0")}`;
const patch = (s: State, id: string, change: Partial<Job>): State => ({
  ...s,
  jobs: s.jobs.map((j) => (j.id === id ? { ...j, ...change } : j)),
});

export function newJob(s: State, ask: string, now: number): { state: State; job: Job; effects: Effect[] } {
  const job: Job = {
    id: jobId(s.company.nextJob), title: ask.trim().split("\n")[0].slice(0, 80), ask, status: "planning", plan: null,
    tokenCap: s.company.limits.tokensPerJob, tokensUsed: 0, tokenBase: {}, people: [], createdAt: now,
  };
  const state = { ...s, company: { ...s.company, nextJob: s.company.nextJob + 1 }, jobs: [...s.jobs, job] };
  return { state, job, effects: [{ kind: "deliver", personId: FOUNDER_ID, text: `JOB ${job.id}: ${ask}` }] };
}

export function applyRequest(s: State, r: CompanyRequest): { state: State; effects: Effect[] } {
  const none = { state: s, effects: [] as Effect[] };
  const from = personById(s.company, r.from);
  if (!from) return none;
  if (r.type === "message") {
    const to = r.to ? personByName(s.company, r.to) : undefined;
    if (!to || !canMessage(s.company, from.id, to.id)) return none;
    return { state: s, effects: [{ kind: "deliver", personId: to.id, text: `[From ${from.name}] ${r.message}` }] };
  }
  if (from.rank !== "founder") return none;
  if (r.type === "role") {
    return { state: { ...s, roles: [...s.roles.filter((x) => x.id !== r.role.id), r.role] }, effects: [{ kind: "save-role", role: r.role }] };
  }
  if (!s.jobs.some((j) => j.id === r.job)) return none;
  if (r.type === "plan") {
    const errors = validatePlan(s.company, r.plan, s.roles);
    if (errors.length) return { state: s, effects: [{ kind: "deliver", personId: FOUNDER_ID, text: `Plan for ${r.job} refused: ${errors.join(" ")}` }] };
    return { state: patch(s, r.job, { status: "awaiting-plan", plan: r.plan }), effects: [] };
  }
  return { state: patch(s, r.job, { status: "awaiting-merge", summary: r.summary }), effects: [] };
}

export function approvePlan(
  s: State,
  id: string,
  pick: { name: (taken: string[]) => string; id: () => string },
): { state: State; hired: Person[]; effects: Effect[] } {
  const job = s.jobs.find((j) => j.id === id);
  if (!job?.plan || job.status !== "awaiting-plan") return { state: s, hired: [], effects: [] };
  const { company, hired } = applyHires(s.company, job.plan, s.roles, pick);
  const names = hired.map((p) => `${p.name} (${p.role})`).join(", ");
  const state = patch({ ...s, company }, id, {
    status: "running",
    tokenCap: job.plan.tokenCap ?? job.tokenCap,
    people: [...new Set([...job.people, ...hired.map((p) => p.id)])],
  });
  const text = `Plan for ${id} approved.${names ? ` Hired: ${names}.` : ""} The cards are on the boards. Brief the Lead of each floor, or the person working alone there.`;
  return { state, hired, effects: [{ kind: "deliver", personId: FOUNDER_ID, text }] };
}

export function rejectPlan(s: State, id: string, note: string): { state: State; effects: Effect[] } {
  const why = note.trim();
  if (!why) return { state: patch(s, id, { status: "rejected", plan: null }), effects: [] };
  return {
    state: patch(s, id, { status: "planning", plan: null }),
    effects: [{ kind: "deliver", personId: FOUNDER_ID, text: `Plan for ${id} sent back: ${why} Make a new plan with job_plan.` }],
  };
}
```

- [ ] **Step 4: Add spawnPerson**

In `src/spawnmodal.ts`, after `spawnForConductor`:

```ts
/** Start a member of the company in its floor's project. A floor always gets
 *  worktrees when it is a git repo: two people must not share a checkout. */
export async function spawnPerson(dir: string, spec: AgentSpec, boot: boolean): Promise<void> {
  const ws = onCreateWorkspace(dir);
  await prepareIsolation(ws, true);
  const start = onCreateAgent(ws, spec);
  if (boot) await runLimited([start], MAX_CONCURRENT_BOOT);
}
```

Add `AgentSpec` to the `./panetypes` import if it is missing. `onCreateWorkspace` is `createWorkspace`, which returns the open workspace for a folder already open (`src/workspace.ts:73`). If its return type is not `Workspace`, mirror line 120. If `prepareIsolation` does nothing because the folder is not a repo, the person runs in the folder, which is the right result for the Founder's `office` folder.

- [ ] **Step 5: Implement the engine**

```ts
// src/company/engine.ts
/* Runs the company: reads requests, applies the rules, starts people, types
 * messages into their terminals. All decisions live in reduce.ts and watch.ts. */
import { homeDir } from "@tauri-apps/api/path";
import { fsCreateDir, sendMessage } from "../ipc";
import { workspaces, newId } from "../appstate";
import { drainJsonl } from "../fleetbridge";
import { cliLook, CLI_PRESETS, nameForNewPane, effectiveArgs } from "../crew";
import { deliverTask, spawnPerson } from "../spawnmodal";
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
import { FOUNDER_ID, type CompanyRequest, type Job, type Person, type RoleDef } from "./types";

const OFFICE_REL = "office";
const REQUESTS_REL = "requests.jsonl";
const TICK_MS = 1500;

let root = "";
let state: State | null = null;
const consumed = new Map<string, number>();
const listeners = new Set<() => void>();

export const onCompanyChange = (fn: () => void) => { listeners.add(fn); };
const changed = () => listeners.forEach((fn) => fn());

export function companyView() { return state; }

export function isCompanyFloor(dir: string | null): boolean {
  return !!dir && !!state?.company.floors.some((f) => sameFolder(f.dir, dir));
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
    ...cliLook(preset.badge, preset.label),
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

/** Type into a person's terminal, waking them first if they are stopped. */
async function deliver(personId: string, text: string): Promise<void> {
  const p = state && personById(state.company, personId);
  if (!p) return;
  await ensurePerson(p);
  const found = paneOf(personId);
  if (!found) return;
  if (found.pane.running) {
    await sendMessage(found.pane.id, text).catch(() => {});
    return;
  }
  await found.pane.restart?.();
  deliverTask(found.ws, [p.name], text, 8000);
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
    const r = applyRequest(state, req);
    state = r.state;
    await persist(state.jobs);
    await run(r.effects);
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

export async function approve(id: string): Promise<void> {
  if (!state) return;
  const r = approvePlan(state, id, {
    name: (taken) => nameForNewPane("agent", taken),
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
```

If a name imported here is exported from a different file (`cliLook`, `CLI_PRESETS`, `sameFolder`), fix the import path; `grep` shows the owner. `cliLook` may live in `main.ts` and reach `spawnmodal.ts` through `configureSpawnModal`: in that case add `export const cliLookOf = onCliLook` in `src/spawnmodal.ts` and use it here.

`initCompany` sets `effectiveArgs(preset, true)`, so members run with the CLI's skip permissions flag. They run unattended and the plan approval is the gate.

- [ ] **Step 6: Start it, and stop the Conductor on floors**

In `src/bridges.ts` after the `initFleetBridge({...})` call:

```ts
  void initCompany().catch(() => {});
```

with `import { initCompany } from "./company/engine";` at the top.

In `src/kanban.ts:392`:

```ts
    if (conductorMode === "off" || !ctx || isCompanyFloor(ctx.dir)) return;
```

with `import { isCompanyFloor } from "./company/engine";`. Check `DockContext` for the folder field name in `src/dockstore.ts` and use it. A Lead dispatches on these floors, and auto mode would approve cards behind your back.

Add a test to `src/conductor.test.ts` only if `isCompanyFloor` is moved into `planConductor`. It stays in `kanban.ts`, so cover it in Task 9's end to end run.

- [ ] **Step 7: Run tests and type check**

Run: `rtk npx vitest run src/company src/fleetbridge src/spawnmodal && rtk npx tsc --noEmit`
Expected: PASS, no type errors.

- [ ] **Step 8: Commit**

```bash
rtk git add src/company src/spawnmodal.ts src/bridges.ts src/kanban.ts
rtk git commit -m "feat: company engine, plans and hiring"
```

### Task 7: Stall and token limits

`src/agentcost.ts:58` counts `cache_read`. A long Claude session reads its cache on every turn, so that sum reaches millions quickly and a 5,000,000 cap would trip on normal work. The cap counts fresh tokens only.

**Files:**
- Create: `src/company/watch.ts`
- Modify: `src/agentcost.ts:16-22,52-65`, `src/company/engine.ts`, `src/company/reduce.ts`
- Test: `src/company/watch.test.ts`

**Interfaces:**
- Consumes: `Job`, `Limits`, `Effect`; `agentUsage(pane)`.
- Produces: `Usage.fresh: number | null`.
- Produces: `Alert = { key: string; job: string | null; personId: string | null; text: string; level: "warn" | "stop" }`.
- Produces: `watchWork(i: { cards: WatchedCard[]; nudges: Map<string, number>; limits: Limits; bossOf: (id: string) => string | null; now: number }): { alerts: Alert[]; effects: Effect[]; nudges: Map<string, number> }`.
- Produces: `watchBudget(job: Job): { alert: Alert | null; pause: boolean }`.

```ts
export interface WatchedCard {
  id: string;
  title: string;
  job: string;
  assigneeId: string;
  quietSince: number | null; // when the person went idle, null while working
  returns: number;
}
```

- [ ] **Step 1: Write the failing test**

```ts
// src/company/watch.test.ts
import { describe, it, expect } from "vitest";
import { watchBudget, watchWork, type WatchedCard } from "./watch";
import { DEFAULT_LIMITS, type Job } from "./types";

const MIN = 60_000;
const card = (over: Partial<WatchedCard>): WatchedCard => ({ id: "c1", title: "Login", job: "J-1", assigneeId: "d1", quietSince: null, returns: 0, ...over });
const run = (cards: WatchedCard[], nudges = new Map<string, number>(), now = 100 * MIN) =>
  watchWork({ cards, nudges, limits: DEFAULT_LIMITS, bossOf: () => "l1", now });

describe("watchWork", () => {
  it("leaves a working person alone", () => {
    expect(run([card({})]).effects).toEqual([]);
  });
  it("leaves someone quiet for under 10 minutes alone", () => {
    expect(run([card({ quietSince: 91 * MIN })]).effects).toEqual([]);
  });
  it("nudges the boss the first time", () => {
    const r = run([card({ quietSince: 80 * MIN })]);
    expect(r.effects).toEqual([{ kind: "deliver", personId: "l1", text: expect.stringContaining("Login") }]);
    expect(r.alerts).toEqual([]);
    expect(r.nudges.get("c1")).toBe(1);
  });
  it("does not nudge again in the same quiet stretch until another 10 minutes pass", () => {
    const first = run([card({ quietSince: 85 * MIN })]);
    expect(run([card({ quietSince: 85 * MIN })], first.nudges, 101 * MIN).effects).toEqual([]);
  });
  it("asks you the second time", () => {
    const r = run([card({ quietSince: 70 * MIN })], new Map([["c1", 1]]));
    expect(r.alerts[0]).toMatchObject({ job: "J-1", personId: "d1", level: "warn" });
  });
  it("forgets the nudges of a card that started moving", () => {
    expect(run([card({})], new Map([["c1", 1]])).nudges.has("c1")).toBe(false);
  });
  it("stops a card returned 3 times", () => {
    const r = run([card({ returns: 3 })]);
    expect(r.alerts[0].text).toContain("3");
  });
});

const job = (used: number, cap = 1000): Job => ({ id: "J-1", title: "t", ask: "a", status: "running", plan: null, tokenCap: cap, tokensUsed: used, tokenBase: {}, people: [], createdAt: 1 });

describe("watchBudget", () => {
  it("is quiet under 80%", () => {
    expect(watchBudget(job(799))).toEqual({ alert: null, pause: false });
  });
  it("warns at 80%", () => {
    expect(watchBudget(job(800))).toMatchObject({ alert: { level: "warn" }, pause: false });
  });
  it("pauses at 100%", () => {
    expect(watchBudget(job(1000))).toMatchObject({ alert: { level: "stop" }, pause: true });
  });
  it("ignores a job that is not running", () => {
    expect(watchBudget({ ...job(5000), status: "paused" }).pause).toBe(false);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `rtk npx vitest run src/company/watch.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
// src/company/watch.ts
/* Agents do not report back on their own, and managers loop. These two checks
 * are what stops a Job from quietly stalling or burning tokens. */
import type { Effect } from "./reduce";
import type { Job, Limits } from "./types";

export interface Alert {
  key: string;
  job: string | null;
  personId: string | null;
  text: string;
  level: "warn" | "stop";
}

export interface WatchedCard {
  id: string;
  title: string;
  job: string;
  assigneeId: string;
  quietSince: number | null;
  returns: number;
}

export function watchWork(i: {
  cards: WatchedCard[];
  nudges: Map<string, number>;
  limits: Limits;
  bossOf: (id: string) => string | null;
  now: number;
}): { alerts: Alert[]; effects: Effect[]; nudges: Map<string, number> } {
  const alerts: Alert[] = [];
  const effects: Effect[] = [];
  const nudges = new Map<string, number>();
  const span = i.limits.stallMinutes * 60_000;
  for (const c of i.cards) {
    if (c.returns >= i.limits.maxReturns) {
      alerts.push({ key: `returns-${c.id}`, job: c.job, personId: c.assigneeId, level: "warn", text: `"${c.title}" went back ${c.returns} times and needs you.` });
    }
    if (c.quietSince === null) continue;
    const quiet = i.now - c.quietSince;
    const had = i.nudges.get(c.id) ?? 0;
    nudges.set(c.id, had);
    if (quiet < span * (had + 1)) continue;
    const boss = i.bossOf(c.assigneeId);
    if (had === 0 && boss) {
      nudges.set(c.id, 1);
      effects.push({ kind: "deliver", personId: boss, text: `"${c.title}" has gone quiet for ${i.limits.stallMinutes} minutes. Check on it.` });
    } else {
      alerts.push({ key: `stall-${c.id}`, job: c.job, personId: c.assigneeId, level: "warn", text: `"${c.title}" is stuck.` });
    }
  }
  return { alerts, effects, nudges };
}

export function watchBudget(job: Job): { alert: Alert | null; pause: boolean } {
  if (job.status !== "running") return { alert: null, pause: false };
  const share = job.tokensUsed / job.tokenCap;
  if (share >= 1) {
    return { pause: true, alert: { key: `cap-${job.id}`, job: job.id, personId: null, level: "stop", text: `${job.id} reached its token cap and is paused.` } };
  }
  if (share >= 0.8) {
    return { pause: false, alert: { key: `cap-${job.id}`, job: job.id, personId: null, level: "warn", text: `${job.id} has used ${Math.round(share * 100)}% of its tokens.` } };
  }
  return { alert: null, pause: false };
}
```

Add a fresh count in `src/agentcost.ts`. In `Usage`:

```ts
  /** Tokens without cached re-reads: what a budget should count. */
  fresh: number | null;
```

In `read`, start with `const u: Usage = { usd: null, tokens: null, fresh: null, contextPct: null };`, and in the Claude branch add:

```ts
    u.fresh = rows.reduce((n, r) => n + r.input_tokens + r.output_tokens + r.cache_creation, 0) || null;
```

In the other branch add `u.fresh = u.tokens;`. Fix any `Usage` literal the compiler flags, including tests.

In `src/company/reduce.ts`, add:

```ts
export function pauseJob(s: State, id: string): State { return patch(s, id, { status: "paused" }); }
export function resumeJob(s: State, id: string, extra: number): State {
  const job = s.jobs.find((j) => j.id === id);
  return job ? patch(s, id, { status: "running", tokenCap: job.tokenCap + extra }) : s;
}
```

Also stop delivery for a paused Job's people: in `applyRequest`, for `message`, add before the return:

```ts
    if (s.jobs.some((j) => j.status === "paused" && j.people.includes(from.id))) return none;
```

with a test in `reduce.test.ts`:

```ts
it("holds messages from people on a paused job", () => {
  const s = newJob(state(), "x", 1).state;
  s.jobs[0] = { ...s.jobs[0], status: "paused", people: ["d1"] };
  expect(applyRequest(s, { type: "message", ts: 1, from: "d1", to: "Hai", message: "done" }).effects).toEqual([]);
});
```

Add to `src/company/engine.ts`:

```ts
import { agentUsage } from "../agentcost";
import { pauseJob, resumeJob } from "./reduce";
import { watchBudget, watchWork, type Alert, type WatchedCard } from "./watch";

const WATCH_MS = 30_000;
let alerts: Alert[] = [];
let nudges = new Map<string, number>();
const quiet = new Map<string, number>();

/** Fresh tokens of everyone on a Job, since they joined it. */
function measure(job: Job): Job {
  const base = { ...job.tokenBase };
  let used = 0;
  for (const id of job.people) {
    const found = paneOf(id);
    const fresh = found ? agentUsage(found.pane)?.fresh ?? 0 : 0;
    base[id] ??= fresh;
    used += fresh - base[id];
  }
  return { ...job, tokenBase: base, tokensUsed: Math.max(job.tokensUsed, used) };
}

async function doingCards(): Promise<WatchedCard[]> {
  const out: WatchedCard[] = [];
  for (const floor of state?.company.floors ?? []) {
    const file = await readBoardFile(floor.dir).catch(() => null);
    const doing = file?.board.lists.find((l) => l.title.trim().toLowerCase() === "doing");
    for (const c of doing?.cards ?? []) {
      if (!c.job || !c.assigneeId) continue;
      const pane = paneOf(c.assigneeId)?.pane;
      const working = !!pane?.running && Date.now() - pane.lastOutputAt < 60_000;
      if (working) quiet.delete(c.id);
      else if (!quiet.has(c.id)) quiet.set(c.id, Date.now());
      out.push({ id: c.id, title: c.title, job: c.job, assigneeId: c.assigneeId, quietSince: quiet.get(c.id) ?? null, returns: c.returns ?? 0 });
    }
  }
  return out;
}

async function watch(): Promise<void> {
  if (!state) return;
  const work = watchWork({
    cards: await doingCards(),
    nudges,
    limits: state.company.limits,
    bossOf: (id) => (state ? personById(state.company, id)?.boss ?? null : null),
    now: Date.now(),
  });
  nudges = work.nudges;
  const next: Alert[] = [...work.alerts];
  const jobs = state.jobs.map(measure);
  state = { ...state, jobs };
  for (const job of jobs) {
    const b = watchBudget(job);
    if (b.alert) next.push(b.alert);
    if (b.pause) state = pauseJob(state, job.id);
  }
  alerts = next;
  await persist(state.jobs.filter((j) => j.status === "running" || j.status === "paused"));
  await run(work.effects);
}

export const companyAlerts = () => alerts;

export async function raiseCap(id: string, extra: number): Promise<void> {
  if (!state) return;
  state = resumeJob(state, id, extra);
  await persist(state.jobs.filter((j) => j.id === id));
}
```

In `initCompany`, after `next();`:

```ts
  window.setInterval(() => void watch().catch(() => {}), WATCH_MS);
```

`c.returns` needs one field: add `returns?: number; // times a Lead sent it back` to `Card` in `src/board.ts` and `mcp/src/board.ts`, keep it in both `normalizeCard` functions, and in `mcp/src/ops.ts` `moveCard` increment it when a card that has a `job` moves from a list titled Done or Review back to Doing or To do. Add an `mcp/src/ops.test.ts` case for that.

- [ ] **Step 4: Run tests and type check**

Run: `rtk npx vitest run src/company src/agentcost && rtk npx tsc --noEmit`
Run from `mcp/`: `rtk npx vitest run src/ops.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk git add src mcp/src
rtk git commit -m "feat: stall and token limits for company jobs"
```

### Task 8: Company panel

Temporary Phase 1 screen. Phase 2 replaces it with the office, and reuses `needs.ts` for the Needs you rail.

**Files:**
- Create: `src/company/needs.ts`, `src/company/panel.ts`, `src/company/panel.css`
- Modify: `src/dock.ts:12-34,111-115`, `src/main.ts:413-416`, `src/inboxhelp.ts:54,102`
- Test: `src/company/needs.test.ts`

**Interfaces:**
- Consumes: `companyView()`, `companyAlerts()`, `giveJob`, `approve`, `reject`, `raiseCap`, `openPerson`, `onCompanyChange`.
- Produces: `NeedRow = { key: string; kind: "plan" | "merge" | "paused" | "alert"; jobId: string | null; personId: string | null; title: string; detail: string }`.
- Produces: `needsYou(jobs: Job[], alerts: Alert[]): NeedRow[]`, `createCompanyPanel()`.

- [ ] **Step 1: Write the failing test**

```ts
// src/company/needs.test.ts
import { describe, it, expect } from "vitest";
import { needsYou } from "./needs";
import type { Job } from "./types";

const job = (over: Partial<Job>): Job => ({ id: "J-1", title: "Add login", ask: "a", status: "running", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 1, ...over });

describe("needsYou", () => {
  it("is empty while work just runs", () => {
    expect(needsYou([job({})], [])).toEqual([]);
  });
  it("shows a plan with what it will cost you in people", () => {
    const rows = needsYou([job({ status: "awaiting-plan", plan: { summary: "Build it", cards: [], hires: [{ role: "qa", floor: "a" }] } })], []);
    expect(rows[0]).toMatchObject({ kind: "plan", jobId: "J-1", title: "Approve plan: Add login" });
    expect(rows[0].detail).toContain("Build it");
    expect(rows[0].detail).toContain("1 hire");
  });
  it("shows merge and paused jobs", () => {
    const rows = needsYou([job({ status: "awaiting-merge", summary: "Done" }), job({ id: "J-2", status: "paused" })], []);
    expect(rows.map((r) => r.kind)).toEqual(["paused", "merge"]);
  });
  it("puts plans first, then stops, then the rest", () => {
    const rows = needsYou([job({ status: "awaiting-merge" }), job({ id: "J-2", status: "awaiting-plan", plan: { summary: "s", cards: [], hires: [] } })], [
      { key: "k", job: "J-1", personId: "d1", text: "stuck", level: "warn" },
    ]);
    expect(rows.map((r) => r.kind)).toEqual(["plan", "alert", "merge"]);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `rtk npx vitest run src/company/needs.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
// src/company/needs.ts
/* What the owner has to act on, in the order worth acting on it. */
import type { Alert } from "./watch";
import type { Job } from "./types";

export interface NeedRow {
  key: string;
  kind: "plan" | "merge" | "paused" | "alert";
  jobId: string | null;
  personId: string | null;
  title: string;
  detail: string;
}

const ORDER: Record<NeedRow["kind"], number> = { plan: 0, paused: 1, alert: 2, merge: 3 };
const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;

export function needsYou(jobs: Job[], alerts: Alert[]): NeedRow[] {
  const rows: NeedRow[] = [];
  for (const j of jobs) {
    if (j.status === "awaiting-plan" && j.plan) {
      rows.push({
        key: `plan-${j.id}`, kind: "plan", jobId: j.id, personId: null, title: `Approve plan: ${j.title}`,
        detail: `${j.plan.summary} ${plural(j.plan.cards.length, "card")}, ${plural(j.plan.hires.length, "hire")}.`,
      });
    }
    if (j.status === "paused") {
      rows.push({ key: `paused-${j.id}`, kind: "paused", jobId: j.id, personId: null, title: `Paused: ${j.title}`, detail: "It reached its token cap." });
    }
    if (j.status === "awaiting-merge") {
      rows.push({ key: `merge-${j.id}`, kind: "merge", jobId: j.id, personId: null, title: `Review and merge: ${j.title}`, detail: j.summary ?? "" });
    }
  }
  for (const a of alerts) {
    if (a.level === "stop") continue; // a paused Job already has its row
    rows.push({ key: a.key, kind: "alert", jobId: a.job, personId: a.personId, title: a.text, detail: "" });
  }
  return rows.sort((a, b) => ORDER[a.kind] - ORDER[b.kind]);
}
```

```ts
// src/company/panel.ts
/* Temporary home of the company until the 3D office replaces it: give the
 * Founder a job, act on what needs you, see who works where. */
import "./panel.css";
import { approve, companyAlerts, companyView, giveJob, onCompanyChange, openPerson, raiseCap, reject } from "./engine";
import { needsYou, type NeedRow } from "./needs";
import type { Person } from "./types";

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

function rowHtml(r: NeedRow): string {
  const job = r.jobId ? ` data-job="${esc(r.jobId)}"` : "";
  const buttons =
    r.kind === "plan" ? `<button data-act="approve"${job}>Approve</button><button data-act="reject"${job}>Send back</button>`
    : r.kind === "paused" ? `<button data-act="raise"${job}>Add 1M tokens</button>`
    : r.personId ? `<button data-act="open" data-person="${esc(r.personId)}">Open</button>`
    : "";
  return `<li class="co-row"><b>${esc(r.title)}</b><span>${esc(r.detail)}</span><div>${buttons}</div></li>`;
}

const personHtml = (p: Person) =>
  `<li><button class="co-person" data-act="open" data-person="${esc(p.id)}"><i style="background:${esc(p.look.shirt)}"></i>${esc(p.name)}<span>${esc(p.role)}</span></button></li>`;

export function createCompanyPanel() {
  let body: HTMLElement | null = null;

  const draw = () => {
    const s = companyView();
    if (!body || !s) return;
    const needs = needsYou(s.jobs, companyAlerts());
    const founder = s.company.people.filter((p) => p.rank === "founder");
    const floors = s.company.floors.map((f) => {
      const people = s.company.people.filter((p) => p.floor === f.id).sort((a, b) => a.rank.localeCompare(b.rank));
      return `<h4>${esc(f.name)}</h4><ul>${people.map(personHtml).join("") || `<li class="co-none">Nobody here yet</li>`}</ul>`;
    }).join("");
    const list = body.querySelector<HTMLElement>("[data-needs]");
    const org = body.querySelector<HTMLElement>("[data-org]");
    if (list) list.innerHTML = needs.map(rowHtml).join("") || `<li class="co-none">Nothing needs you.</li>`;
    if (org) org.innerHTML = `<h4>Top floor</h4><ul>${founder.map(personHtml).join("")}</ul>${floors}`;
  };

  return {
    mount(el: HTMLElement) {
      body = el;
      el.innerHTML =
        `<form class="co-give"><label for="coAsk">Give the Founder a job</label>` +
        `<textarea id="coAsk" rows="3" placeholder="Add marketing to gcare"></textarea>` +
        `<button type="submit">Send</button></form>` +
        `<h3>Needs you</h3><ul class="co-needs" data-needs></ul>` +
        `<h3>People</h3><div class="co-org" data-org></div>`;
      el.querySelector("form")?.addEventListener("submit", (e) => {
        e.preventDefault();
        const box = el.querySelector("textarea");
        if (!box?.value.trim()) return;
        void giveJob(box.value);
        box.value = "";
      });
      el.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLElement>("[data-act]");
        if (!b) return;
        const job = b.dataset.job ?? "";
        if (b.dataset.act === "approve") void approve(job);
        if (b.dataset.act === "reject") {
          const note = el.querySelector<HTMLTextAreaElement>("textarea")?.value ?? "";
          void reject(job, note);
        }
        if (b.dataset.act === "raise") void raiseCap(job, 1_000_000);
        if (b.dataset.act === "open") openPerson(b.dataset.person ?? "");
      });
      onCompanyChange(draw);
      draw();
    },
    show: draw,
  };
}
```

The Send back button takes its reason from the job box, so the box's placeholder must say so while a plan is waiting: in `draw`, set `textarea.placeholder` to `Why send it back? Leave empty to cancel the job.` when any row has kind `plan`, and to `Add marketing to gcare` otherwise.

```css
/* src/company/panel.css */
.dp-company { display: flex; flex-direction: column; gap: 14px; padding: 12px; overflow: auto; }
.dp-company h3, .dp-company h4 { margin: 0; font-weight: 600; }
.dp-company h3 { font-size: 13px; }
.dp-company h4 { font-size: 12px; opacity: .7; margin-top: 8px; }
.dp-company ul { list-style: none; margin: 0; padding: 0; }
.co-give { display: flex; flex-direction: column; gap: 6px; }
.co-give textarea { resize: vertical; font: inherit; }
.co-give button { align-self: flex-end; }
.co-row { display: flex; flex-direction: column; gap: 4px; padding: 8px 0; border-top: 1px solid rgb(255 255 255 / .08); }
.co-row span { opacity: .7; font-size: 12px; }
.co-row div { display: flex; gap: 6px; }
.co-person { display: flex; align-items: center; gap: 8px; width: 100%; padding: 4px; background: none; border: 0; color: inherit; font: inherit; text-align: left; cursor: pointer; }
.co-person:focus-visible { outline: 2px solid #f5b83d; }
.co-person i { width: 10px; height: 10px; border-radius: 50%; }
.co-person span { margin-left: auto; opacity: .6; font-size: 12px; }
.co-none { opacity: .6; font-size: 12px; padding: 4px 0; }
```

Match existing button classes from the Fleet panel if it has them.

- [ ] **Step 4: Add it to the dock**

In `src/dock.ts`:

```ts
import { createCompanyPanel } from "./company/panel";
export type ToolId = "kanban" | "pomodoro" | "diff" | "fleet" | "flow" | "company";
```

Add `company: "Company",` to `TITLES`, `"company"` to `TOOL_IDS`, and `tools.company = createCompanyPanel();` after `tools.flow`. `mount(body, actions)` accepts the shorter `mount(el)`.

In `src/main.ts`, after the `"m"` branch:

```ts
  } else if (k === "o") {
    e.preventDefault();
    dockToggle("company");
  }
```

In `src/inboxhelp.ts`, add before line 54:

```ts
  { id: "company", name: "Company", what: "Give the Founder a job and see what needs you.", keys: ["Ctrl", "Shift", "O"] },
```

and add `"company"` to the array on line 102.

Check that Ctrl+Shift+O is not already bound: `rtk grep -n '"o"' src/main.ts`. Pick another letter if it is.

- [ ] **Step 5: Run tests and type check**

Run: `rtk npx vitest run src/company src/dock src/inboxhelp && rtk npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/company src/dock.ts src/main.ts src/inboxhelp.ts
rtk git commit -m "feat: company panel"
```

### Task 9: End to end, then release

This task uses real Claude Code agents and spends real quota.

**Files:**
- Modify: `CHANGELOG.md`, `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `README.md`

- [ ] **Step 1: Full test run**

Run: `rtk npm test` and, from `mcp/`, `rtk npm test && rtk npm run build`
Expected: everything passes.

- [ ] **Step 2: Run the app**

Run: `rtk npm run tauri:dev`. Confirm the window is ByteHive Dev, not the installed app. Open a scratch git repo as a project, not a real one.

- [ ] **Step 3: Scenario A, add marketing**

Press Ctrl+Shift+O. Send `Add marketing to <scratch>`.

Expected, in order: Founder pane starts in `~/.bytehive/company/office`. A plan row appears with 1 hire. Approve it. A new pane starts on that floor, in its own worktree. In that pane, ask `What is your job, and what skills do you have?`. It answers as a marketer and names `campaign-brief`. `~/.bytehive/company/company.json` lists the person with `boss: "founder"`.

- [ ] **Step 4: Scenario B, a job across two floors**

Open a second scratch repo. Send `Add a VERSION file with 1.0.0 to both projects`.

Expected: one plan with cards on both floors. After approval the cards are in To do on both boards with `job` set. Work finishes, the Founder calls `job_finish`, and a Review and merge row appears. You approved twice in total.

- [ ] **Step 5: Scenario C, rules hold**

In a staff pane type `Use fleet_send to message the Founder directly.`
Expected: the tool returns an error naming the person's boss. Then `Use agent_spawn to start another agent.` Expected: refused.

- [ ] **Step 6: Scenario D, limits**

Set `tokensPerJob` to 2000 in `company.json` while the app is closed, restart, run a job. Expected: a warn row, then a paused row. Messages stop. Add 1M tokens and the Job runs again. Set the value back to 5000000.

Then leave a card in Doing with its agent stopped for over 10 minutes with `stallMinutes` set to 1. Expected: the boss is nudged, then a row appears. Restore the value.

- [ ] **Step 7: Check old behavior**

Start a plain agent from New agent in a project with no company people. Expected: it behaves as before, `fleet_send` broadcast still works, Conductor still runs on a folder that is not a floor. Note that every open project becomes a floor once a job is given, so test this before Scenario A or in a fresh `~/.bytehive/company`.

If this shows the Conductor stopped on projects the user never used the company on, change `isCompanyFloor` to require at least one person on the floor, and add that to `engine.ts`:

```ts
export function isCompanyFloor(dir: string | null): boolean {
  if (!dir || !state) return false;
  const floor = state.company.floors.find((f) => sameFolder(f.dir, dir));
  return !!floor && state.company.people.some((p) => p.floor === floor.id);
}
```

- [ ] **Step 8: Docs and release**

Add a CHANGELOG section for 0.14.0 written for users. Mention the worktree board fix. Add a short Company section to `README.md`. Bump the version to 0.14.0 in `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`.

```bash
rtk git add CHANGELOG.md README.md package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
rtk git commit -m "chore: version 0.14.0"
```

Merge `feat/company` into `revert/maestro-v0.10.10`, push, tag `v0.14.0`, push the tag, and watch the Windows job until it is green. Check `latest.json` on `tdat-dev/maestro-releases` shows 0.14.0.

---

## What this plan leaves out on purpose

- The 3D office, character editing, Codex and OpenCode roles, and removing old mechanisms. Those are Phases 2 to 4, each planned after this ships.
- A reused person keeps the branch from when they were hired. Branches named per Job only apply to people hired for that Job. Making a reused person start a new branch on each Job needs a new worktree per Job, and belongs with Phase 3.
- `requests.jsonl` lines written while the app is closed are skipped on the next start, as `drainJsonl` does for the outbox. Founder and Lead are stopped then too, so only the last request of a session can be lost.
