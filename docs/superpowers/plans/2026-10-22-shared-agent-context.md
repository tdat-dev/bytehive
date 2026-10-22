# Shared Agent Context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Any agent with the maestro MCP reads what Claude Code, Codex and OpenCode agents did in the project. In the app, a stopped agent's task can go to an open agent, and an agent out of usage hands over by itself.

**Architecture:** The MCP server (`mcp/`) reads every CLI's conversation store directly and parses it with the app's own pure parsers. It is bundled by esbuild, so it can import `../src`. A shared pure module `src/convdigest.ts` turns a parsed chat into a digest; `handoff.ts` and the MCP both use it. The app publishes `cli/session/folder/since` per agent in `fleet.json`, so the MCP can name conversations. Hand over and auto takeover stay in `limitguard.ts`.

**Tech Stack:** TypeScript, vitest, @modelcontextprotocol/sdk, zod, esbuild, `node:sqlite` (Node >= 22.13), Tauri app (vanilla TS).

## Global Constraints

- **No em dash** (U+2014) anywhere: code, comments, copy, commits.
- **Shell commands** go through `rtk`.
- **Files:** CRLF working files (`core.autocrlf=true`). Edit with Edit/Write; never reflow unrelated lines.
- **`MAESTRO_LAWS`:** one line with none of `& | < > % ! ^ ( ) " '`. `laws.test.ts` enforces it.
- **fleet_send / laws / tool text:** English.
- **Window:** conversations are looked for in the last 14 days, 20 at most, newest first.
- **`autoHandover` pref:** on by default. It fires only on a usage-limit episode, never on Stop or exit.
- **Commits:** one per task, Conventional Commits, ending with `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`.

---

### Task 1: `src/convdigest.ts`, with handoff.ts on top of it

**Files:**
- Create: `src/convdigest.ts`, `src/convdigest.test.ts`
- Modify: `src/handoff.ts` (use the module; output unchanged)

**Interfaces (produces):**
```ts
export interface Digest { asks: string[]; todos: Todo[]; files: FileChange[]; lastWords: string }
export function digestOf(chat: Chat | null, firstAsks?: string[]): Digest
export function asksSection(asks: string[], intro: string): string   // "" when none
export function planSection(todos: Todo[], intro: string): string    // "" when none
export function filesSection(files: FileChange[], intro: string): string
export function lastWordsSection(text: string, intro: string): string
export function planProgress(todos: Todo[]): string                  // "3/7 done", "" when none
```
`digestOf` moves the ask merging out of `handoffBrief`: the later asks come from the chat's user items, and the first asks are kept when the later ones don't already include them. Clipping constants move with it (ASK_CHARS 700, LAST_WORDS_CHARS 1500, MAX_ASKS 6, MAX_FILES 30).

- [ ] **Step 1:** Write `convdigest.test.ts`:
  - `digestOf` merges first asks without duplicates;
  - `planProgress([done, doing, todo])` returns `"1/3 done"`;
  - `asksSection` of 10 asks shows the first and the latest 5, with "(the first, then the latest 5 of 10)".
- [ ] **Step 2:** Run `rtk npx vitest run src/convdigest.test.ts`. Expect FAIL (no module).
- [ ] **Step 3:** Implement `convdigest.ts`, then rewrite `handoffBrief` as header + `asksSection(d.asks, "What it was asked, in order")` + `planSection(d.todos, "Its plan when it stopped:")` + `filesSection(d.files, "Files it changed")` + `lastWordsSection(d.lastWords, "Its last words:")` + closing line.
- [ ] **Step 4:** Run `rtk npx vitest run src/convdigest.test.ts src/handoff.test.ts`. Expect PASS, with handoff output byte-identical.
- [ ] **Step 5:** Commit `refactor(handoff): one digest of a conversation, shared`.

### Task 2: bundle maestro-mcp with esbuild

**Files:**
- Modify: `mcp/package.json` (build script `esbuild src/index.ts --bundle --platform=node --format=esm --target=node22 --packages=external --outfile=dist/index.js --banner:js="#!/usr/bin/env node"`; `typecheck: tsc --noEmit`; devDependency esbuild; engines `>=22.13`)
- Modify: `mcp/tsconfig.json`: `rootDir: ".."`, `include: ["src", "../src/chatmodel.ts", "../src/codexmodel.ts", "../src/opencodemodel.ts", "../src/convdigest.ts"]`, `allowImportingTsExtensions` + `noEmit`

**Steps:**
- [ ] `cd mcp && rtk npm i -D esbuild@0.25.10`
- [ ] Run `rtk npm run build`, then `rtk npm test`. The existing tests stay green, and `dist/index.js` is one file.
- [ ] Smoke test: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | node dist/index.js D:/maestro` lists the tools. A response line appears after initialize; use the existing smoke approach from the README if needed.
- [ ] Commit `build(mcp): bundle with esbuild so it can use the app's parsers`.

### Task 3: MCP conversation stores

**Files:**
- Create: `mcp/src/context/stores.ts`, `mcp/src/context/stores.test.ts`

**Interfaces (produces):**
```ts
export type Cli = "claude" | "codex" | "opencode";
export interface Conversation {
  cli: Cli; id: string; folder: string; updated: number; started: number;
  running: boolean | null;               // null: this store can't tell
  path: string;                          // claude/codex: file; opencode: session id
}
export interface Env { home: string; now: number; days?: number; limit?: number }
export function projectFolders(dir: string): string[]          // dir + git worktrees, deduped by sameDir
export function sameDir(a: string, b: string): boolean
export function claudeSlug(dir: string): string                 // non [A-Za-z0-9] -> "-"
export async function findConversations(folders: string[], env: Env): Promise<{ list: Conversation[]; notes: string[] }>
export async function readConversation(c: Conversation, env: Env): Promise<{ chat: Chat; firstAsks: string[] }>
```

**Rules:**
- **Claude:**
  - files: `home/.claude/projects/<claudeSlug(folder)>/*.jsonl` (top level only);
  - `id`: the file name without `.jsonl`;
  - `updated`: mtime; `started`: birthtime, falling back to mtime;
  - `running`: true when some `home/.claude/sessions/*.json` has this sessionId and `process.kill(pid, 0)` does not throw, else false.
- **Codex:**
  - where: day folders `home/.codex/sessions/YYYY/MM/DD` within `days`;
  - files: `rollout-*.jsonl` whose first line's `cwd` matches by `sameDir`;
  - `id`: the uuid at the end of the name;
  - `started`: the time in the name, else birthtime.
- **OpenCode:**
  - `const { DatabaseSync } = await import("node:sqlite")` in a try. If that fails, add the note "OpenCode conversations need Node 22.13 or newer";
  - db: `home/.local/share/opencode/opencode.db`, opened `{ readOnly: true }`;
  - query: `SELECT id, directory, time_created, time_updated FROM session WHERE parent_id IS NULL AND time_updated >= ?`, filtered by `sameDir`;
  - `running`: null.
- **Read:**
  - Claude/Codex: the first 256 KB (whole lines) feed a head chat, whose first 2 user items are `firstAsks`. The last 4 MB, cut at a line start, feed the chat. Parse with `createChat` / `createCodexChat`.
  - OpenCode: message+part rows as JSONL, the same shape as `rows_after` in `src-tauri/src/transcripts/opencode.rs` (`{kind,id,messageId,created,updated,data}`), ordered by time_updated. Feed them to `createOpencodeChat`.

**Tests:** a temp HOME holding:
- two Claude sessions in `D--app` (a live pid = `process.pid`, plus a dead pid 999999);
- a Codex rollout in D:/app and one in E:/other;
- an OpenCode db with the tables from `opencode.rs` tests.

Assert:
- the order is newest first;
- the folder filter holds;
- `running` is set;
- `readConversation` yields user text and todos;
- a session older than `days` is skipped.

- [ ] Write the tests, then run them. They should fail.
- [ ] Implement, then run again. They should pass.
- [ ] Commit `feat(mcp): find every CLI's conversations in a project`.

### Task 4: `agent_context` tool

**Files:**
- Create: `mcp/src/context/agentcontext.ts`, `mcp/src/context/agentcontext.test.ts`
- Modify: `mcp/src/fleet.ts` (FleetAgent gains `cli`, `session`, `folder`, `since`, all optional), `mcp/src/server.ts` (register the tool, add a paragraph to INSTRUCTIONS)

**Interfaces (produces):**
```ts
export function nameOf(c: Conversation, fleet: FleetAgent[], all: Conversation[]): string | null
export function resolveWho(who: string, convs: Conversation[], fleet: FleetAgent[]): Conversation | null
export function listText(convs: Conversation[], digests: Map<Conversation, Digest>, fleet: FleetAgent[], now: number, notes: string[]): string
export function detailText(c: Conversation, d: Digest, name: string | null, now: number): string
export async function agentContext(dir: string, who: string | undefined, env: Env): Promise<string>
```

**`nameOf`:**
- A fleet agent with `session` names the conversation when `session` equals its `id` or its `path`.
- Otherwise a fleet agent of the same `cli` with `folder` same-dir names it, when it is the newest conversation of that cli/folder with `updated >= since`.

**`resolveWho`, in order:**
1. a fleet name (case-insensitive), through `nameOf`;
2. an id prefix of at least 4 characters;
3. a CLI word (`claude`, `codex`, `opencode`, `claude code`), giving the newest of that cli.

**List line:**
```
- Ana [3f2a91c0] Claude Code, running, active 2 min ago. Task: <first ask clipped 120>. Plan 3/7 done. Folder: <folder>
```
- Each conversation shows its first 8 id characters.
- Folder is shown only when it differs from `dir`.

**Detail:** the header line, then:
- `asksSection(d.asks, "What it was asked, in order")`;
- `planSection(d.todos, "Its plan:")`;
- `filesSection(d.files, "Files it changed")`;
- `lastWordsSection(d.lastWords, "Its last words:")`;
- the carry-on line when todos are open.

**Tool description** (verbatim): "What other coding agents (Claude Code, Codex, OpenCode) did in this project, including ones that stopped. Call it with no arguments when you start working here, and with who to read one agent's full context before you continue or take over its work." The input is `who` (optional): an agent name, a conversation id, or a CLI name.

**Instructions paragraph:** "Other agents may have worked here before you, in Maestro or not, with any CLI. Call agent_context first to see who did what, and agent_context with who to pick up an unfinished task instead of starting over."

- [ ] Write the tests (listing names via fleet session, via folder+since; resolve order; detail text has plan and carry-on line). They should fail.
- [ ] Implement, register the tool in server.ts with `env = { home: os.homedir(), now: Date.now() }`. The tests should pass, and `rtk npm run build` should succeed.
- [ ] Commit `feat(mcp): agent_context reads what every agent in the project did`.

### Task 5: the app publishes cli/session/folder/since

**Files:**
- Modify: `src/fleetbridge.ts` (`BridgeAgent` + `serializeFleet` add `cli`, `session`, `folder`, `since`)
- Modify: `src/bridges.ts:269-289`:
  - `cli: p.spec.badge`;
  - `session: p.spec.sessionId ?? knownConversation(p.id) ?? ""`;
  - `folder: p.spec.ranIn ?? ""`;
  - `since: p.spawnedAt ?? 0`.
- Modify: `src/chatsource.ts` (`export function knownConversation(paneId: string): string | null` returning `known.get(paneId)?.path ?? null`)
- Test: `src/fleet.test.ts` or `fleetbridge.test.ts`: serializeFleet carries the new fields.

- [ ] Write the test, see it fail, implement, see it pass.
- [ ] Commit `feat(fleet): say each agent's CLI and conversation in fleet.json`.

### Task 6: law sentence

**Files:** `src/laws.ts`, `src/laws.test.ts`

- [ ] Add a test: `MAESTRO_LAWS` contains `agent_context`.
- [ ] Append to MAESTRO_LAWS: ` Before you start a task, call agent_context to see what the other agents in this project did, and never redo work they finished.`
- [ ] Run `rtk npx vitest run src/laws.test.ts`. Expect PASS.
- [ ] Commit `feat(laws): agents look at agent_context before they start`.

### Task 7: hand over to an open agent

**Files:**
- Modify: `src/limitguard.ts`
- Modify: `src/panetypes.ts` (`handedTo?: string` on Pane)
- Modify: `src/inbox.ts` (menu, stage button, `handOverTo`)
- Test: `src/limitguard.test.ts`

**Interfaces (produces):**
```ts
export interface OpenTarget { paneId: string; name: string; cli: string }
export function openTargets(pane: Pane, now?: number): OpenTarget[]
export function takeOverLine(from: { name: string; cli: string }, why: string): string
export async function handOverToAgent(fromId: string, toId: string, why: string): Promise<string | null>
```

**`openTargets`:** other panes in the same workspace that pass all of these:
- running;
- `spec.badge !== pane.spec.badge`, and not a shell (`profile`/preset shell);
- `spec.role !== "conductor"`;
- `sameDir(spec.ranIn, pane.spec.ranIn)`;
- task state `idle` or `review`.

Order: CLIs whose plan has room first.

**`takeOverLine`:** "Take over the unfinished task of Ana (Claude Code), which stopped because <why>. Call the maestro agent_context tool with who set to Ana to read its task, plan, changed files and last words, check git status, then finish what is left. Say in one line what you picked up first."

**`handOverToAgent`:**
1. `sendMessage(toId, line)`;
2. set `from.handedTo = to.spec.name`;
3. set `from.spec.title = "Handed to <to>: <old title>"`.

`checkAgents` skips the carry-on when `pane.handedTo` is set. Restart clears `handedTo` (in pane restart path: `pane.handedTo = undefined`).

**Menu** (`agentMenu`):
- open targets first, as "Hand over to Bo (OpenCode)";
- then presets, as "Hand over to a new Codex agent";
- 5 items at most.

**Stage:** a stopped agent with any target gets `<button class="ib-act" data-stage="handover">Hand over</button>` after Resume. Clicking it calls `openMenu(x, y, items)` with the same items.

**Tests:**
- `openTargets` keeps the idle same-folder OpenCode agent;
- it skips a busy one, one in another folder and the conductor;
- `handOverToAgent` sends the line and marks `handedTo`;
- carry-on is not sent when `handedTo` is set.

- [ ] Write the tests, see them fail, implement, see them pass, then run `rtk npx vitest run src/inbox.test.ts` too.
- [ ] Commit `feat(handoff): hand a task to an agent that is already open`.

### Task 8: auto takeover on a usage limit

**Files:**
- Modify: `src/prefs.ts` (`autoHandover: boolean`, default true)
- Modify: `src/prefsview.ts` (`{ id: "prefAutoHandover", key: "autoHandover" }`)
- Modify: `index.html` (toggle before prefAutoResume): "**Hand over when an agent runs out of usage**: Another CLI whose plan has room takes the task over at once, an agent already open in the same folder first, else a new one. Off waits for the limit to reset."
- Modify: `src/limitguard.ts` (`checkAgents`)
- Test: `src/limitguard.test.ts`, `src/prefs.test.ts`

**Logic** (on a new episode only, `e.handed` flag on Episode):
```ts
if (getPref("autoHandover") && !e.handed) {
  e.handed = true;
  const why = "it hit its plan's usage limit";
  const open = openTargets(found.pane, now).filter((o) => roomFor(o.cli, now))[0];
  const name = open ? await handOverToAgent(t.paneId, open.paneId, why)
    : (() => { const p = handoffTargets(found.pane, now).find((x) => roomFor(badgeOfPreset(x.id), now)); return p ? handOff(t.paneId, p.id, why) : null; })();
  if (name) { found.pane.handedTo = name; notify(`${t.name} ran out of usage`, `${name} took over its task.`); }
}
```
Here `roomFor(badge) = planResetFor(planOf(badge), now) === null`. `checkAgents` stays sync: the handover runs as a fire-and-forget promise and `e.handed` guards re-entry.

**Tests:**
- a new episode hands over once to the open idle agent;
- with no open agent it spawns a preset with room;
- with no room it falls back to carry-on at reset;
- with the pref off it never hands over;
- prefs default `autoHandover` true.

- [ ] Write the tests, see them fail, implement, see them pass.
- [ ] Commit `feat(limits): an agent out of usage hands its task over by itself`.

### Task 9: docs, real check, release

**Files:** `mcp/README.md` (agent_context row; Codex and OpenCode install lines), `CHANGELOG.md`, version files via `scratchpad/bump.js` (FROM 0.9.13, TO 0.10.0)

**Steps:**
- [ ] Run the full suites: `rtk npx vitest run`, `cd mcp && rtk npm test && rtk npm run build`, `rtk npx tsc --noEmit`.
- [ ] Real check: `node mcp/dist/index.js D:/maestro` through a JSON-RPC script that calls `agent_context`. The list should show this session (Claude) plus any Codex/OpenCode conversations in D:/maestro. Then call it with who set to an OpenCode id.
- [ ] README and CHANGELOG; bump; commit; push `feat/agent-inbox` and `feat/agent-inbox:master`; tag v0.10.0; watch the Release workflow; check latest.json.
