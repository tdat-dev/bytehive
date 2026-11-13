# Harness research: how ByteHive should build its harness

Date: 2026-11-13. Status: research report (two passes), waiting for the owner's yes.
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

## 5. Deeper evidence (second pass, same day)

A source-code study of eleven production harnesses (arXiv 2609.00006, July
2026 snapshots of Claude Code, Codex, Gemini CLI, Mistral Vibe, OpenHands,
Aider, Mini-SWE-Agent, Hermes, Pi, OpenCode, OpenClaw) and a survey of 391
repositories (marmelab, Sep 2026) add measured facts, some of which correct
section 4:

- Loop sophistication does not predict results: a 50-line loop over one bash
  tool scores 74% on SWE-bench Verified. Harness value is in context, tools,
  checks and safety, not in a clever loop.
- Fewer tools is one of the few changes that improves success, tokens and time
  together (Vercel: 80% of tools removed, success 80% to 100%, tokens halved).
  Past about 15 tools, load them on demand (Claude Code's deferred tools).
- Rules in prose are mostly not followed: written security rules in 481
  CLAUDE.md files had an effect 4% to 16% of the time. A rule that matters must
  be a script: a hook, a lint, a test. Write the lint's message as the fix.
- Context files help only when people write them: machine-written ones lowered
  task success and raised cost by over 20%; human-written ones helped by about
  4% (ETH Zurich, 138 tasks).
- A second reviewer agent can hurt: removing harness parts one at a time
  (NLAH, arXiv 2603.25723), adding a reviewer lowered success by 8 points on a
  41% baseline. A four-role team scored 72.2% against a single agent's 71.8%.
  Past four hand-overs between agents, tasks "almost always failed"
  (Microsoft). Anthropic's evaluator paid off only at the edge of what the
  model does alone.
- What works for "it says done but it is not": a verify-on-stop guard
  (Hermes). When a turn changed code and ran no check since, the stop is
  refused and the agent is told to verify. It costs one hook, not an agent.
- Cheap caps beat clever stuck detection: hash the tool call, warn after two
  identical failures, stop after eight.
- Harnesses rot: every control compensates for something a model once got
  wrong. Give each one the incident it exists for and a condition to remove
  it; garden them (OpenAI's doc-gardening agent).
- Few harnesses check themselves: 60% have neither a test nor an eval. A guard
  needs a test both ways (fires when it should, stays quiet when it should
  not), and a harness change needs an eval: 20 to 50 frozen tasks from real
  failures, one change at a time.
- Permission rules written ahead are worse than approving actions as they come
  (113 people, 20 points fewer bad actions blocked), and 93% of prompts get
  approved anyway.
- The meta-harness has arrived: Databricks' Omnigent (June 2026) drives 23
  harnesses (Claude Code, Codex, OpenCode, Antigravity and more) behind one
  API, through five integration modes (SDK, CLI subprocess, ACP subprocess,
  native TUI, native server), tests each adapter against a conformance bench,
  lets any harness call another as a sub-agent, enforces one policy through
  each vendor's own hooks, and keeps sessions on a server with multi-device
  sync. That is where ByteHive is going; it should take the good parts and
  stay small.
- ACP (Agent Client Protocol) is the common way to drive a harness without a
  terminal: Antigravity (`agy acp`) and OpenCode (`opencode acp`) ship it, and
  adapters exist for Claude Code and Codex.

## 6. What ByteHive's harness should be

ByteHive stays an outer harness (a meta-harness): it does not run its own
model loop. What it owns, in order of value:

| Part | What it does | Why (evidence) |
|-|-|-|
| Guard plane | One policy file per project (blocked commands, protected paths, required checks) turned into each CLI's own hook: Claude PreToolUse, Codex PreToolUse (Bash), Antigravity PreToolUse, OpenCode plugin. Every guard ships with tests both ways. | prose rules hold 4% to 16%; Omnigent does this across vendors |
| Verify-on-stop | A Stop hook: if the turn edited files and ran no check since, the stop is refused with "run the checks first" (the project's check command, from the policy file). | cheapest fix for a false "done"; Hermes |
| Lean launch | Per role: a short tool list, only the bytehive MCP, no skills unless the role needs them; tokens per start measured. | fewer tools improves success; 58k to 14k measured here |
| The map | A brief of at most 100 lines, written by people, pointing to docs; nothing machine-generated pasted in. | OpenAI; ETH Zurich |
| Live news | A per-turn hook adds only what changed since the agent's last turn. | cheap, tested in three CLIs |
| Hand-off | A written note in files (card, progress note, git), never more than four hand-offs on one task. | Anthropic; Microsoft |
| Reviewer | Off by default. On for cards marked risky, and then a different vendor than the doer, judged against the card's own checks. | NLAH -8 points; Omnigent's Polly |
| Caps | Same failing call twice: warn; eight times: stop and ask. A token cap per card (the Company has one). | Hermes; cheap |
| Telemetry | Per session: tools used, guards fired, stops refused, checks run, tokens. Read from the transcripts ByteHive already parses. | 5 of 391 repos measure; nothing improves unseen |
| Harness bench | 20 to 50 frozen tasks from the owner's real past corrections; run a CLI with and without a change; keep what wins. | the only way to know a part helps |
| Garden | Each guard and rule records its incident and expiry; a monthly pass proposes removals. | harness rot |

## 7. Plan

1. Verify-on-stop and the guard plane for Claude Code first (hooks given per
   run through `--settings`, so nothing global changes), with tests both ways.
2. The same guards for Codex (`-c hooks...`) and Antigravity
   (`.agents/hooks.json` in the worktree).
3. Lean launch profiles and the brief.
4. Telemetry from transcripts, shown per agent: what it ran, what was blocked,
   tokens.
5. Harness bench with tasks taken from past sessions where the owner corrected
   an agent.
6. ACP as the headless driver for Antigravity and OpenCode (ties into the
   headless spec).
7. A reviewer on another vendor for risky cards, measured on the bench before
   it becomes a default.

## 8. For the owner to decide

- Your global CLAUDE.md, OpenCode AGENTS.md (121 KB) and skills cost 30k to
  60k tokens on every agent start, and the evidence says big instruction
  files hurt. ByteHive can leave them out of its own agents, or you can slim
  them. Which?
- Start with step 1 (verify-on-stop and guards) now?

## Sources

- https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
- https://www.anthropic.com/engineering/harness-design-long-running-apps
- https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- https://openai.com/index/harness-engineering/ (read from a copy at github.com/celesteanders/harness)
- https://arxiv.org/abs/2609.00006 (Harness Engineering: a source-code study of eleven systems)
- https://marmelab.com/blog/2026/09/24/the-state-of-ai-harness-engineering-2026.html
- https://arxiv.org/abs/2603.25723 (Natural-Language Agent Harnesses, the ablation)
- https://arxiv.org/abs/2604.21003 (The Last Harness You'll Ever Build)
- https://arxiv.org/abs/2607.14159 (MemoHarness)
- https://www.anthropic.com/engineering/code-execution-with-mcp
- https://antigravity.google/docs/hooks
- https://developers.openai.com/codex/hooks
- https://opencode.ai/docs/plugins/
