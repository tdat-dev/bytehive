# bytehive-mcp

MCP server for the [ByteHive](https://github.com/tdat-dev/maestro) kanban board.
Lets an AI agent (Claude Code, etc.) read and edit the board of the workspace it
runs in — the same board the ByteHive app renders. Source of truth is
`.bytehive/board.json` in the workspace folder.

People in the ByteHive Company skip all of this: the app carries a one-file build
of this server (`npm run build:mcp` at the repo root) and hands it to them with
`--mcp-config`. It needs Node.js on PATH.

## Install (Claude Code)

The package is not on npm yet, so install from a local checkout. One-time, user
scope: it works in every workspace because the server resolves the board from
the directory the agent runs in:

```
cd mcp && npm install && npm run build
claude mcp add --scope user bytehive -- node <absolute path to mcp/dist/index.js>
```

## Install (Codex, OpenCode)

Codex, in `~/.codex/config.toml`:

```toml
[mcp_servers.bytehive]
command = "node"
args = ["<absolute path to mcp/dist/index.js>"]
```

OpenCode, in `~/.config/opencode/opencode.json` under `"mcp"`:

```json
"bytehive": { "type": "local", "command": ["node", "<absolute path to mcp/dist/index.js>"], "enabled": true }
```

## Tools

| Tool | What it does |
|------|--------------|
| `board_get` | Read the whole board (lists, cards, ids) |
| `card_add` | Add a card (`list`, `title`, `desc?`, `labels?`, `due?`, `checklist?`) |
| `card_update` | Patch a card's fields |
| `card_move` | Move a card to another list / position (moving into "Doing" claims the card for the calling agent) |
| `card_delete` | Delete a card |
| `card_done` | Move a card to Done + attach git change evidence and who finished it |
| `list_add` / `list_rename` / `list_delete` | Manage lists |
| `fleet_status` | List the other agents ByteHive runs in this workspace + their live status (needs / active / idle / stopped) |
| `fleet_send` | Send a message into another agent's terminal (or broadcast to all) — for hand-offs and coordination |
| `agent_output` | Read the recent on-screen text of another agent (by name) — check a worker's progress or whether it's stuck |
| `agent_context` | What every coding agent (Claude Code, Codex, OpenCode) did in this project, read from their own conversations, including ones that stopped and ones run outside ByteHive. No arguments: a list. `who` (a name, an id, or a CLI): one agent's asks, plan, changed files and last words, to carry its task on |
| `agent_spawn` | Ask ByteHive to spawn new worker agent(s) in this workspace — a conductor grows its own crew (with an optional task) |

Cards and lists are addressed by id (from `board_get`) or by title; ambiguous
titles are rejected with a hint to use the id. Labels:
`green | yellow | orange | red | purple | blue`. Due dates: `yyyy-mm-dd`.

## Agent identity

ByteHive sets `BYTEHIVE_AGENT=<pane name>` (and `BYTEHIVE_WORKSPACE=<folder>`) in
every terminal it spawns. When present, `card_done` records it as `done.by`
and `card_move` into "Doing" sets it as the card's `assignee` (never
overwriting an existing one) — so the ByteHive board shows which agent is
working on, and finished, each card.

## Fleet coordination

`fleet_status` and `fleet_send` let an agent see and message the rest of the
fleet through a file bridge (same idea as the board): ByteHive publishes the
roster to `<workspace>/.bytehive/fleet.json`, and `fleet_send` appends to
`<workspace>/.bytehive/outbox.jsonl`, which ByteHive watches and types into the
target agent's terminal. This is the substrate a "conductor" agent uses to
hand work to idle agents.

A conductor goes further with `agent_output` (read a worker's screen to see its
progress) and `agent_spawn` (grow its own crew): ByteHive publishes each agent's
recent screen into `fleet.json`, and `agent_spawn` appends to
`.bytehive/spawn-requests.jsonl`, which ByteHive watches and boots the requested
CLI(s) into the same workspace.

## Shared context across CLIs

`agent_context` reads each CLI's own store for the project folder and every
git worktree of its repo, for the last 14 days: Claude Code's
`~/.claude/projects/<folder>/*.jsonl`, Codex's `~/.codex/sessions/`
rollouts, and OpenCode's `~/.local/share/opencode/opencode.db` (through
`node:sqlite`, so Node 22.13 or newer). It needs nothing from the ByteHive app;
when the app runs, it also names each conversation after its agent, from the
`cli`/`session`/`folder`/`since` fields of `.bytehive/fleet.json`.

The build bundles the app's own conversation parsers (`../src/chatmodel.ts`,
`codexmodel.ts`, `opencodemodel.ts`, `convdigest.ts`) with esbuild.

## How it stays in sync

Every tool call re-reads `.bytehive/board.json`, applies the change, and writes
it back atomically. The ByteHive app watches the file and re-renders within a
few seconds; UI edits land in the same file, so the next tool call sees them.

## Limitations

The server resolves the board from its process's current working directory.
An agent ByteHive spawned into an isolated git worktree (ByteHive's worktree
isolation feature) has that worktree as its cwd, so by default it reads and
writes `<worktree>/.bytehive/board.json` — a separate file from the main
workspace board shown in the ByteHive kanban panel, not the same board.
Workaround: pass the workspace directory explicitly, e.g.
`bytehive-mcp <dir>` (or `node dist/index.js <dir>` from a local checkout), so
the agent manages the same board.json the UI renders regardless of its cwd.
