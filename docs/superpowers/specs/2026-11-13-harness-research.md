# Harness research: how ByteHive should build its harness

Date: 2026-11-13. Status: research report, waiting for the owner's yes.
Companion: `2026-11-13-headless-agents-shared-memory.md` (headless runs, token
measurements).

## 1. What a harness is

The harness is everything around the model that turns it into an agent:

| Part | What it does |
|------|--------------|
| Loop | call the model, run the tools it asks for, feed results back, stop when done |
| System prompt | who the agent is and how it works |
| Tools | what it can do and see (files, shell, browser, MCP) |
| Context assembly | which files, memories and notes go in before and during the task |
| Compaction and resets | what happens when the window fills |
| Memory | notes that outlive a session |
| Hooks | code that runs at points of the loop: block, add context, check |
| Sub-agents | fresh windows for side work, returning a short result |
| Verification | how "done" is checked: tests, a separate evaluator, a person |
| Permissions and sandbox | what it may touch |
| Observability | traces, usage, cost |

Claude Code, Codex, Antigravity and OpenCode are each a harness. ByteHive sits
one level up: it starts them, gives them work, holds the board and the shared
memory. That outer layer is a harness too, and it is the one ByteHive owns.

## 2. What the best sources say

From Anthropic, "Effective harnesses for long-running agents" (Nov 2025):

- Each new session starts with no memory. Fix it with artifacts, not hope: an
  initializer run writes a feature list (JSON, every item `passes: false`), a
  progress file and an init script, and makes the first commit.
- Every later session works on one feature, commits, and updates the progress
  file. JSON is safer than Markdown: models overwrite it less.
- Every session starts by getting its bearings: `pwd`, progress file, `git log`,
  the feature list, start the app and run one basic end-to-end test before
  touching anything.
- Agents declare victory too early and mark features done without testing end
  to end. Give them real test tools (browser automation) and say plainly that
  editing or removing tests is unacceptable.

From Anthropic, "Harness design for long-running application development"
(Mar 2026):

- Planner, generator and evaluator. The planner turns 1 to 4 sentences into a
  product spec, high level only (details it gets wrong cascade). The generator
  builds. The evaluator clicks through the running app and grades against
  criteria with hard thresholds.
- Agents praise their own work. A separate evaluator, tuned to be skeptical, is
  far easier to get right than a self-critical generator.
- Before each chunk, generator and evaluator agree a contract: what "done"
  means and how it will be checked. They talk through files.
- Context resets with a structured hand-off beat compaction when a model gets
  "context anxiety"; newer models need resets less.
- Cost: the full harness was over 20 times the price of a solo run ($200 and 6
  hours against 20 minutes) and far better. The evaluator pays off only on tasks at the edge of
  what the model does alone.
- "Every component in a harness encodes an assumption about what the model
  can't do on its own." Remove one piece at a time to see what is load
  bearing, and re-check with every new model.

From Anthropic, "Effective context engineering for AI agents" (Sep 2025):

- Context is a finite attention budget; recall falls as tokens grow (context
  rot). Aim for the smallest set of high-signal tokens.
- System prompt at the right altitude: not brittle if-else rules, not vague.
  Start minimal on the best model, add only what failures show is needed.
- Few tools with no overlap. "If a human engineer can't say which tool to use,
  the agent can't either." Tool results must be token efficient.
- Just in time context: keep references (paths, queries, links) and load on
  demand. Claude Code is hybrid: CLAUDE.md up front, grep and glob on demand.
- Long tasks: compaction (keep decisions, bugs, recent files; clear old tool
  results first), structured notes outside the window, sub-agents that return
  1,000 to 2,000 token summaries.

From OpenAI, "Harness engineering: leveraging Codex in an agent-first world"
(Feb 2026):

- A map, not a manual. One big AGENTS.md failed: it crowds out the task,
  everything becomes important, it rots, it cannot be checked. Keep AGENTS.md
  near 100 lines as a table of contents into a structured `docs/` (design
  docs, execution plans active and done, tech debt, references).
- What the agent cannot see does not exist: decisions in chat or in heads must
  be written into the repo.
- Make the app legible to the agent: bootable per worktree, UI driven through
  CDP, logs and metrics queryable, all torn down after the task.
- Enforce invariants mechanically (custom linters, structural tests) and write
  the lint messages as fix instructions for the agent.
- When the agent struggles, ask what capability or context is missing and add
  it to the repo; never "try harder".
- Garbage collection: recurring background agents find drift from the "golden
  principles" and open small clean-up pull requests.

From research papers (2026): "The Last Harness You'll Ever Build" (Sylph.AI)
evolves the harness itself with a worker, an evaluator and an evolution agent
that edits the harness from the history of attempts. "MemoHarness" (Notre
Dame, LMU, USC) keeps an experience bank of per-case diagnoses and distilled
patterns and adapts the harness per task from retrieved experience. Both treat
the harness, not the model, as the thing that learns.

## 3. The four harnesses side by side (this machine, 2026-11-13)

| | Claude Code 2.1.293 | Codex 0.165.0 | Antigravity 1.5.4 | OpenCode 2.2.5 |
|-|-|-|-|-|
| Instructions | CLAUDE.md user, project, local, `@` includes | AGENTS.md global and project (`project_doc_max_bytes`), `developer_instructions`, `model_instructions_file` | AGENTS.md / GEMINI.md up the tree, `.agents/rules/*.md`, global rules | AGENTS.md up the tree and global, config `instructions` |
| Own memory | auto memory files per project | `~/.codex/memories`: MEMORY.md, memory_summary.md, rollout summaries per conversation | `brain/<conversation>/` artifacts, `knowledge/` | none built in |
| Hooks | many events; SessionStart and UserPromptSubmit add context | stable; SessionStart, UserPromptSubmit add context, PreToolUse and PostToolUse Bash only | SessionStart, PreInvocation (before every model call, `ephemeralMessage`), Pre/PostToolUse, Stop | JS plugins, `experimental.chat.system.transform` |
| Sub-agents | yes | multi_agent (stable) | agents | agents |
| Headless | `-p`, stream-json in and out | `exec --json`, app-server | `-p`, stream-json in and out, ACP | `run --format json`, HTTP service, ACP |
| One-word run, tokens | 58k default, 14k lean | 26k | 30k | 64k |
| Per-run text from ByteHive | `--append-system-prompt`: works | `-c developer_instructions`: works | `.agents/rules` in worktree: works | not found yet |
| Per-turn hook from ByteHive | `--settings` hooks: works | `-c hooks...` + bypass trust: works | `.agents/hooks.json` in worktree: works | worktree plugin: did not fire |

A finding about this machine: the owner's own global files are the biggest
cost. Claude's default 58k is mostly the global CLAUDE.md with its includes,
the skills and the hooks; OpenCode's 64k comes largely from a 121 KB global
AGENTS.md. That is the "one big manual" OpenAI warns against, paid on every
start of every agent.

## 4. Principles for a good harness

1. Smallest high-signal context. Measure tokens per start; a map up front,
   everything else on demand.
2. Few, sharp tools per role. A reviewer does not need write tools.
3. Artifacts carry state between sessions: a task list with pass flags, a
   progress note, git commits. Not transcripts.
4. One unit of work at a time, ended clean: committed, noted, tests green.
5. Start every session the same way: get bearings, run a smoke test, then work.
6. Separate the doer from the judge. A skeptical evaluator with written
   criteria and hard thresholds, used where the task is at the model's edge.
7. Make the product checkable by the agent: run it per worktree, drive it, read
   its logs.
8. Turn every repeated review comment into a mechanical check whose message
   tells the agent how to fix it.
9. Reset with a hand-off when the window fills, rather than compacting forever.
10. Every piece is an assumption: measure it, remove what no longer carries
    weight, re-check with each new model.
11. Learn from runs: keep failures and fixes, feed the right ones back next
    time.

## 5. What ByteHive's harness should be

ByteHive should not write its own inner loop (model calls and tool running).
The owner's plans only work through the official CLIs, and each vendor tunes
its loop for its model. ByteHive should be the outer harness: it owns
everything that decides what each agent sees and when work counts as done.

| Outer harness part | What ByteHive does | Built on |
|-|-|-|
| Launch profiles | per role: model, tools, MCP set, permissions; lean by default (Claude 14k not 58k) | `fleet.rs run_extras`, per-CLI flags in section 3 |
| The map | a brief of at most about 100 lines per run: task, where the docs are, the laws, who else is working | per-run text injection |
| Live news | each turn, only what changed since that agent's last turn (a card moved, a hand-off arrived), through the per-turn hooks | hooks in section 3; empty when nothing changed |
| Task artifacts | the board card is the feature list: checklist items with done flags, edited through card tools only; a progress note per card | board, `card_*` MCP tools |
| Session start | the brief tells the agent to get bearings first: progress note, `git log`, start the app per worktree, smoke test | init script per project |
| Hand-off | `handoff_write` at the end of a task, at most 300 words; the next agent gets it in its brief; context reset when usage passes a threshold | new MCP tools |
| Evaluator | a separate agent, ideally another CLI (another model is less kind to the work), with the card's criteria; runs only for cards marked hard or on request | Company already has QA seats |
| Guardrails | PreToolUse hooks for destructive commands; lints with fix-it messages | hooks, project linters |
| Ledger | tokens in, cached, out and cost per run and per card; time | `usage.jsonl` |
| Learning | every failed check, send-back or revert is written as a short lesson tied to the card's kind; similar lessons go into the next brief for that kind of task | project memory, new experience file |

What ByteHive keeps out: no giant instruction files, no transcript pasting, no
global file edits behind the owner's back, no evaluator on easy tasks.

## 6. How to know it works

Pick five real tasks from the board. Run each three ways: today's setup, lean
launch plus brief, and lean plus brief plus hooks plus hand-off. Record tokens,
time, whether the result passed review, and how many send-backs. Keep a piece
only if it pays for itself, and run the same five again when a new model
lands.

## 7. Plan

1. Lean launch profiles and the brief (the biggest token saving, no new UI).
2. Live news hook for Claude, Codex and Antigravity.
3. `handoff_write`, `handoff_get`, context reset on a usage threshold.
4. Evaluator seat on another CLI for cards marked hard.
5. Ledger and the lesson file.
6. The five-task comparison, then remove what does not carry weight.

## 8. For the owner to decide

- Your global CLAUDE.md, OpenCode AGENTS.md (121 KB) and skills cost 30k to
  60k tokens on every agent start. ByteHive can leave them out of its own
  agents only, keep them as they are, or you can slim them. Which?
- Should the evaluator be a different CLI from the doer by default?
- Start with step 1 now?

## Sources

- https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
- https://www.anthropic.com/engineering/harness-design-long-running-apps
- https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- https://openai.com/index/harness-engineering/ (read from a copy at github.com/celesteanders/harness)
- https://arxiv.org/abs/2604.21003 (The Last Harness You'll Ever Build)
- https://arxiv.org/abs/2607.14159 (MemoHarness)
- https://antigravity.google/docs/hooks
- https://developers.openai.com/codex/hooks
- https://opencode.ai/docs/plugins/
