# Phase 0 results: role launch on a real Claude agent

Date: 2026-11-05. Claude Code 2.1.293, Windows 11, probe folder in the session scratchpad (not in the repo).

Every `claude -p` call also carried `--settings '{"disableAllHooks":true}'`, because on this machine the
user hooks otherwise pop up terminal windows and slow the run. Nothing else was needed for permissions:
no `--permission-mode` or `--dangerously-skip-permissions` was used.

| # | Check | Command (shortened) | Result | Verdict |
|---|-------|---------------------|--------|---------|
| 1 | Prompt file is applied | `claude -p "What is the probe word? ..." --append-system-prompt-file probe/prompt.md` | Reply ended with `PROBE-ROLE` | PASS |
| 2 | Plugin dir is loaded | same call plus `--plugin-dir probe/plugin` | init lists plugin `probe-role` and skill `probe-role:brief-writer` | PASS |
| 3 | Skill is used when the task points at it | retry once: prompt "... Use the brief-writer skill if you have it." | Model called `Skill probe-role:brief-writer`, reply `The probe word is **HONEYCOMB**.` + `PROBE-ROLE` | PASS |
| 3b | Skill is auto-picked from a vague description | original prompt "What is the probe word? Then try to create a file named x.txt." | Model did NOT call the skill, answered `PROBE-ROLE` as the probe word, no HONEYCOMB | FAIL (not a flag bug, see notes) |
| 4 | `--disallowedTools "Edit,Write,MultiEdit,NotebookEdit"` holds | same call as 1 | No `x.txt` in the folder; the model said it has no Write tool. It then tried `PowerShell New-Item`, which was denied by the permission system (`permission_denials` has 2 entries) | PASS (see notes) |
| 5 | Resume keeps the role | `claude -p "Say hello." --resume 516c9063-... --append-system-prompt-file ... --plugin-dir ...` | Same session id returned, reply `Hello!\n\nPROBE-ROLE` | PASS |
| 6 | Same flags through `cmd //c` with a spaced path | `cmd //c claude -p "Say hello." --append-system-prompt-file "C:\Users\tvmar\.bytehive\probe prompt.md"` | Reply `Hello! What can I help you with today?\n\nPROBE-ROLE` | PASS |

Session ids: check 1 and 2 `516c9063-4bef-484a-bc84-f7abe39c67d1` (also used for resume);
check 3 retry `1befb844-bd31-47fc-9740-a40fdf2167c2`.

## Notes

- Skills from `--plugin-dir` register under a namespace: `probe-role:brief-writer` (plugin name + skill name).
  The plugin loads next to the user's own plugins; it is added, not exclusive.
- The skill is only invoked if the model decides to. A one-line vague `description` was not enough for the
  first, unprompted run. Task 4 should write skill descriptions that name when to use them, and the role
  prompt may need a sentence like "use your skills when they apply".
- `--disallowedTools` removed the file tools. Shell tools are NOT blocked by it: the model tried to create
  the file through PowerShell (denied here only because of this session's permission mode). A company
  member running with bypassPermissions could write a file through Bash/PowerShell. If a role must be
  truly read-only, `Bash` and `PowerShell` have to be in the disallowed list too.
- `--resume` keeps the same session id. The role text is not stored in the history, so the flags have to be
  passed again on every resume (this run did that).
- Quoting through `cmd /c` with a path that contains a space works with double quotes.
- The `rm` of the spaced-path copy (`C:\Users\tvmar\.bytehive\probe prompt.md`) worked and the file is gone.

## Decision for Task 4

- `--append-system-prompt-file` works (checks 1, 5, 6): use it. The fallback `--append-system-prompt`
  with stripped text is NOT needed.
- `--plugin-dir` works (checks 2, 3): skills are loaded as `<plugin>:<skill>`. The fallback of appending
  skills into the prompt file is NOT needed.
- `--disallowedTools` holds for the named tools (check 4) but does not stop shell writes. Add `Bash` and
  `PowerShell` for roles that must not write.
- Resume keeps the role only when the role flags are re-passed (check 5). `buildRoleLaunch` must be used
  for resume launches as well as fresh ones.
