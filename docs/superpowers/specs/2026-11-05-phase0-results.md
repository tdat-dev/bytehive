# Phase 0 results: role launch on a real Claude agent

Date: 2026-11-05. Claude Code 2.1.293, Windows 11, probe folder in the session scratchpad (not in the repo).

Every `claude -p` call also carried `--settings '{"disableAllHooks":true}'`, because on this machine the
user hooks otherwise pop up terminal windows and slow the run. Nothing else was needed for permissions:
no `--permission-mode` or `--dangerously-skip-permissions` was used.

| # | Check | Command (shortened) | Result | Verdict |
|---|-------|---------------------|--------|---------|
| 1 | Prompt file is applied | `claude -p "What is the probe word? ..." --append-system-prompt-file probe/prompt.md` | Reply ended with `PROBE-ROLE` | PASS |
| 2 | Plugin dir is loaded | stream-json retry (session `1befb844-...`), same flags as the first call | The init message lists plugin `probe-role` and skill `probe-role:brief-writer`. The first call (session `516c9063-...`) used json output and has no init listing | PASS |
| 3 | Skill is used when the task points at it | retry once: prompt "... Use the brief-writer skill if you have it." | Model called `Skill probe-role:brief-writer`, reply `The probe word is **HONEYCOMB**.` + `PROBE-ROLE` | PASS |
| 3b | Skill is auto-picked from a vague description | original prompt "What is the probe word? Then try to create a file named x.txt." | Model did NOT call the skill, answered `PROBE-ROLE` as the probe word, no HONEYCOMB | FAIL (not a flag bug, see notes) |
| 4 | `--disallowedTools "Edit,Write,MultiEdit,NotebookEdit"` holds | same call as 1 | No `x.txt` in the folder; the model said it has no Write tool. It then tried `PowerShell New-Item` twice; both appear in `permission_denials` (2 entries). Why they were denied was not established (the model itself said the block did not match the allowed folders) | PASS for the file tools only (see notes) |
| 5 | Resume works with the role flags re-passed | `claude -p "Say hello." --resume 516c9063-... --append-system-prompt-file ... --plugin-dir ...` | Same session id returned, reply `Hello!\n\nPROBE-ROLE`. All role flags were re-passed on the resume call; a resume without them was NOT run | PASS (flags re-passed only) |
| 6 | Only `--append-system-prompt-file` with a spaced path, run via `cmd //c` from Git Bash (not the app's spawn path) | `cmd //c claude -p "Say hello." --append-system-prompt-file "C:\Users\tvmar\.bytehive\probe prompt.md"` | Reply `Hello! What can I help you with today?\n\nPROBE-ROLE` | PASS |

Session ids: check 1 and 2 `516c9063-4bef-484a-bc84-f7abe39c67d1` (also used for resume);
check 3 retry `1befb844-bd31-47fc-9740-a40fdf2167c2`.

## Notes

- Skills from `--plugin-dir` register under a namespace: `probe-role:brief-writer` (plugin name + skill name).
  The plugin loads next to the user's own plugins; it is added, not exclusive.
- The skill is only invoked if the model decides to. A one-line vague `description` was not enough for the
  first, unprompted run. Task 4 should write skill descriptions that name when to use them, and the role
  prompt may need a sentence like "use your skills when they apply".
- Tested: with `--disallowedTools` the model reported having no Write tool and no `x.txt` was created. It
  then tried PowerShell `New-Item` twice and both attempts show up in `permission_denials`. The reason for
  those denials was not established.
- Inferred, not tested: that `--disallowedTools` does not cover Bash/PowerShell, that a member running with
  bypassPermissions could write a file through Bash/PowerShell, and that adding `Bash` and `PowerShell` to
  the disallowed list would stop it. None of these was run.
- `--resume` returned the same session id. Precaution, not a result: the role text is probably not kept in
  the history, so re-pass the flags on every resume. This run always re-passed them; a resume without the
  flags was never run.
- Tested: quoting a path with a space in double quotes worked for `--append-system-prompt-file` via
  `cmd //c` from Git Bash. The app's own spawn path was not exercised.
- The `rm` of the spaced-path copy (`C:\Users\tvmar\.bytehive\probe prompt.md`) worked and the file is gone.

## Decision for Task 4

- `--append-system-prompt-file` works (checks 1, 5, 6): use it. The fallback `--append-system-prompt`
  with stripped text is NOT needed.
- `--plugin-dir` works (checks 2, 3): skills are loaded as `<plugin>:<skill>`. The fallback of appending
  skills into the prompt file is NOT needed.
- `--disallowedTools` removed the named file tools (check 4, tested). Inferred, not tested: shell tools are
  not covered, so a role that must not write may need `Bash` and `PowerShell` in the list too; verify that
  before relying on it.
- Resume worked with the role flags re-passed (check 5). Precaution, not a result: use `buildRoleLaunch`
  for resume launches as well as fresh ones. Resume without the flags was not tested.

# Phase 0 results: WebGL next to terminals in WebView2

Date: 2026-11-05. Windows 11 Pro 10.0.26200, WebView2 runtime 156.0.4314.58, three 0.170.0. Canvas renderer
reported by the page: ANGLE, Intel UHD Graphics 630, Direct3D11 (the machine also has an NVIDIA RTX 2070
Max-Q; the spike asks for `powerPreference: "low-power"`). 12 logical cores. App: `npm run tauri:dev`
(ByteHive Dev, Vite dev server, debug Rust build), driven over WebView2 CDP on port 9333.

Setup: a 45vw x 60vh three.js canvas (5 floors, 30 desks, 30 capsules, one hemisphere light, antialias) fixed
over the app at z-index 40 and `pointer-events: none`, rendering at about 60 fps while "moving". 12 PowerShell
shell agents (6 in each of two scratch folders), started through the real Home composer. Frame gaps are
measured with `requestAnimationFrame` inside the page. All clicks were real mouse events through CDP, window
resizes were native `SetWindowPos` calls on the dev window, and `visibilityState` was `visible` with the
window not minimized throughout; a separate rAF counter ran at about 60 per second per loop.

| # | Gate | Result | Verdict |
|---|------|--------|---------|
| 1 | Moving scene, 10 New agent modal open/close, 20 project switches (10 round trips, alpha and beta), 10 window resizes: `over200` must be 0 | Two valid runs: `over200` 0 and 0, `maxGap` 100 ms and 100 ms, `longMax` 105 ms and 94 ms, `lost` 0. An earlier run had 1 gap of 266 ms during the modal phase (right after spawning the 12 shells); its resize phase was invalid (script bug gave the window a 65535 px height) | PASS in 2 of 3 runs (see notes) |
| 2 | Scene still, 1 hour idle: `lost` 0, or restored within 2 s | 19:59 to 20:59 local, sampled every minute: `lost` 0, `restored` 0, `isContextLost()` false at every sample and at the end, `over200` 0, `maxGap` 99 ms. Screenshot at the end shows the scene drawn | PASS |
| 3 | Scene still, CPU of the app process tree for 1 minute: under 5% | 12.5 CPU-s in 60 s over 31 processes = 20.8% of one core = 1.74% of all 12 cores (the pass rests on dividing by all 12 cores; as a share of one core it is 20.8%, which is over 5%) (WebView2 16.8% of a core, bytehive-dev 3.6%, shells 0.4%). Control with the canvas hidden: 1.90% of all cores | PASS, on the all-cores figure |

No fallback (OffscreenCanvas in a Worker, second Tauri window) was needed.

## Notes

- Gate 1 passed in 2 of 3 runs. The first run had one 266 ms gap in the modal phase, right after spawning the 12
  shells; the cause was not measured. A user opening the New agent modal right after spawning agents is a normal
  case, so re-measure it in Phase 2.
- The earlier freeze (src/terminal.ts:124) was triggered by the glow when toggling a crew card in the spawn modal
  while xterm WebGL canvases were present. This test never toggled that glow and the terminals used the DOM
  renderer, not xterm's WebGL addon, so that trigger is untested, not ruled out.
- Gate 3: hiding the canvas costs the same CPU (1.9% vs 1.74%), so the scene adds about nothing when still. The
  1.7 to 1.9% is the app itself idle.
- Not covered: the second half of gate 2 (context restored within 2 s) never ran, because no context was lost
  in the hour, so how fast a lost context comes back is unknown; the crew card glow trigger above; a moving scene's CPU/GPU load was not measured, only its frame gaps; the canvas drawing buffer is
  sized once, so window resizes stretched it and did not reallocate it; the shells were idle and only one pane is
  visible at a time; no sleep/wake, display-off or GPU switch during the hour; no streaming agent output.
- The window lost foreground focus at about minute 20 (someone used the machine) but stayed visible and rAF
  kept running, so the hour still counts.

Bottom line: viable on this machine; re-measure with a resizing renderer and a streaming fleet in Phase 2.
