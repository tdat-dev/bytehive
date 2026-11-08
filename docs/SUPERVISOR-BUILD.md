# Maestro Supervisor build (side-by-side edition)

A second Maestro that installs and runs **next to** the regular app without
touching it: same features, own name, own data, own ports. Built for the
Supervisor hackathon work; the regular release flow (`docs/RELEASING.md`)
is unchanged.

```powershell
npm run build:supervisor
# → dist-supervisor/maestro-supervisor.exe (+ browser-extension/ beside it)
```

No installer: NSIS/WiX are not installed here, so the build emits a portable
exe (`tauri build --no-bundle`) and renames it. Copy the whole
`dist-supervisor/` folder anywhere and run it.

## What keeps the two apart

| | Maestro | Maestro Supervisor |
|---|---|---|
| exe | `maestro.exe` | `maestro-supervisor.exe` |
| app id | `com.maestro.app` | `com.maestro.supervisor` |
| app data (settings, sessions) | identifier-based | separate, identifier-based |
| dashboard port | 8477 (+1 terminal WS) | 8577 (+1 terminal WS) |
| browser hub file | `~/.maestro/browser-hub.json` | `~/.maestro/browser-hub.supervisor.json` |
| auto-update | on (bytehive-releases feed) | on (own supervisor-releases feed) |

How it works:

- `tools/supervisor-config.mjs` deep-merges the base config with the small
  `src-tauri/tauri.supervisor.conf.json` overlay (name, binary, app id,
  updater endpoint) into a resolved config used for that build only, then
  deletes it: nested overrides work without depending on CLI merge
  semantics, and nothing drifts (version included).
- `VITE_EDITION=supervisor` bakes `src/edition.ts` into the Supervisor
  branch (window/tray/Settings name, no update checks);
  `VITE_DASH_PORT=8577` moves the dashboard.
- Rust detects the edition at runtime from the exe's own file name
  (`src-tauri/src/edition.rs`), so it stays right even though both editions
  share one `target/` dir: the browser hub file and tray strings follow it.
- The Supervisor edition never registers the Chrome native host
  (`owns_browser()` is false for it): the installed Maestro keeps talking
  to the extension, exactly like beside a dev build. Agents spawned from
  the Supervisor app still reach its own hub via `MAESTRO_HUB_FILE`.

## Auto-update

The Supervisor edition updates from its own feed,
`maestro-supervisor-releases/releases/latest/download/latest.json` (same
signing key as Maestro, separate endpoint baked via the resolved config),
so the two editions never cross-update.

- Cut a release by pushing a tag: `git tag supervisor-v0.10.11 && git push
  origin supervisor-v0.10.11`. Versions track Maestro's; tags carry the
  `supervisor-` prefix. The Supervisor Release workflow builds the signed
  installer + `latest.json` on windows-latest (which has NSIS/WiX) and
  publishes them.
- `RELEASES_TOKEN` must have Contents: read and write on
  `tdat-dev/maestro-supervisor-releases` too, not just
  `tdat-dev/bytehive-releases`, or publishing fails. It is a fine-grained
  token: add the repo under its repository access (or mint a new one and
  `gh secret set RELEASES_TOKEN`).
- Install the first Supervisor release manually once; updates after that
  are automatic. A portable local build also follows this feed: accepting
  an update installs the released edition next to it.

## Notes

- Run both apps at once freely; each keeps its own projects and sessions.
- Uninstall the portable build by deleting its folder (plus
  `%APPDATA%\com.maestro.supervisor` data if you want it fully gone).
