# Headless agents with shared memory

Date: 2026-11-13. Status: research done, waiting for the owner's yes before any code.

## Goal

From ByteHive, hand a task to Codex, Antigravity, Claude Code or OpenCode
without opening a terminal. The agent starts already knowing the project and
what the other agents did, and a later task to the same agent never re-teaches
it. Tokens spent on context stay small and are measured.

## What exists today

- Native seats run each CLI in a ConPTY terminal (`fleet.rs`). Only Claude gets
  ByteHive text, through `--append-system-prompt` with the laws
  (`fleet.rs run_extras_with`). Codex, OpenCode and Antigravity start bare.
- The `bytehive` MCP server is already registered in all four CLIs' own
  configs on this machine: `~/.claude.json`, `~/.codex/config.toml`,
  `~/.config/opencode/opencode.json` and `agy mcp list`. Its tools
  `agent_context`, `memory_get`, `board_get` and `fleet_send` work from any of
  them.
- `agent_context` reads Claude, Codex and OpenCode conversation stores
  (2026-10-22 spec). It does not read Antigravity's store
  (`~/.gemini/antigravity-cli/conversations/<id>.db`, one SQLite file per
  conversation, 78 on this machine).
- Seats remember their session id (`Spec.session_id`) and resume it.

## Research 1: the harnesses

A harness is the program around the model: its system prompt, tool
definitions, the files it reads into context, compaction and resume. Each CLI
is one. ByteHive does not replace them; it feeds them.

### Headless modes (versions on this machine)

| CLI | One task | Long-lived process | Resume | Usage reported |
|-----|----------|--------------------|--------|----------------|
| Claude Code 2.1.293 | `claude -p --output-format json` | `-p --input-format stream-json --output-format stream-json`: one process, a turn per stdin line | `--resume <id>`, `--session-id <uuid>` to choose it | `usage` with cache fields, `total_cost_usd` |
| Codex 0.165.0 | `codex exec --json` (close stdin, or it waits for it) | `codex app-server` daemon (JSON-RPC, threads); `codex queue --thread` to add a message to a live session | `codex exec resume <thread>` | `turn.completed.usage`, cumulative per thread |
| Antigravity `agy` 1.5.4 | `agy -p --output-format json` | `-p --input-format stream-json --output-format stream-json`; also `agy acp` (Agent Client Protocol over stdio) | `--conversation <id>`, `-c` for the latest | `usage` cumulative per conversation |
| OpenCode 2.2.5 | `opencode run --format json` | background service with an HTTP API (`POST /api/session/:id/prompt`); `opencode acp` | `--session <id>`, `--fork` | per message `tokens` |

Gotchas found while measuring:

- `codex exec` does not exit after `turn.completed`: it sat until a 300 s
  timeout. ByteHive must end the run itself on `turn.completed`, or use the
  app-server.
- OpenCode's default model on this machine is `kaggle/...` at
  `http://localhost:9000/v1`, which was down: the run failed with
  ECONNREFUSED and then hung. Not measured. Headless runs need a model check
  and a timeout.
- Antigravity resume took 41 s for a one-word answer.

### Where each harness reads context, and what ByteHive can write per run

Per run means a flag or a file in the agent's own worktree. ByteHive never
edits the owner's global files (`~/.claude`, `~/.codex`, `~/.gemini`,
`~/.config/opencode`).

| CLI | Reads from | Per-run injection, tested |
|-----|------------|---------------------------|
| Claude Code | system prompt, `CLAUDE.md` (user, project, local, with `@` includes), skills, hooks, MCP server instructions | `--append-system-prompt[-file]` (already used), `--mcp-config` + `--strict-mcp-config`, `--settings`, `--tools`, `--setting-sources` |
| Codex | base instructions (`model_instructions_file`), `developer_instructions`, `AGENTS.md` (global, then project up to `project_doc_max_bytes`) | `-c developer_instructions="..."`: tested, the agent answered from it |
| Antigravity | `AGENTS.md` / `GEMINI.md` in the start folder and parents, `<dir>/.agents/rules/*.md`, global `~/.gemini/...` rules | `.agents/rules/bytehive.md` written in the worktree: tested, the agent answered from it. Keep it out of git with `.git/info/exclude` (gitexclude.rs already does this for `.bytehive`). |
| OpenCode | `AGENTS.md` up the tree, global `AGENTS.md`, config `instructions` array (files, globs, URLs) | Not tested (provider down). Candidates: `OPENCODE_CONFIG` pointing at a ByteHive config with `instructions`, or a rule file in the worktree. Verify first. |

So yes: ByteHive can write into every agent, without touching the owner's
files.

### Own harness or wrap the CLIs?

An own harness (ByteHive calls the model API and runs its own tool loop) would
own every token and could share one memory across models. It loses on the
deciding point: the owner's Claude, ChatGPT and Google plans work only through
their official CLIs. A direct API is billed per token and needs keys. It would
also mean rebuilding editing, shell, sandbox, permissions and each vendor's
tuned prompts.

Decision proposed: wrap the CLIs headless. ByteHive owns the memory and the
routing, not the loop.

## Research 2: tokens (measured 2026-11-13, empty git folder, "Reply with only the word ok.")

| Run | Tokens in | Of which cached | Cost / note |
|-----|-----------|-----------------|-------------|
| Claude, default (all MCP, skills, hooks, global CLAUDE.md) | 58,435 | 0 (all cache write) | $0.47 |
| Claude, only bytehive MCP, no skills, no hooks | 42,777 | 0 | $0.34 |
| Claude, as above + `--setting-sources project,local` | 25,518 | 22,223 | $0.03 |
| Claude, as above + `--tools` six tools | 13,929 | 0 (new prompt) | first run of that prompt |
| Claude, second turn by `--resume` | 13,949 | 12,801 | 1,144 fresh |
| Codex, default config | 26,226 | 0 | |
| Codex, `--ignore-user-config` | 27,237 | 12,416 | |
| Codex, second turn by `exec resume` | +27,260 | +27,008 | about 250 fresh |
| Antigravity, default | 30,578 | 0 | 11.5 s |
| Antigravity, second turn by `--conversation` | +22,225 | +8,192 | about 14,000 fresh, 41 s |
| OpenCode, `-m deepseek/deepseek-flash` | 63,942 | 0 | the global `~/.config/opencode/AGENTS.md` alone is 121 KB |

Per-turn hooks were tested on 2026-11-13 too (see the harness report,
`2026-11-13-harness-research.md`): a hook given per run reached the model in
Claude (`--settings` hooks), Codex (`-c hooks...` plus
`--dangerously-bypass-hook-trust`) and Antigravity (`.agents/hooks.json` in the
worktree, `PreInvocation` with an `ephemeralMessage`). An OpenCode plugin in
the worktree's `.opencode/plugins/` using `experimental.chat.system.transform`
did not reach the model.

What this says:

1. The harness itself costs 14k to 58k tokens before any work. A ByteHive
   brief of 1 to 2k tokens is small next to it, so the brief is not the
   problem. Launch flags are: Claude's default launch costs four times the lean
   one.
2. Resume is the real saving. Claude and Codex bill almost nothing fresh on a
   second turn while the cache is warm (Claude's prompt cache lasts about 5
   minutes). Antigravity caches little.
3. Reloading another agent's transcript is what costs: a long conversation is
   hundreds of thousands of tokens. A digest is a few hundred.

## Design

### 1. A headless runner per CLI (`native/src/headless/`)

One module per CLI behind one trait: start, send a task, stream events, stop,
resume id, usage. No terminal. A run shows in ByteHive like a seat (row,
state, last words, usage), with an "Open terminal" button that resumes the
same session interactively when a person wants to look.

- Claude: one long-lived `-p` stream-json process per agent (protocol already
  verified, see `claude-stream-json-protocol`). Lean flags: bytehive MCP only,
  `--strict-mcp-config`, `--disable-slash-commands`, hooks off,
  `--setting-sources project,local`, and a short `--tools` list per role.
- Codex: `codex exec --json` and `exec resume`, ended on `turn.completed`.
  Move to `app-server` later if starting a process per task is slow.
- Antigravity: `agy -p --input-format stream-json --output-format
  stream-json`, one process per agent; `--conversation` to come back.
- OpenCode: its HTTP service and `/api/session/:id/prompt`, after the model
  check.

### 2. Shared memory in three tiers

- Tier 0, always in context (budget 1,500 tokens): a brief ByteHive writes
  per run. It holds the task (card title, description, checklist), the
  project memory facts (`memory.json`), one line per agent at work (who, CLI,
  card) and the laws. It is written to `.bytehive/briefs/<agent>.md` and given
  through the per-run injection above.
- Tier 1, on demand: MCP tools the agent calls only when it needs them:
  `agent_context(who)`, `memory_get`, `board_get`, and a new `handoff_get`.
  Nothing is pasted in advance.
- Tier 2, the agent's own conversation: always resumed, never rebuilt.
  ByteHive keeps every agent's session id (Claude session, Codex thread,
  Antigravity conversation, OpenCode session).

Hand-over between agents: at the end of a task the agent writes a hand-over
note (at most 300 words) through a new MCP tool `handoff_write`: what was done,
files changed, what is left. The next agent gets it in its brief. It does not
read the first agent's transcript.

### 3. Fill the gaps

- `agent_context` learns Antigravity's store (`conversations/*.db`, read only).
- A token ledger per run (in, cached, out, cost) under
  `.bytehive/usage.jsonl`, shown on the run's row. The Company's budgets
  already exclude cache reads (m13).

## Plan, small steps

1. Runner trait + Claude stream-json runner + one "Run without terminal" entry
   in the New task sheet. Measure a real task against the same task in a
   terminal seat.
2. Codex and Antigravity runners.
3. Brief writer (tier 0) + per-CLI injection + git exclude for
   `.agents/rules/bytehive.md`.
4. `handoff_write` / `handoff_get` + Antigravity in `agent_context`.
5. OpenCode runner once its injection is verified.

## Open questions for the owner

- Should Claude's lean launch also become the default for terminal seats?
  It saves about 30k to 45k tokens per start but drops the owner's skills and
  global CLAUDE.md for those agents.
- Which CLI goes first after Claude: Codex or Antigravity?
- Antigravity's weak cache (14k fresh per resumed turn) makes it the costly
  one for many small tasks. Prefer it for long tasks?

## Sources

- Codex config keys: https://developers.openai.com/codex/config-sample
- Antigravity rules: https://antigravity.google/docs/rules
- OpenCode rules and server: https://opencode.ai/docs/rules/, https://opencode.ai/docs/server/
- CLI `--help` output of the versions above, and the runs in this session.
