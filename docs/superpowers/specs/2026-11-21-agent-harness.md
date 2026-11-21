# One harness for every agent CLI

Date: 2026-11-21. Owner: "làm hết cho tất cả agents đi, nó phải là 1 bộ khi cài bytehive đi với nhau hết đầy đủ và sau này là harness không thể bị thiếu."

## What every agent gets

| Piece | What it is |
|---|---|
| Laws | `crate::laws`: the board protocol, who outranks whom, English between agents, agent_context and memory first. The Director gets its own set. |
| MCP | the `bytehive` server of the installed app (`mcp-server\bytehive-mcp.mjs` beside `bytehive.exe`), never an old copy a config file points at. |
| Guards | `crate::guard`: commands that destroy work are refused; a turn that changed code runs a check before it ends. |
| Identity | `BYTEHIVE_AGENT`, `BYTEHIVE_WORKSPACE`, `BYTEHIVE_RUN_DIR`, `BYTEHIVE_HUB_FILE` in the agent's environment (fleet.rs). |

## How each CLI gets them

Per run where the CLI has a way, so the owner's files are not touched and the app's copy always wins. Where a CLI reads a piece only from its own config file, ByteHive keeps its one entry in that file right at every start (`harness/globals.rs`, one entry edited, everything else byte for byte, a `.bytehive-bak` of the file as it was).

| CLI | Laws | MCP | Guards |
|---|---|---|---|
| Claude | `--append-system-prompt` | `--mcp-config <data>/harness/claude-mcp.json` | `--settings` hooks |
| Codex | `-c developer_instructions='…'` | `-c mcp_servers.bytehive.command/args/env_vars` | `-c hooks.*` |
| Antigravity (agy) | `<run dir>/.agents/rules/bytehive.md`, kept out of git by `info/exclude` | entry in `~/.gemini/antigravity/mcp_config.json` | none found |
| OpenCode | the MCP server's own instructions | entry `mcp.bytehive` in `~/.config/opencode/opencode.json` | none found |
| Gemini, Qwen, Aider, Cursor, Copilot, Goose | none yet (not installed here, nothing tested) | none yet | none yet |

`harness::coverage` holds this table in code. A test fails when a preset has no line in it, and when Claude or Codex lose a piece.

## Measured

- Claude: a per-run `--mcp-config` server named `bytehive` replaces the global one of the same name (init event: one `bytehive`, `source: "dynamic"`, the per-run server's tool listed, the global one's not).
- Codex 0.165: `-c` overrides replace the config.toml entry, `env_vars` is accepted (`codex mcp get bytehive --json`), and the 1483-character laws pass through `cmd /c` whole. Codex starts MCP servers with a short list of variables, so the identity names go in `env_vars`.
- OpenCode 2.2.5: `OPENCODE_CONFIG_CONTENT` is loaded by `debug config`, but the TUI talks to a shared background service (`opencode serve --service`), which never sees a per-run config; `--standalone` listed no servers at all. Its own `mcp add --global` wrote a v2 `mcp.servers` entry into the v1 file beside the old one, so the file is edited by ByteHive instead.
- Antigravity 1.5.13: `--settings` rejects `mcpServers`, `hooks`, `rules`, `instructions` ("unknown setting"). `agy mcp list` shows the edited entry.
- Before: OpenCode's and Antigravity's entries pointed at `D:/bytehive/mcp/dist/index.js` (an old dev build without browser_record), Antigravity's with `D:\bytehive` fixed in as the workspace for every project; Codex's config.toml the same.

## Fewer tokens, news each turn (0.19.1, 0.19.2)

- `board_get` gives a short text view (`mcp/src/boardview.ts`): 70 KB to 7.8 KB on ByteHive's own board. The memory tools answer with the one fact they changed, not the 16 KB memory. The laws no longer send an agent to `memory_get` when the memory came in the MCP instructions.
- The browser tools: Claude Code (client name `claude-code`) loads MCP schemas only when needed and keeps the 19 separate tools; every other client gets one `browser` tool (`mcp/src/browsergate.ts`), 2.5 KB instead of 9.5 KB per request. `BYTEHIVE_BROWSER_TOOLS=each|one` overrides. Measured: Codex ignores tools added mid-session (`tools/list_changed`), so loading them later was no option. A real Codex run used the single tool to list browsers, pick one and read a page.
- News (`native/src/guard/news.mjs`, wired with the guards): UserPromptSubmit tells the agent in a few lines which cards were added, moved or deleted and which facts were saved or changed since its last turn; Stop takes in the agent's own turn so it is not told what it did. Measured: Claude (`--settings`) and Codex (`-c hooks.UserPromptSubmit`) both put the hook's `additionalContext` in front of the model. A change someone else makes during the agent's turn is taken in silently with it.

## Not done

- News for Antigravity (its `.agents/hooks.json` PreInvocation format is not written down here yet) and for OpenCode (no hook that fires).

- Guards for OpenCode and Antigravity: OpenCode's plugin did not fire from a worktree in an earlier test, and agy's hooks only add messages. Both need their own work.
- An OpenCode agent's MCP server runs in the shared service, so it cannot be told which agent it serves.
