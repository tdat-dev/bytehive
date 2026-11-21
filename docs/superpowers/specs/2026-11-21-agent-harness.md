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

## Not done

- Guards for OpenCode and Antigravity: OpenCode's plugin did not fire from a worktree in an earlier test, and agy's hooks only add messages. Both need their own work.
- An OpenCode agent's MCP server runs in the shared service, so it cannot be told which agent it serves.
