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
