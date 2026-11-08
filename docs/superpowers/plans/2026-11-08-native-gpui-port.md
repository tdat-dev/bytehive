# ByteHive native (Rust + GPUI): port plan

Written 2026-11-08 after the first slice passed the owner's gate (looks, terminal keys and Telex, RAM 46 MB vs 238 MB private working set at five terminals). Work happens on `feat/native-gpui`, crate `native/`. The Tauri app stays the daily build until phase 5.

## Owner decisions (2026-11-08)

- The in-app code editor (CodeMirror) is dropped: files open in VS Code. The file tree and the diff view stay.
- No screen reader support for now (GPUI has no UI Automation). Revisit when GPUI adds it.
- Company comes after cutover; Tauri keeps running it until then.
- A Tauri 0.14.2 release writes the fleet and settings to `~/.bytehive/export/web-state.json`, which native imports once.

## Releases repo

The public releases repo is `tdat-dev/bytehive-releases` (renamed from `maestro-releases` on 2026-11-08; never create a repo by the old name). Before tagging the 0.14.2 export release, merge branch `chore/bytehive-releases` (commit 1806505: release.yml, the tauri.conf.json updater endpoint, README, docs). The native updater in phase 5 points at `https://github.com/tdat-dev/bytehive-releases`, never the old name.

## Rules that do not bend

- Terminal input: raw Win32 key messages through win32-input-mode (`native/src/keys.rs`), never a key table. Only Ctrl+V and Alt+F4 stay local over a terminal.
- Looks are checked by real screenshots against the running Tauri app, zoomed side by side, and by measurement when they differ (the "heavy text" turned out to be a Bold file named Medium).
- `.bytehive/` files stay byte compatible, so the MCP server, board and memory work with either app.
- Never two apps on one fleet: native refuses fleet writes while a ByteHive (Tauri) process is alive.

## Phases

| # | Slice | Done when | Estimate |
|---|---|---|---|
| 1 | Live fleet: agent-core registry behind the queue, New task (CLI, folder), stop, resume, stage switch, needs-you state, fleet saved to `~/.bytehive/native/session.json` | Claude, Codex and OpenCode spawn; switching keeps scrollback; stop, quit, relaunch shows it stopped; resume continues the conversation | 4 to 6 days |
| 2 | Import and daily driver: 0.14.2 export and import, projects, worktree per agent, Ctrl+K, settings, tray, toasts, follow the session after /clear, single instance | The owner's real fleet appears stopped and resumes | 5 to 7 days |
| 3 | Review loop: diff, land, merge, discard, files drawer (opens VS Code), History, markdown | An agent's worktree is reviewed and merged without leaving the app | 6 to 9 days |
| 4 | Fleet intelligence: usage, pricing, plan meter, limit handoff, Board, memory, MCP bridge files, schedules | The MCP server drives native as it drives Tauri | 5 to 7 days |
| 5 | System and cutover: browser hub, phone dashboard, installer, updater, release workflow | Clean machine install works; 0.14.x updates carry data over | 5 to 7 days |
| 6 | Company engine in Rust, `engine.test.ts` as the oracle; the office is a later design (there is no shipped 3D office) | Engine scenarios replay the same | 8 to 12 days |

The estimates are inferred from the egui build's pace and are the weakest numbers here.

## Reuse

- `crates/agent-core` (restored from v0.12.13): registry, PTY and pipe sessions, git, worktrees, landing, transcripts, usage.
- From v0.12.13 when their phase comes: `supervise` (task state, asks, limits, digest), `browser-hub`, `dashboard`. Restore with `git restore --source v0.12.13` inside this worktree.
- gpui-kit 0.9.0 ships an Input with IME handling, Diff, Markdown, a highlighter, a command palette and a dock: try them before building our own.

## Known GPUI gaps

- No letter-spacing: the 24px Inbox headline runs about 8 percent wider.
- No radial gradient: the faint amber glow at the top is missing (a pre-rendered image can stand in).
- Startup measured 6 to 7 s on a busy machine, 2.2 s of it the app's own CPU: measure on an idle machine before cutover.
