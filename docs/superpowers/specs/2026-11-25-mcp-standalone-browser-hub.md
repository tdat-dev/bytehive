# The browser tools without the app: a standalone hub in bytehive-mcp

Date: 2026-11-25. Branch: `feat/mcp-standalone-hub` (from `feat/native-gpui`).

## The ask

The bytehive MCP server (`browser_navigate`, `browser_read_page`, `browser_computer`
and the rest) should drive the owner's real Chrome even when ByteHive is closed.
With ByteHive open it must work as it does today, the two must not fight over the
browser, and risky clicks (send, post, pay, delete) must still wait for the user's OK.

## How it works today

```text
agent (Claude Code, Codex...) ─stdio─ bytehive-mcp ─ws─┐
                                                       ├─ hub, inside ByteHive (crates/browser-hub)
Chrome extension ─native messaging─ bytehive-host-*.exe ─ws─┘
```

- **Hub file.** `~/.bytehive/browser-hub.json` holds `{port, token, pid}`. The app's
  hub binds `127.0.0.1:0`, writes the file at start, and every 5 s takes it back if
  it names a port nobody answers on (`crates/browser-hub/src/hub/file.rs`). A test
  run of the app writes its own file and passes it to its agents in `BYTEHIVE_HUB_FILE`.
- **Wire.** One WebSocket per peer. The first message is
  `{type:"hello", role:"agent"|"browser", token, agent?}`, answered with `hello_ok`.
  Agents send `{type:"call", id, tool, args}`. The hub picks the agent's browser,
  adds `args._ask`, and forwards `{type:"call", id:hid, agent, tool, args}`. The
  browser answers `{type:"result"|"error", id:hid, ...}`; the hub swaps the id back.
  Browsers also send `browser_info` (browser, email, extension version) and `pause`
  (the Stop pill over the page).
- **Risky clicks.** With `_ask`, the extension does not click a left-click target
  whose label matches `RISKY` (`browser-extension/background.js`); it answers
  `needsConfirm: {what, url}`. The hub tells the agent `{type:"hold", id}` (wait up
  to 10 min), shows the ask in ByteHive, and on OK sends the call again with
  `confirmed: true`.
- **Native host.** Chrome starts `%LOCALAPPDATA%\ByteHive\NativeHost\bytehive-host-*.exe`
  (a copy of bytehive.exe) and keeps it running while Chrome runs, app or no app.
  It re-reads the installed hub file and dials it every 2 s; it tells the extension
  `hub_up` / `hub_down` and replays `browser_info` on every reconnect.
- **MCP side.** `HubClient` (`mcp/src/browser.ts`) reads the hub file on the first
  call, dials it and fails with "ByteHive isn't running" when nothing answers.

So with the app closed, Chrome, the extension and the native host are all still
there and still dialling. The only missing piece is a hub at the port the file names.

## The approach: the MCP becomes the hub when no app is there

`bytehive-mcp` carries a small hub of its own (TypeScript, same wire as the Rust
hub). Nothing changes in the app, the native host or the extension.

### Who runs the hub

On a browser call, `HubClient` resolves the hub:

1. Read the hub file (`BYTEHIVE_HUB_FILE`, else `~/.bytehive/browser-hub.json`).
   If its port answers, connect as today.
2. Otherwise try to become the hub: take `~/.bytehive/browser-hub.lock` (created
   exclusively, holds our pid; a lock whose pid is gone is stale and removed). With
   the lock, look at the file once more, then start the hub on `127.0.0.1:0` with a
   fresh token and write `{port, token, pid, standalone: true}` to the installed
   file `~/.bytehive/browser-hub.json` (the one the native host reads).
3. If another MCP process holds the lock, wait up to 3 s for the file to name a hub
   that answers, then connect to it.

The process that runs the hub also connects to it as an ordinary agent, so there is
one code path for calls. Other MCP processes (other Claude Code sessions, Codex...)
connect to it like they connect to the app.

The native host finds the new hub on its next dial (within 2 s). The first call on a
fresh hub waits up to 5 s for a browser to say hello before it answers "no browser".

### No fight with the app

- **The app always wins.** The app writes the hub file when it starts. The standalone
  hub reads the file every 2 s; once it names another port that answers, it steps
  down: it fails its in-flight calls with "ByteHive just opened and took over the
  browser; try again", closes every socket and the server, and drops the lock. The
  native host and the agents then dial the file again and reach the app.
- **The standalone hub never claims a live hub's file.** It starts only when the
  named port does not answer, and the app's own 5 s watcher keeps its file while the
  app runs.
- **App closes while agents work.** The file names a dead port; the next browser call
  from any MCP process starts a standalone hub (step 2).
- **The hosting MCP process exits** (its session ended). On exit it removes the hub
  file if the file still names it, and the lock. The next call from another MCP
  process starts a new hub. Calls in flight at that moment fail and can be retried.
- **Test runs of the app** (`BYTEHIVE_HUB_FILE` set to their own file) are left alone:
  the standalone hub only ever writes the installed file.

### Safety without the app's window

- The standalone hub always sends `_ask: true` (there is no setting to turn it off
  without the app), and it removes `confirmed` and `_ask` from what agents send, so
  an agent cannot skip the check by passing `confirmed: true` itself.
- When the extension holds a click, the hub sends the agent's own connection
  `{type:"confirm", id, ask, what, url}`. That agent's MCP process asks **its user**
  through MCP elicitation (the client shows a yes/no form to the person; the model
  cannot answer it) and replies `{type:"answer", ask, ok}`. The hub takes the answer
  only from the connection that owns the hold, once, within 10 minutes; on yes it
  sends the call again with `confirmed: true`, on no the agent is told not to retry.
- A client without elicitation support, or a failed or cancelled form, means **no**:
  the agent is told "ByteHive is closed, so nobody can OK this click; ask the user to
  do it themselves or to open ByteHive". Nothing is ever allowed by default.
- **Stop pill.** A `pause` from the page stops that agent's browser calls. Without the
  app there is no Go on button, so the agent's next call asks the user (same
  elicitation) "You pressed Stop for X. Let it use the browser again?"; no means it
  stays stopped.
- Same exposure as the app's hub: loopback only, random token, file in the user's
  home.

### What the standalone hub leaves out

- Steps on pages Chrome keeps from extensions (`chrome://`, the Web Store, Load
  unpacked): those are done from Windows by the app. Without it the agent is told to
  ask the user, or to open ByteHive.
- Telling an outdated extension to reload, profile names from Chrome's Local State
  (it labels a browser `Chrome · email`), the live view and activity list.

## Code

- `mcp/src/hub/server.ts`: WebSocket server, peers, hello and token.
- `mcp/src/hub/route.ts`: routing calls, list and select browser, holds, pause.
- `mcp/src/hub/claim.ts`: hub file, lock, step-down watcher, exit clean-up.
- `mcp/src/browser.ts`: `HubClient` resolves or starts the hub; handles `confirm`
  through elicitation.
- Dependency: `ws` (bundled by `bundle.mjs` into `bytehive-mcp.mjs`, as today).
- Tests (vitest): routing with a fake browser and agent over real sockets; the hold
  and answer flow, including a refused answer from another connection; step-down
  when the file names another live hub; stale lock clean-up.

## Shipping

The installer puts the bundle at `<app>\mcp-server\bytehive-mcp.mjs`, which is what
Claude Code's `bytehive` entry runs. So it reaches the owner with the next native
release (version bump, CHANGELOG, tag). No app code changes.

## Checked by hand before done

1. ByteHive closed: from a Claude Code session using the new bundle, navigate, read a
   page and click a plain link in the owner's Chrome. A risky click asks in the
   client and is refused when declined.
2. ByteHive open: the same calls go through the app's hub (hub file names the app's
   pid), and the app's ask still shows for a risky click.
3. Open ByteHive while the standalone hub runs: the hub steps down, the next call
   goes through the app.
