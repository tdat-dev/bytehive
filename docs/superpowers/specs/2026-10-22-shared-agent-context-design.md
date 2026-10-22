# Shared agent context across CLIs

Date: 2026-10-22. Status: approved by the user in chat.

## Goal

Agents of different CLIs (Claude Code, Codex, OpenCode) working in one project
know what each other did, and one picks up another's unfinished task when the
first stops. Both directions, any pair of CLIs. This works for any agent that
has the maestro MCP server installed, inside or outside Maestro, even while the
Maestro app is closed.

## What exists today

- `handoff.ts handoffBrief` + `limitguard.ts handOff`: the right-click menu item
  "Hand over to <CLI>" spawns a NEW agent in the same folder. That agent is
  briefed from the old one's transcript: asks, todo plan, changed files and
  last words.
- `chatsource.ts`: reads each CLI's conversation store through Rust commands.
  `chatmodel.ts`, `codexmodel.ts` and `opencodemodel.ts` are pure parsers with
  no DOM and no IPC.
- MCP `agent_output` only returns another Maestro agent's recent screen, from
  `.maestro/fleet.json`.
- Only Claude agents spawned by Maestro get `MAESTRO_LAWS` (through
  `--append-system-prompt`). Every other agent learns about Maestro from the MCP
  tool descriptions alone.

## Design

### 1. The MCP server reads conversations itself

New module `mcp/src/context/`. Given the project folder (the existing
resolution: argv, then MAESTRO_WORKSPACE, then cwd), it searches this set of
folders:

- the project folder;
- every git worktree of the same repo (`git worktree list --porcelain`), which
  also catches Maestro's agent worktrees.

It looks for conversations active in the last 14 days, newest first, at most 20.

| CLI | Where | How |
|-----|-------|-----|
| Claude Code | `~/.claude/projects/<slug of folder>/*.jsonl` | Port `claude_project_slug`. A conversation is running when a `~/.claude/sessions/<pid>.json` with its sessionId names a live pid. |
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | Read the first line (session_meta) for the cwd and match the folder. |
| OpenCode | `~/.local/share/opencode/opencode.db` | Open read-only through `node:sqlite`. Top-level sessions whose `directory` matches, emitted as the same JSONL rows the Rust reader emits, so `createOpencodeChat` parses them. If `node:sqlite` is missing (Node older than 22.13), OpenCode is skipped and the list says so. |

Parsing reuses `src/chatmodel.ts`, `src/codexmodel.ts` and
`src/opencodemodel.ts` as they are. The mcp build changes from `tsc` to an
esbuild bundle (`dist/index.js`, packages external) so it can import from
`../src`. A long conversation is read twice:

- its start, for the first asks;
- its last 4 MB, for everything else, the same way `limitguard conversationOf`
  does.

### 2. One digest shape, shared by the app and the MCP

A new pure module `src/convdigest.ts` holds:

- `digestOf(chat, firstAsks)`, which returns `{ asks, todos, files, lastWords }`;
- the renderers built on it.

`handoff.ts handoffBrief` is refactored onto it without changing its output
(its tests stay green). Both the MCP and the app use it.

### 3. Tool `agent_context({ who? })`

**Without `who`,** it lists the conversations in this project:

```
Conversations in this project (newest first):
- Ana [3f2a91c0] Claude Code, running, active 2 min ago. Task: fix the upload test. Plan 3/7 done. Folder: D:\.maestro-worktrees\app-1a2b
- [ses_8k2] OpenCode, stopped, active 1 h ago. Task: add the export page. Plan 5/5 done.
```

**With `who`,** it returns the full context of one conversation:

- its header (CLI, state, folder, times);
- what it was asked: the first ask, then the latest ones;
- its plan with done / doing / to do;
- the files it changed;
- its last words.

When the plan has open items, it ends with: "To carry this on: check git status
in its folder, then finish the open items."

`who` matches in this order:

1. a Maestro agent name (case-insensitive);
2. an id prefix of at least 4 characters;
3. a CLI name ("claude", "codex", "opencode"), which picks that CLI's newest
   conversation.

If nothing matches, it returns the list with a hint.

**Names.** `fleet.json` gains `cli` and `session` for each agent. For `session`:

- Claude: `spec.sessionId`;
- Codex: the rollout path;
- OpenCode: the session id.

The last two come from `chatsource`'s `known` map, through a new
`conversationOf(paneId)`. The MCP uses these fields to put the Maestro name on
a conversation. Agents outside Maestro show their id only.

**Description** (what makes every CLI use it): "What other coding agents
(Claude Code, Codex, OpenCode) did in this project, including ones that
stopped. Call it with no arguments when you start working here, and with who
to read one agent's full context before you continue or take over its work."

### 4. Laws

`MAESTRO_LAWS` gets one sentence:

> Before you start a task, call agent_context to see what the other agents in
> this project did, and never redo work they finished.

`laws.test.ts` enforces its format: one line, no cmd metacharacters.

### 5. Hand over to an open agent (app)

`handoffTargets(pane)` returns two kinds of target:

- **Open agents:** other running agents of a different CLI whose `ranIn` is the
  same folder (same worktree), not busy (state idle or needs), not the conductor.
- **The existing CLI presets.**

Menu labels are "Hand over to Bo (OpenCode)" for an open agent and "Hand over
to a new Codex agent" for a preset.

Handing to an open agent sends this one line through `sendMessage`:

```
Take over the unfinished task of Ana (Claude Code), which stopped because <why>. Call the maestro agent_context tool with who set to Ana to read its task, plan, changed files and last words, check git status, then finish what is left. Say in one line what you picked up first.
```

The old agent's row title becomes "Handed to Bo". `pane.handedTo` is set, which
turns off its carry-on at reset. On the stage, a stopped agent that has any target
gets a **Hand over** button next to Resume, and the button opens the same
targets as a small menu.

### 6. Automatic takeover on usage limit (app, on by default)

- **New pref:** `autoHandover`, default true, in Settings next to "Carry on when
  a usage limit resets".
- **When it fires:** in `limitguard checkAgents`, when a new limit episode starts
  for agent A and the pref is on. The target is the first of:
  1. an open idle agent in A's folder whose CLI's plan has room
     (`planResetFor(planOf(badge)) === null`) and is not A's CLI;
  2. a new agent of the first preset in `handoffTargets` whose plan has room.
- **Once per episode.** Afterwards A is marked `handedTo`, so carry-on never
  also fires for it.
- **Notification:** "Ana ran out of usage. Bo (OpenCode) took over its task."
- **No candidate:** fall back to today's behaviour (carry on at reset).
- **Never** on a manual Stop or a closed app. Resume covers those, and a Stop is
  usually meant.

## Not in scope

- Writing context files to disk.
- Injecting context into non-Claude system prompts.
- Installing the MCP into Codex or OpenCode for the user. The README gets the
  install lines for both.

## Testing

- **mcp.** Fixtures in a temp HOME:
  - a Claude project dir with two sessions plus a sessions/pid file;
  - a Codex day folder with matching and non-matching rollouts;
  - an OpenCode db built with `node:sqlite`.

  Assert the list order, the matching rules, the digest content and the
  `who` resolution.
- **src/convdigest.** The `handoff.test.ts` output stays unchanged, and the
  digest gets new cases.
- **app:**
  - `limitguard.test.ts`: auto handover picks an idle same-folder agent with
    plan room, falls back to a new agent, then to carry-on, fires once per
    episode, and suppresses carry-on.
  - `handoffTargets`: open-agent targets.
  - `laws.test.ts` stays green.
- **Real check:** a Claude agent works in D:\maestro, then OpenCode outside
  Maestro calls `agent_context`. Do the reverse too.

## Release

- App: v0.10.0, with a CHANGELOG entry.
- `mcp/dist` is rebuilt. The user's Claude points at
  `D:\maestro\mcp\dist\index.js`.
