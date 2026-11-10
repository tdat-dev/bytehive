# Company in the native app: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The native ByteHive app gets the Company back: give the Founder a job, approve one plan, hired people work in their own worktrees and terminals, Needs you lists what waits for the owner, the owner merges and marks done. The 3D office comes in a later plan.

**Architecture:** A Rust port of the Tauri Company (oracle: `D:\bytehive-worktrees\office\src\company\*.ts` and their `*.test.ts`, branch feat/office-3d at 38837e7) in `native/src/company/`. Pure modules (types, store, org, reduce, needs, watch, lock, roles, launch) are ported file by file with their tests. One `engine.rs` drives them through a `Host` trait, implemented over the native `Fleet` and `Terminal`, so the engine is tested with a fake host. The bundled Node MCP server (`mcp/`) is reused unchanged: same files on disk under `~/.bytehive/company`, same `requests.jsonl`. The UI is a new surface `native/src/inbox/company/` (Needs you rail, job box, Jobs, floors as 2D lists); a person's desk is the existing stage with their seat focused.

**Tech Stack:** Rust 2024, gpui-kit =0.9.0, serde/serde_json, the existing `agent_core` crates, Node 22.13+ for the MCP server, NSIS installer.

## Global Constraints

- Worktree `D:\bytehive-worktrees\native-company`, branch `feat/native-company` off `feat/native-gpui`. Build and test from `native/`: `rtk cargo test`.
- Same on-disk format as the Tauri app (types.ts) so the MCP server and the owner's existing Company data work unchanged. Never write `~/.bytehive/company` in a test or by hand: tests use a temp dir; dev runs set `BYTEHIVE_COMPANY_ROOT` to a sandbox.
- Behaviour comes from the oracle: when unsure, read the TS function and its tests and do the same. Every ported module carries the oracle's test cases (same names in snake_case).
- Files under 400 lines, split past 500; one feature per file; strings in `inbox/words.rs`, look in `inbox/style.rs` (native/CLAUDE.md). Port look from the Tauri CSS (`src/company/office/*.css`, `src/styles/`) with exact values.
- A focused terminal is pure passthrough: a new app key needs a `terminal/keys.rs` entry and must not steal keys from a focused terminal except as today's Ctrl+K.
- Never `cargo fmt`. Never the U+2014 long dash. Commits conventional, small, NO Co-Authored-By line. Files are CRLF where they are: never `sed -i`.
- Never kill a process you did not start (the owner's ByteHive runs). No Claude in Chrome tools.
- Owner limits: app under 1 GB RAM, idle CPU under 2%, no frame over 100 ms, typing never slowed.

---

### Task 1: Types, store and lock

**Files:**
- Create: `native/src/company/mod.rs`, `native/src/company/types.rs`, `native/src/company/store.rs`, `native/src/company/lock.rs`, `native/src/company/store_tests.rs`
- Modify: `native/src/main.rs` (add `mod company;`)
- Oracle: `src/company/types.ts`, `store.ts` + `store.test.ts`, `lock.ts` + `lock.test.ts`

**Interfaces:**
- Produces: serde types `Company, Floor, Person, Rank, Look, Limits (Default = DEFAULT_LIMITS), RoleDef, Hire, PlanCard, NewRole, JobPlan, JobStatus, Job, CompanyRequest` with `#[serde(rename_all = "camelCase")]` and enum tags matching the TS (`"awaiting-plan"`, `"job-done"`); `FOUNDER_ID`.
- `store::root() -> Option<PathBuf>`: `BYTEHIVE_COMPANY_ROOT` if set, else `%USERPROFILE%\.bytehive\company`.
- `store::load_company(root) -> anyhow::Result<Company>` (empty company with the Founder when missing, as `loadCompany`), `save_company(root, &Company)` (write `company.json.tmp` then rename), `load_jobs(root) -> Vec<Job>` plus unreadable file names, `save_job(root, &Job)`.
- `lock::lock_move(...)` as `lockMove` in lock.ts (take, keep, refuse, stale by `{pid, at}`), `lock::read/write`.

- [ ] Step 1: Copy two real files as fixtures: a `company.json` and a `J-0002.json` from the Task 12 sandbox (`%USERPROFILE%\.bytehive\company-dev\`, read only) into `native/src/company/fixtures/`. Write `store_tests.rs`: both fixtures parse and serialize back to equal `serde_json::Value`s (no field lost, unknown fields kept via `#[serde(flatten)] extra: serde_json::Map` on Company, Person, Job).
- [ ] Step 2: Port each `store.test.ts` and `lock.test.ts` case to Rust tests against a `tempfile::tempdir()` root. Run `rtk cargo test company::` and see them fail to compile.
- [ ] Step 3: Write the types, store and lock. Run until green.
- [ ] Step 4: Commit `feat: the Company's files in the native app, same format as before`.

### Task 2: Org rules, alerts

**Files:**
- Create: `native/src/company/org.rs`, `native/src/company/org_tests.rs`, `native/src/company/alerts.rs`
- Oracle: `org.ts` + `org.test.ts`, `alerts.ts`

**Interfaces:**
- Consumes: Task 1 types.
- Produces: `plan_cap(&JobPlan, limit) -> u64`, `can_message(&Company, from, to) -> bool`, `resolve_targets(&Company, from, to: Option<&str>) -> Result<Vec<String>, String>`, `reuse_staff(&Company, &JobPlan, roles) -> JobPlan`, `validate_plan(&Company, &JobPlan, roles, limits) -> Result<(), String>` (same error words as the TS: the Founder reads them), `apply_hires(&Company, &JobPlan, roles, names) -> (Company, Vec<String>)`, `Alert { key, job, person_id, text, level, standing }`.

- [ ] Step 1: Port every `org.test.ts` case. Run, see them fail.
- [ ] Step 2: Port the functions. Run until green.
- [ ] Step 3: Commit `feat: the Company's org rules in the native app`.

### Task 3: The reducer

**Files:**
- Create: `native/src/company/reduce.rs`, `native/src/company/reduce_tests.rs`
- Oracle: `reduce.ts` + `reduce.test.ts`

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces: `State { company: Company, jobs: Vec<Job> }`, `Effect` enum with the same variants as the TS `Effect` union (deliver, hire, start, interrupt, board writes, alert, save), and `apply_request`, `approve_plan`, `reject_plan`, `raise_cap`, `pause_job`, `halt_job` (Stop), `resume_job`, `finish_job`, `new_job` with the TS signatures turned into `fn(&State, ...) -> Reduced { state, effects }`.

- [ ] Step 1: Port every `reduce.test.ts` case. Run, see them fail.
- [ ] Step 2: Port the reducer. Run until green.
- [ ] Step 3: Commit `feat: the Company's reducer in the native app`.

### Task 4: Needs you and the watchers

**Files:**
- Create: `native/src/company/needs.rs`, `native/src/company/watch.rs`, `native/src/company/needs_tests.rs`
- Oracle: `needs.ts` + `needs.test.ts` (with the merge row that walks every person, 3506371), `watch.ts` + `watch.test.ts`

**Interfaces:**
- Produces: `NeedRow { key, kind: NeedKind (Plan, Merge, Paused, Alert), job_id, person_id, title, detail, standing, plan: Option<PlanDetail> }`, `needs_you(jobs, alerts, names, merged: impl Fn(&str) -> bool) -> Vec<NeedRow>`; `cards_to_watch`, `watch_work`, `watch_budget` with the TS inputs as plain structs.

- [ ] Step 1: Port every case of both test files. Run, see them fail.
- [ ] Step 2: Port. Run until green.
- [ ] Step 3: Commit `feat: Needs you and the Company's watchers in the native app`.

### Task 5: Roles, laws and the launch of a person

**Files:**
- Create: `native/src/company/roles.rs`, `native/src/company/launch.rs`, `native/src/company/launch_tests.rs`, `native/assets/company/roles/**` (copied from `src/company/roles/` of the oracle), `native/src/company/laws.rs`
- Modify: `native/build.rs` (generate `roles_gen.rs`: one `(path, include_str!(...))` per file under `assets/company/roles`, with `rerun-if-changed`)
- Oracle: `roles.ts` + `roles.test.ts`, `launch.ts` + `launch.test.ts`, `src/laws.ts` (FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS, COMPANY_COMMON), engine.ts `writePersonFiles` (people/<id>/prompt.md, mcp.json, settings.json, plugin/)

**Interfaces:**
- Produces: `roles::seed(root)` (writes shipped roles once, never over an edited one, as `seedRoles`), `roles::load(root) -> Vec<RoleDef>`; `launch::compose_prompt(person, role, company) -> String`, `launch::person_files(root, person, role, mcp_server: &Path) -> io::Result<PersonFiles { prompt, mcp, settings, plugin }>`, `launch::args(person, role, files) -> Vec<String>` (`--append-system-prompt-file`, `--mcp-config`, `--plugin-dir`, `--disallowedTools`, the Founder's `--settings {"disableAllHooks":true}`), `launch::env(person, root) -> Vec<(String, String)>` (`BYTEHIVE_AGENT_ID`, and `BYTEHIVE_COMPANY_ROOT` when not the default), `launch::node_verdict(version_text) -> Result<(), String>` (22.13 or newer).

- [ ] Step 1: Port `roles.test.ts` and `launch.test.ts` cases, plus a test that the laws carry no cmd.exe metacharacters is NOT needed (prompts go in a file) but a test that each rank gets its laws first is.
- [ ] Step 2: Copy the role folders, write build.rs generation, port. Run until green.
- [ ] Step 3: Commit `feat: roles and the launch of a Company person in the native app`.

### Task 6: The MCP server ships with the native app, and a seat can be a person

**Files:**
- Modify: `tools/build_native_installer.ps1` (run `npm --prefix mcp ci` and `npm --prefix mcp run bundle`, stage the bundle as `mcp-server\bytehive-mcp.mjs`), `native/installer/bytehive.nsi` (install `mcp-server\*`), `.github/workflows/native.yml` (Node setup step; clean-machine check lists `mcp-server\bytehive-mcp.mjs`)
- Modify: `native/src/fleet.rs` (Spec gains `#[serde(default)] pub person: Option<String>`; `start` uses the Company launch for a person instead of the laws, and sets its env)
- Create: `native/src/company/server.rs` (`mcp_server() -> Option<PathBuf>`: beside the exe in `mcp-server\`, else in dev `<repo>\mcp\dist\bytehive-mcp.mjs`; `node_check() -> Result<(), String>` running `node --version` with no window)
- Test: `native/src/company/launch_tests.rs` (a person's Spec builds the args and env), `fleet` unit test that a plain seat still gets the laws.

- [ ] Step 1: Read `mcp/package.json` for the bundle script name and output file; read `tools/build_native_installer.ps1` and the NSIS script to follow how `browser-extension` is staged.
- [ ] Step 2: Tests for the person launch and for the plain seat. Run, see them fail.
- [ ] Step 3: Implement. Build the installer locally once (`powershell -ExecutionPolicy Bypass -File tools/build_native_installer.ps1`) and list that the bundle is inside (7-Zip list or install to a temp dir with `/S /D=`); never install over the owner's app.
- [ ] Step 4: Commit `feat: the native app carries the MCP server and starts Company people`.

### Task 7: The engine

**Files:**
- Create: `native/src/company/engine.rs` (state, tick, effects), `native/src/company/host.rs` (trait), `native/src/company/requests.rs` (drain `requests.jsonl` from a byte offset, partial last line kept), `native/src/company/engine_tests.rs` (fake host), `native/src/company/fleet_host.rs` (Host over Fleet)
- Oracle: `engine.ts` + `engine.test.ts`, `src/fleetbridge.ts:235` (`drainJsonl`), `src/agentcost.ts:81` (tokens)

**Interfaces:**
- Produces: `trait Host { fn people_seats(&self) -> Vec<SeatView>; fn spawn_person(&mut self, person: &Person, files: &PersonFiles, worktree: bool) -> Result<(), String>; fn type_message(&mut self, person: &str, text: &str) -> bool; fn interrupt(&mut self, person: &str); fn at_prompt(&self, person: &str) -> bool; fn asking(&self, person: &str) -> bool; fn tokens(&self, person: &str) -> Option<u64>; fn wake(&mut self, person: &str); fn read_board(&self, dir: &str) -> Option<Board>; fn write_board(&mut self, dir: &str, board: &Board) -> bool; fn now_ms(&self) -> u64; }`; `Engine::start(root, host) -> Result<Engine, Locked>`, `Engine::tick(&mut self, host)` (heartbeat every 1.5 s, drain requests, apply, run effects; watch every 30 s), owner actions `give_job(text, floor)`, `approve(job)`, `send_back(job, note)`, `raise_cap(job, n)`, `stop_job(job)`, `finish_job(job)`, `dismiss(key)`, `view() -> CompanyView { company, jobs, needs, alerts, locked_by_other }`.
- Board type: reuse `crate::board` (`.bytehive/board.json`), with the card `stopped` flag and checklist ticks as the TS writes them.

- [ ] Step 1: Port the `engine.test.ts` cases (they use a fake ipc and fake panes: map them onto the fake Host). Where a case tests xterm screen scraping, test `at_prompt`/`asking` through the fake instead.
- [ ] Step 2: Run, see them fail. Implement engine, requests, host. Run until green.
- [ ] Step 3: `fleet_host.rs`: `at_prompt`/`asking` from `Seat::status` (`supervise::task_state`), `type_message` writes the text then Enter to the Terminal as `sendMessage` does, `interrupt` sends Esc, `tokens` from `agent_core::usage` for the person's session, `spawn_person` reserves a seat with `person` set, makes the worktree with `worktree::add` (not for the Founder: the Founder runs in the active project folder) and starts it. Trust the folder for Claude as `claudetrust.rs` does (port the needed function into `native/src/company/trust.rs` with its tests; it edits `~/.claude.json` only through that tested code path).
- [ ] Step 4: Commit `feat: the Company engine runs in the native app`.

### Task 8: The Company surface

**Files:**
- Create: `native/src/inbox/company/mod.rs` (open, close, escape, tick wiring), `rail.rs` (Needs you rows with their one button: Approve / Send back with a note box; Review / Mark done for a merge row; Add 1M tokens; Dismiss; Go to person), `jobs.rs` (job box with examples naming open projects, the Jobs list with tokens of cap and a Stop with confirm), `floors.rs` (one block per project: its people with name, role, state dot, what they are on; click opens their desk), `view.rs` (layout)
- Modify: `native/src/inbox/mod.rs` (field, render, tick), `native/src/inbox/app_keys.rs` + `native/src/terminal/keys.rs` (Ctrl+Shift+O switches Company and Inbox, as the Tauri app), `native/src/inbox/palette.rs` (Company item), `native/src/inbox/words.rs`, `native/src/inbox/style.rs`
- Oracle for look: `src/company/office/rail.ts`, `shell.ts`, `office.css` and the Task 12 shots in `D:\Temp\claude\D--bytehive\c04ff738-760b-47e9-a043-a031c38c18c0\scratchpad\t12\shots\` (rail, job box, Desk)
- Test: pure helpers in `native/src/inbox/company/tests.rs` (row buttons per kind, plan sentence, tokens short form "1.2M of 5M", which row is primary: honey on one thing only)

- [ ] Step 1: Tests for the pure helpers (port `planSentence`, `tokensShort`, row button rules from `rail.test.ts`). Run, see them fail.
- [ ] Step 2: Build the surface. A person's desk = focus their seat on the stage (Fleet::focus) with a crumb back to Company. A merge row's Review opens that person's Changes (`inbox/changes`). Opening the app with a Job on disk lands on Company, else the Inbox, as the Tauri app.
- [ ] Step 3: Run the dev app with a sandbox (`BYTEHIVE_COMPANY_ROOT` and `BYTEHIVE_NATIVE_DIR` set to scratch folders) and take shots at 1280x720 and 760x540 beside the Tauri shots. Fix what does not match.
- [ ] Step 4: Commit `feat: Company in the native app: Needs you, jobs and floors`.

### Task 9: End to end, then release

- [ ] Step 1: Full `rtk cargo test` in `native/`. Scenario A on the native dev build with the sandbox: give a marketing job on a scratch repo, send the plan back once, approve, the person starts in their worktree, type in their terminal, merge from the row, Mark done. Scenario B: one Job with two floors, the row walks each branch. Scenario C: Stop a running Job ends the turn and the cards say Stopped. Hash check of the owner's Company before and after (`node tools/company_preflight.mjs --hash` from the office worktree).
- [ ] Step 2: Memory and CPU: app tree under 1 GB with 4 people working; idle under 2%.
- [ ] Step 3: CHANGELOG `## [0.16.0]` (the Company is back in the native app; the 3D office is not yet), version 0.16.0 in `native/Cargo.toml` and `native/Cargo.lock`, commit `chore: release native v0.16.0`, merge into `feat/native-gpui`, push, tag `native-v0.16.0`, watch the Native workflow, check `latest.json` at tdat-dev/bytehive-releases (and the old maestro-releases address) says 0.16.0.
