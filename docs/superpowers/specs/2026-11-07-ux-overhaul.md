# ByteHive UX overhaul: audit and decisions

Date: 2026-11-07. Code audited: `feat/office-3d` at `4b6a504` in `D:\bytehive-worktrees\office`, read only. Status: decided, no product code changed. The owner said do not ask, so every call below is made, with its reason.

Route: `DASHBOARD_PRODUCT`, work mode `AUDIT`, archetype AI agent (runs and approvals) with work management and devtools as secondaries. Methods: `build-premium-website` (render-check, slop audit, human-factors lines, heuristic pass) and `impeccable` (audit, critique, detector, independent reviewer).

## The decision in one page

ByteHive's parts are good and its whole is not. Under 28 surfaces and 6 stacked chrome systems sits a simple product: one tree of Home, Project, Agent. The office already draws that tree, and the list should be the same tree. So:

1. **One frame.** A title bar and a rail on every screen, and a main area that shows one level. Inbox, Grid, Fleet, the dock and the office's separate layer stop being places.
2. **Honey means it is your turn.** The office's dusk behind every screen, honey only on what waits for you. That is the cure for boring, and it makes the app and the office one product.
3. **The terminal belongs to the CLI, truly.** Today it does not. Keys work in the app only when the app has focus, and one OS-level key crosses over.
4. **Trust first.** Fix three wrong truths before any polish: stolen keys, two counts of who needs you, and a "Ready to review" state that default setups never reach.
5. **Spend the room on the work.** Terminal from 52% of the window to about 69%, Review from a drawer to a full tab.

Order: fix keys and truth and lay the new look inside the Task 11 pass, release 0.15.0, delete what is dying (Phase 4 moves up), then build the frame, then Phase 3's character editor on top of it.

Evidence levels used: `MEASURED`, `OBSERVED`, `SIMULATED`, `ASSUMED`, as in the skill. Section 9.3 says which is which.

## 1. The core jobs

Who and where: one developer at a Windows desk, often late, running 3 to 20 CLI agents over 2 to 5 repos, mostly in another window until something needs them. The job they hired ByteHive for: keep many agents moving without babysitting any of them. What they already know: terminals, git, their CLIs' keys. What makes them quit: an app that gets between them and the CLI, or that they cannot trust about who is waiting.

Ranked by how often it happens and how much it costs when it goes wrong.

| # | Job | Steps today (`SIMULATED`) | Target | Rule |
|---|---|---|---|---|
| J1 | **Answer an agent that is waiting** | 2 to 3: find the row, open it, answer in its terminal. Only the Grid answers in 1, and only for cards. Start's "Answer" button does not answer: it navigates | **1 step, from any screen** | Must be one step |
| J2 | **Know what needs me and what is going on** | 0 in Inbox. From Office, Start, Board or Settings the signal differs or is hidden | **0 steps, everywhere** | Always on screen |
| J3 | **Give work** (one agent, a race, or the Founder) | Start: 1. From Inbox: open a modal, type, submit, in a different form from Start's | **1 step**: type and Enter | Must be one step |
| J4 | **Review and land a finished job** | Open drawer, read, tick, merge. Fine in count, poor in room: about 130px of diff at 1280x720 | **1 step after reading** | Merge is one key |
| J5 | **Work in a terminal** | 1 to reach it, but the app still takes keys while you type | **1 to reach, then nothing in the way** | The terminal is the CLI's |
| J6 | **Recover** a stopped or out-of-usage agent | 1 from the row, or `Resume all` far from the list | **1 step** | Must be one step |
| J7 | **Look back**: what did it do, what did I say | 2. History dies with the app. Sessions is a separate modal | 2 steps, and it survives a restart | |
| J8 | **Plan** the work on the Board | Hidden: no button, `Ctrl+Shift+K` or the palette | 2 steps, visibly | |
| J9 | **Set up**: settings, schedule, remote, browser | Modal inside modal | 2 to 3 steps | |

Everything on screen must serve one of these. Anything serving none is cut in section 3.

## 2. Audit

### 2.1 The count

All `MEASURED` on `feat/office-3d` at `4b6a504`, in headless Chrome with a fake backend and 9 agents over 3 projects, or from the source.

| What | Count |
|---|---|
| Surfaces | **28**: 7 places (Start, Focus, Grid, Building, Floor, Desk, fallback column), 8 side sheets (Board, Changes, Fleet, Flow, Pomodoro, Review, History, Explorer), 13 dialogs (New agent, Settings, Confirm, Palette, Help, Sessions, Memory, Compare, Scheduled, Usage, Replays, Board card, Browser view) |
| Chrome systems stacked on one screen | **6**: sentence bar, left list, bottom dock, side sheets, modals, office layer with its own bar |
| App shortcuts | **26**: 15 with Ctrl, 8 with Alt, 3 bare. 2 are dead (`Ctrl+Shift+B`, `Alt+1..9` in the Inbox) and are still taken from the terminal |
| Ways to see an agent's changes | **10** entries into **2** different screens, both called Changes |
| Ways to start an agent | **12** |
| Ways to go to an agent | **10** |
| Places that list all agents | **6**: Queue, Fleet, Grid, palette, office, remote dashboard |
| State vocabularies | **3**: Task (`needs review working idle stopped merged`), Fleet (`needs active idle stopped`), pane bar (`run idle`) |
| Things `✕` means | **4**: remove agent, take out of Grid, close panel, close dialog |
| Words for one thing | agent, person, member; project, workspace, folder, floor; Inbox, Queue, list, Home, Start; Focus, Stage; Grid, Split; Changes, Review, Diff |
| Settings rows | **30** in 7 sections, plus 3 modals opened from inside Settings |
| Tools with no button on the main screens | **8** of 8 on the Help page's own Tools grid, Board among them (3 have one inside Settings). The rail buttons `dock.ts` looks for are gone from `index.html`. The page's subtitle reads "The things without a button of their own" |
| CSS | 230 KB in 22 files, 885 classes, 66 hex colours, 133 rgba, 28 font sizes, 28 radii, 97 box-shadows, 31 z-index values, 48 durations, 32 keyframes, 33 `!important`, 44 button classes |
| Text under 12px | 85 of 334 `font-size` declarations. 145 are fractional |
| Main JS chunk | 1,683 KB raw, 566 KB gzip, parsed on every start, with CodeMirror and 8 languages in it |
| `setInterval` timers | 18 |
| Terminal share of the window | **52%** at 1440x900 (1030x650, 25 rows). Sentence bar 68px, dock zone 84px, agent header 84px |
| Diff height at 1280x720 | about **130px** |

Mechanical checks, quoted:

```
RENDER-CHECK 1.4.1 (Inbox, 1440): overflow_desktop=0 overflow_mobile=0 js_errors=0 contrast_failures=0 word_stacking=1 text_collisions=13 clipped_content=1 dead_controls=0 assert_failures=0 nav_hidden_desktop=1 progress_empty=0 blank_page=0 hidden_without_js=1 broken_images=0 | a11y: small_targets=4 unnamed_controls=0 placeholder_labels=0 img_missing_alt=0 doc_basics=0 focus_stops=30 focus_invisible=1 focus_obscured=0 focus_hidden=1 keyboard_unreachable=0 | slop: scroll_animated=0 nested_boxes=0 emoji_icons=0 warm_neutral_body=0
audit-ai-slop 1.1.0 on src/styles + index.html: BLOCKER_CANDIDATE 82, ADVISORY 19
impeccable detect on index.html + src/styles: 59 findings
```

Dispositions. render-check: `text_collisions` and `word_stacking` come from truncated rows and the row time, real but small, fixed by the new row. `small_targets` is the 15x28 Hide code panel button, plus dock buttons at 390 wide, which this app does not target. `focus_invisible` and `focus_hidden` are the Explorer tree and its filter while the panel is closed: a real keyboard bug, closed panels stay in the Tab order. The slop audit's 82 blocker candidates are nearly all `RUNTIME_HIDDEN_CONTENT`, hover-revealed controls and the splash at `opacity:0`; hover-only `.ctrls` is real, the rest are false positives, and the new row shows its menu on focus as well. The detector's 51 `low-contrast` are `#000` on `#050506` from the first-paint `<style>` and SVG, false positives, since no visible text is black; render-check's `contrast_failures=0` agrees for what was on screen. Real contrast failures are the two tokens named in 5.1. The 19 advisories are mostly the long dash inside CSS comments, not visible copy. The detector's other 8 are real and small: `transition: width` in 4 files, gradient text and an overshoot curve in the splash, a 3px side stripe in the editor, and the old canvas grid.

The code is better than the look: `dead_controls=0`, `unnamed_controls=0`, `keyboard_unreachable=0`, real labels and live regions. That work should be kept.

### 2.2 Scores

Impeccable technical audit:

| Dimension | Score | Key finding |
|---|---|---|
| Accessibility | 3 | Names and roles are good. Closed panels in the Tab order, 85 sub-12px sizes, splash ignores reduced motion |
| Performance | 2 | 1.68 MB eager chunk, 18 timers, whole-list `innerHTML`, 10 blurs. Memory is fine so far (section 8) |
| Responsive | 2 | Works to 760 but nothing folds: text cuts, the header wraps, Files reflows the stage |
| Theming | 1 | Tokens exist but 66 hex and 133 rgba bypass them, and `--ib-*` duplicates the set |
| Implementation integrity | 2 | Three state models, two diff tools, dead keys, stale Help and README |
| **Total** | **10/20** | Acceptable, significant work needed |


Independent design review, `Method: dual-agent`: a separate agent saw only the 30 screenshots and no notes, and the detector run stayed out of its context. Nielsen, 0 to 4:

| # | Heuristic | Score | Key issue |
|---|---|---|---|
| 1 | Visibility of status | 2 | The headline is excellent, but counts and states conflict between screens |
| 2 | Match with the real world | 2 | Plain copy, then Conductor, Director, Fleet, Flow, "Company item" |
| 3 | User control | 2 | `✕` on an agent is ambiguous, an empty Send back cancels a job |
| 4 | Consistency | 1 | Pills and rectangles, five panel widths, three close styles, two review surfaces, 12h and 24h clocks |
| 5 | Error prevention | 1 | Merge is the primary button at 0 of 6 viewed, on one bare key |
| 6 | Recognition over recall | 1 | Eight tools have no button |
| 7 | Flexibility | 3 | Palette and shortcuts are strong |
| 8 | Aesthetic, minimalist | 2 | Calm, but statuses repeat and overlays pile on the same stage |
| 9 | Error recovery | 2 | Plain messages with no reason or next step |
| 10 | Help | 3 | Useful, though some claims are false |
| | **Total** | **19/40** | Poor band: a real overhaul, not a polish |

Busiest decision point: Review, about 43 targets on screen, 3 real decisions. Cognitive load checklist: 6 of 8 failed (single focus, hierarchy, one thing at a time, minimal choices, working memory, progressive disclosure).

Design specificity: the reviewer, told to assume no designer, called three elements interchangeable with any product: Start (greeting, chips, three numbered cards, "only the logo says ByteHive"), the Inbox shell ("a default dark dashboard kit"), and the Help grid ("a marketing features block used as navigation"). On the office: "the only screen with personality", and "a game prototype bolted onto a dashboard". Its advice, taken here: do not make the office more like the Inbox; let its warmth flow back.

Overall gate: `DASHBOARD_PRODUCT GATE: NOT MET` (conditions 1, 6, 7, 9). `OVERALL STATUS: BLOCKED` by the three P0s below. The manual accessibility script was not run: there is no point before the frame changes. It runs at the end of E.

### 2.3 The problems, ranked by damage

**P1. The terminal is not the CLI's. P0.** Screen: Focus, every agent. Heuristic: user control; the owner's own rule. The owner ruled in v0.12.13 that every key belongs to the CLI, and v0.13.0's return to the web app lost that (its changelog says so). `MEASURED`, terminal focused, real key events:

| Key | Reaches the terminal | App also does |
|---|---|---|
| `Ctrl+K` | No | Opens palette |
| `Ctrl+,` | No | Opens Settings |
| `Ctrl+Tab` | No | Switches project |
| `Ctrl+Shift+K` `J` `D` | No | Opens a dock panel |
| `Alt+S` | **Yes** | Toggles Grid |
| `Alt+R` | **Yes** | Opens Review, takes focus |
| `Alt+P` | **Yes** | Pins to Grid |
| `Alt+O` | **Yes** | Opens Sessions |
| `Alt+↑` | **Yes** | Moves to another agent |
| `a` (control) | Yes | Nothing |

The Alt rows fire twice: `inbox.ts:939` calls `preventDefault` in a capture listener but xterm still acts on the key. Claude Code's own docs list `Ctrl+K` as delete to end of line and `Alt+P` as switch model, so today Alt+P changes the model *and* pins the agent. `Ctrl+Shift+B` and `Alt+1..9` are taken from the CLI and do nothing at all (`terminal.ts:279-286`). Fix: 7.2, package A.

**P2. Two truths about who needs you. P0.** Screen: Fleet against the Inbox. Heuristic: visibility of status. `MEASURED`, same moment: the headline says "2 agents need you" (Ana, Dao). Fleet says "3 need you" (Ana, Eli, Hai) and calls Dao idle. Fleet classifies with `paneStatus` and the old "went quiet" guess that `taskstate.ts` was written to replace. A supervisor's tool that lies about this cannot be trusted anywhere. Fix: one model, Fleet gone.

**P3. "Ready to review" often does not mean what it says. P0.** Screen: rail, Home, Review. Heuristic: visibility of status. Read from source, and seen in a real run:

- `changesFor` only looks at agents with a worktree (`tasks.ts`), and own branch has been off by default since the move to the project folder. So by default **no agent ever reaches Ready to review**, and Start still promises "Each agent gets its own git branch".
- For agents with a worktree the count is `git status` against HEAD plus untracked files (`review.rs:176`). Work the agent committed drops out of the count, which is what Company people do, and untracked junk goes in. `task-9-19` shows `+613 −0` beside a commit of one line; Ana's own report names untracked `.claude/` and `logs/`.
- Review shows the branch diff, a different number from the row. Note: the fixture in the audit shots exaggerates this (rail +148, drawer +30), but the two sources differ in the code regardless.

Fix: one function for "what this agent changed", used by the row, the state and Changes.

**P4. There is no map.** Screens: all. Heuristics: consistency, recognition. 28 surfaces, 6 stacked chrome systems, no place named on screen outside the office. Home is a screen you leave and come back to with a pill; the office is a layer that covers the app and has an Inbox button to leave it; Board is a sheet over the terminal with no button. A person cannot say where they are, so they cannot learn where anything is. Fix: section 4.

**P5. The core job has three behaviours.** Screens: Focus, Grid, Start, office. Heuristic: consistency. In Focus you answer in the terminal, in Grid cards answer in one click, Start's `Answer` only navigates, and the office's `Approve` is a 60px outline chip beside its equal `Send back`. The only action a rail row offers is Resume, the least urgent one. Focus can open on a working agent while two others wait (`audit-03`). Fix: 4.3 and 6.4.

**P6. The work gets the leftovers.** Screens: Focus, Review. Heuristic: aesthetic and minimalist design. The terminal gets 52% of the window while a status sentence takes 68px and a floating dock 84px. Review is a 600px drawer over a terminal nobody needs at that moment: at 1280x720 it shows about 130px of diff, five lines, under a file list that takes more room than the code. Fix: frame and Changes tab.

**P7. It looks like nothing.** Screens: all but the office. This is what the owner means by boring. `--accent` is `#f4f4f5`: the app has no colour of its own. Primary buttons are white pills, selection is grey, honey lives only in the logo and near-amber state. Every container is a rounded near-black panel with a hairline border, so nothing is more important than anything else. The office beside it is warm, lit and alive, with different buttons, nouns and panel placement. Two products. Fix: section 5.

**P8. Things you need have no door.** Screens: all. Heuristic: recognition. All 8 tools on the Help page have no button on a main screen, Company included. `Ctrl+Shift+O` is the only way to the headline feature for someone who never used it. Fix: tabs, title bar, rail.

**P9. Merge is too easy, Cancel too hidden.** Screens: Review, rail. Heuristic: error prevention. Merge is the loudest control with nothing viewed, Discard sits in the same row, and `M` is one stray press. In the rail, an empty note on Send back cancels a job; the warning is a placeholder. "Review and merge" rows offer only `Mark done`, with no way to merge on that screen. Fix: 6.7 and 7.3.

**P10. Same name, different thing.** Heuristic: consistency. Two tools called Changes with different scope and actions; at a Desk two Changes controls sit on one screen. 10 ways in. `✕` has 4 meanings. Clocks mix 6:17 PM and 18:21, and durations 0:36 and 35s. "Ask before running" in Settings is "Don't ask before running" in New agent. Fix: 7.1 and the glossary.

**P11. Words that stopped being true.** Heuristic: match, help. Help has a Chat section for a chat removed in v0.9.9, and a "Book" keycap. Start says a waiting agent "moves to the top of the list": the list has been fixed order since. Flow's empty state names a Director. README sells a workspace wizard, broadcast, tab drag and a mascot, all gone.

**P12. Slow front door.** Screen: launch. Heuristic: visibility of status. 2.1s of splash on a timer at every start, ignoring reduced motion on purpose, then 1.68 MB of JS, fonts from Google over the network. First run asks you to read three cards. Fix: H and J.

**P13. Memory that isn't.** Screen: History, Flow. History reads "Kept while ByteHive is open"; Flow keeps 200 messages in RAM. After a restart, "what happened while I was away" has no answer, which is the question both exist for.

**P14. Small windows and overlays.** Screens: 760, Files, Board. Header wraps when Files opens, the sentence cuts off, Board clips Done, the limit warning sits on the composer's border, a toast covers the limits (`task-9-19`), and autofocus makes Close the loudest thing in every drawer.

Also seen in a real run, worth checking in package A: at the Founder's Desk the state said Working while the terminal said done and waiting for approval (`task-9-06`), and "Still working" showed in Changes. Jobs said "No job is running" while Ana was Working (`task-9-19`).

### 2.4 What works, and stays

- The sentence: "2 agents need you. 2 are ready to review, 2 are working." It is the best line in the product.
- The palette, grouped by state, with keys shown.
- The plain voice: "Its branch and the changes on it stay in git", "Claude has used 91% of its limit. Codex has room." and the sentence under each setting.
- Fixed row order. A hard-won call that most agent tools get wrong.
- Comments on diff lines that go back to the agent, and Viewed.
- The office's light.
- Under the skin: labelled controls, focus handling, `Calm` drawing less when unfocused, scrollback trimmed for hidden terminals, three.js lazy loaded, Pointer Events for drag.

Persona notes. Power user: fast once in a terminal, no visible "next that needs me", and the keys he learned fight his CLI. First-timer: must notice an amber row, understand the answer happens in a terminal box, and later find a small chip to review, then meet two panels called different things.

## 3. Keep, merge, cut

The test for each thing: which ranked job does it serve, does another surface already serve that job, and what breaks if it goes. KEEP means it stays and may move. MERGE means the job survives inside something else and the surface goes. CUT means the job goes.

| Surface or feature | Verdict | Where it goes, and why |
|---|---|---|
| Real terminals | **KEEP** | The product. It gets more room and gets its keys back (J5) |
| Agent list (Queue) | **KEEP** | Becomes the Rail, on every screen. Fixed order stays: the owner made nothing jump on a state change, and that was right |
| Review | **KEEP**, promoted | From a 600px drawer to the Changes tab, full width. The most valuable screen in the app after the terminal has the worst room |
| History | **KEEP**, promoted | Agent tab, and it stops being lost: "Kept while ByteHive is open" is not a history. Written to the project's `.bytehive/` |
| Sessions | **KEEP** | Project tab. Add "Continue in a new agent" later: `pane.restart({session})` already exists |
| Office | **KEEP** | The main area at the Home and Project levels. It stops being a layer that covers the app |
| Company (Founder, Lead, Jobs, plan) | **KEEP** | Same rail, same composer, same tray |
| Board | **KEEP**, promoted | Project tab, full width. Today it has no visible button and its fourth list is cut off at 1440 |
| Own branch per agent | **KEEP**, made honest | Off by default since the owner moved agents into the project folder, and that stands. But Start still promises "Each agent gets its own git branch". It becomes a visible chip on the composer, so you choose per job and the promise is true when shown |
| Files and editor | **KEEP** | Project tab, and a right split in Agent. Reading a file beside the terminal is real work |
| Project memory | **KEEP** | Project tab. The list and "Ask an agent to check" are good |
| Command palette | **KEEP** | It is the best screen in the app today. Moves to the title bar, gains Recent, keeps its grouping by state |
| Hand over, limit guard, plan meter | **KEEP** | Unique to this product and truly useful. Meter stays at the rail foot, one line, opens Usage |
| Browser live view and Ask first | **KEEP** | Its asks join the same tray as CLI asks: one place for "wants to do X" |
| Remote dashboard | **KEEP** | Settings. Spec for `remote.zoldify.com` is separate |
| Scheduled agents, presets | **KEEP** | Settings › Automation, a page, not a modal opened from a modal. Plan decision D15 stands |
| Race and Compare | **KEEP**, merged in | Stays a feature, leaves the modal: a switcher at the top of Changes (`Ana · Bo · Cid`). Compare exists to choose a diff, so it belongs where diffs are |
| New agent modal | **MERGE** into composer | Start and New agent are one form built twice (`inboxstart.ts`, `inboxnew.ts`) with 2 copies of the CLI logic. One composer, shown in place on Home and as a popover from the rail |
| Start screen | **MERGE** into Home | Home without a Company. The box and the waiting list stay, the greeting stays as a small line, the logo gets out of the way |
| Grid | **MERGE** | The owner asked for many agents at a glance, and tiles were already dropped because terminals read only at about 80 columns (v0.9.13). What is left is one terminal plus cards. Cards go to Project › Agents and to Home, answers go to the tray. The cost is a whole mode: `layoutSplit`, `borrowed`, pins, `splitClosed`, `widenGrid`, `gridfit.ts`, two shortcuts and a Settings row. Side-by-side reading of two agents is the one thing lost; if the owner misses it, add a plain Split right to the Agent view, not a mode |
| Fleet | **CUT** | The rail is the Fleet. It also tells a different truth (see P2). `paneStatus` goes with it. `fleet_status` and `fleet_send` for agents stay |
| Flow | **MERGE** into History | A hand-off is an event in two agents' histories, and the office draws it as a paper plane. An empty side panel that mentions a Director and forgets all on restart has not earned a shortcut |
| Changes dock (`diffview.ts`) | **MERGE** into Review | Two tools called Changes. Commit and Discard for agents without a branch move to the decision bar |
| Dock (bottom bar) | **CUT** | 84px of terminal height to hold seven things that all have better homes. It is a floating-pill style with no job of its own |
| Dock panel (side sheet host) | **CUT** | Every tool that was in it is now a tab. No more sheet that covers the terminal and the header |
| Pomodoro | **CUT** | 279 lines that serve no ranked job. A timer has no business beside agents that decide their own time, and Windows has one in Clock. It held `Ctrl+Shift+J` over the terminal |
| Tips (`tips.ts`, 2 Settings rows) | **CUT** | Empty states and the palette teach. A note that shows once and is gone is the weakest form of help |
| Help page | **MERGE** | `?` shows keys for the current screen. The Tools grid goes: a list of things without a button is a confession, and now they have buttons |
| Sentence in the top bar | **MERGE** | It stays big on Home, which is what it is good at. Elsewhere it is a pill in the title bar. It should not take 68px over a terminal |
| Resume all | **KEEP**, moved | One line at the rail foot when more than one agent is stopped |
| Usage modal | **MERGE** | Settings › Usage, opened from the meter |
| Replays | **MERGE** | A filter inside Sessions; the player stays. Own modal goes. Recording stays in the `⋯` menu |
| Compare modal | **MERGE** | See Race |
| Splash | **CUT** | 2.1s of black on every start, by timer, with reduced motion ignored on purpose. The logo motion can live on the website. A first paint shows the real app |
| Review chip ("Cid is ready") | **CUT** | Replaced by the Changes tab badge |
| Conductor modes, Director | **CUT**, sooner | Phase 4's Task 18, moved up (section 9). Their controls are on the screens this overhaul rebuilds |
| Canvas background picker | **CUT** | `background.ts` is 16.8 KB and its `.bg-opt` grid is not in `index.html`, so nothing can open it. Confirm at removal |
| Detached windows | **KEEP**, unchanged | Costs nothing here. Their tab drag is gone, so check the path still works, and cut it if it does not |
| Broadcast | **CUT** | Already gone from the UI. `Ctrl+Shift+B` still gets swallowed from the terminal and README still sells it |
| Sprites, mascot assets | **CUT** | Task 19: 4.8 MB in `src/assets`, nothing reads them |

Net result, counted the same way as section 2: 28 surfaces become 19, of which 13 dialogs become 5 and 8 side sheets become 3, and all of them live in one frame.

Three calls the owner may want to overrule, stated so he can:

1. **Grid.** He asked for it, shaped it and it took many rounds. The argument is above, and if he disagrees it survives as Project › Agents plus a Split right, not as a mode.
2. **The top sentence.** He liked it in the Cinema look. It stays as the Home headline.
3. **The Inbox name.** It goes from the UI. The rail is the inbox; a separate "Inbox" word is what forced the office to need a button to leave.

## 4. New information architecture

### 4.1 The model

ByteHive has one tree, and the office already drew it:

```
Home (the building)        everything you run
└─ Project (a floor)       one folder
   └─ Agent (a desk)       one CLI, one branch, one terminal
```

Everything in the app sits on exactly one of these three levels. The list and the office are two pictures of the same tree, not two products. That single sentence replaces the current mental load of Start, Inbox, Focus, Grid, Fleet, Office, Floor and Desk.

| Level | What it answers | Tabs | With a Company it looks like |
|---|---|---|---|
| Home | What needs me, what is going on, what next | none | The building |
| Project | How is this folder doing | Agents, Board, Files, Memory, Sessions | The floor, in the Agents tab |
| Agent | What is it doing, what did it change | Terminal, Changes, History | No scene. The terminal wins |

### 4.2 The frame

The frame never changes. Only the main area does.

```
┌───────────────────────────────────────────────────────────────────────────┐
│ ⬢ ByteHive › gcare › Dao     [ Jump or run… Ctrl K ]   ⬢ 2 need you   _ □ ✕ │ 48
├──────────────┬────────────────────────────────────────────────────────────┤
│ + New agent  │ Dao  Write the pricing page copy    Needs you               │ 44
│              │ [Terminal] [Changes 6] [History]             Files   ⋯      │
│ Founder      ├────────────────────────────────────────────────────────────┤
│ bytehive   3 │                                                            │
│  ⬢ Fix flaky │                                                            │
│  ◌ Add retry │                real terminal, edge to edge                 │
│  ◎ Split css │                                                            │
│ gcare      3 │                                                            │
│  ⬢ Pricing ◂ │                                                            │
│  ◎ Invoices  │                                                            │
│              │                                                            │
│ ┌ tray ────┐ │                                                            │
│ │ ⬢ Ana    │ │                                                            │
│ │ npm test │ │                                                            │
│ │ Allow Deny│ │                                                           │
│ └──────────┘ │                                                            │
│ Claude 91%  ⚙│                                                            │
└──────────────┴────────────────────────────────────────────────────────────┘
```

- **Title bar, 48px**, Windows' height for a bar with a search box, all empty space draggable. The logo is Home, in place of the house icon. Breadcrumb says where you are and each part is a button. The palette field is in the middle, always findable, for people who do not know the key. The status pill is the app's one headline: click it to go to the next thing waiting. It works the same over the office and over a terminal.
- **Rail, 288px, resizable, on every screen, including the office.** Top: New agent. Middle: the tree, projects as headers, agents as rows, in the fixed order the owner chose. With a Company: the Founder first, and people show their role. Bottom: the tray and the meter.
- **Main area.** One of the three levels. Nothing floats over it except a menu, a dialog, or a toast.

At 1440x900 the terminal goes from 1030x650, 52% of the window, to about 1136x790, 69%. That is 45 rows at 14px against 25 today.

Under 1000px wide the rail folds to 56px of marks with state glyphs; hover or `J` shows a label. Under 760 the rail becomes a sheet. Today nothing folds and the sentence just cuts off.

### 4.3 Why the tray sits at the bottom of the rail

The obvious place for Needs you is the top of the list. It is wrong here. Items arrive at random times, and anything inserted above the tree pushes rows down under a moving mouse, breaking the owner's rule that nothing jumps. So the tree stays still and the tray grows upward from the foot, shrinking the tree's scroll area instead of moving it. The honey pill in the title bar covers the top of the eye path, and honey is the only saturated thing on the screen, so position is not what finds it.

The tray shows only agents you are not looking at. The agent on stage asks in its own terminal and you answer there: the owner removed the card over the terminal twice (v0.10.4 and v0.11.9), and this keeps to that. What he kept, and said so in the changelog, is "Grid cards of the other agents still answer with a click". The tray is that, on every screen.

### 4.4 How the pieces fit one mental model

- **Inbox:** no longer a place. Its list is the rail, its sentence is the pill and the Home headline. The word goes.
- **Terminals:** the Agent level, tab 1. One terminal in the main area at a time.
- **Board:** the Project level. A card's assignee links to the Agent. An agent's row shows the card it works on.
- **Review:** the Agent level, tab 2. Nothing else is called Changes. Project-level diff does not exist: changes always belong to an agent, or to the folder's own row.
- **Office:** the picture of Home and Project. Click a person: the camera moves for 333ms and you are at Agent, same rail, same title bar. There is no Inbox button, because you never left. A List or Office switch sits at the top right of the main area at Home and on the Agents tab, remembered.
- **Without WebGL or with a failed scene:** the switch reads List, and says why once. No fourth column, no second layout.

### 4.5 Home

**With a Company, once used, and WebGL fine:** the building fills the main area. The composer sits bottom left with the target set to Founder, example chips above it, floor signboards on the building. Rail shows Founder, Jobs as one line each, the tree, the tray. A plan to approve opens as a sheet over the right of the scene, written as a proposal.

**Without a Company:** a working page, not a landing page.

```
2 agents need you.  2 are ready to review, 2 are working.         (28px, the one sentence)
[ What should an agent do?                                     ]
[ bytehive ▾ ] [ Claude Code ▾ ] [ 1 agent ▾ ]         [ Start ⏎ ]
Needs you         the same cards as the tray, wider, with the answer buttons
To review         rows: job, agent, +148 -455, Review button
Working           rows: job, agent, what it is doing now
Projects          one row each: name, path, counts. Open a folder at the end
```

The first section with anything in it is what you see first, so a quiet Home reads differently from a busy one by shape alone. No project cards in a three-column row, no starter chips once you have run anything.

### 4.6 First run and empty states

First run asks one thing. The composer shows with the folder chip lit, the only honey on the screen, reading `Choose a folder`. One line under it: "Each agent gets its own git branch, so your checkout stays clean." The three cards go: they are a parallel-column filler (craft ban 5), and step three explains something that has not happened. Starter chips stay here only, as they fill the box with a real job. A missing CLI is said in the chip: `Codex (not installed)`. The Founder's job box is not shown until the first agent has run: the Company needs a project, Node and Claude, and a first-time person needs none of that yet. It stays reachable from the palette.

| Where | Empty state |
|---|---|
| Home, nothing running | "Nothing is running." and the composer. No illustration |
| Tray | Not there. No empty tray |
| Pill | `All clear`, in `--ink-3` |
| Project, no agents | "No agents in gcare yet." and the composer with the project set |
| Board | "No cards yet. Agents add cards here as they plan." and Add a card |
| Changes, working | "Still working. The diff lands here when Dao stops." Good copy, keep it |
| Changes, nothing | "No changes on this branch." |
| History | "Nothing yet. What Dao does and what you answer shows up here." |
| Memory | Keep today's placeholder as the sentence |
| Sessions | "No Claude conversations in this folder yet." |
| Office, no people | The empty floor at night, and the composer. It must look like a room, as the addendum says |
| Palette, no match | Keep: `Nothing matches "x".`, and offer "Start an agent with this as the job" |

## 5. Design language

One idea carries the whole system: **honey means it is your turn.** The office already works like this: warm honey rooms in a blue dusk, and the eye goes to the light. The app today is black, white and grey with a small amber accent. The new system puts the office's dusk behind every screen and spends honey only on what waits for you. Then a screenshot of any screen tells you, from across the room, whether you have something to do.

Colour strategy (craft rule): `RESTRAINED`, one accent under 10% of the surface. Provenance: the ground is `OFFICE.sky #191d27` from `palette.ts`, the honey is the logo's and `OFFICE.honey #f5b83d`. Neither is a category default. Why dark, in one sentence: one developer in a long evening session, beside terminals that are dark whatever we do, in a room lit mostly by the screen.

### 5.1 Colour tokens

Tokens are semantic and few, as Linear found when it cut 98 theme variables to 3 inputs. Every value below is computed, not guessed. Contrast is WCAG 2.x against the surface named.

| Token | Hex | OKLCH | Use |
|---|---|---|---|
| `--ground` | `#0F1117` | 17.8% .013 271 | Window, terminal well |
| `--panel` | `#151821` | 21.0% .018 270 | Rail, title bar, dialogs |
| `--raised` | `#1C202B` | 24.5% .022 269 | Cards, inputs, hover row |
| `--over` | `#252A37` | 28.6% .025 268 | Selected row, menus, popovers |
| `--ink` | `#ECEAE4` | 93.7% .008 91 | Primary text. It is the office's `TAG_INK`, near enough |
| `--ink-2` | `#B9BBC4` | 79.3% .013 276 | Secondary text |
| `--ink-3` | `#9095A3` | 67.0% .021 270 | Meta, placeholders, timestamps |
| `--ink-off` | `#6B7080` | 54.7% .026 272 | Disabled only, never information |
| `--honey` | `#F5B83D` | 81.9% .150 81 | Your turn: primary button, needs-you mark, count |
| `--honey-ink` | `#17130A` | 18.9% .018 87 | Text on honey |
| `--honey-soft` | honey at 14% | | Needs-you row fill. On panel it resolves to `#342E25` |
| `--work` | `#6FD6A0` | 79.9% .125 159 | Working |
| `--review` | `#8FB8F2` | 77.5% .095 257 | Ready to review |
| `--stop` | `#A7ABB8` | 74.2% .019 272 | Stopped, idle |
| `--danger` | `#F2857A` | 73.4% .135 27 | Destructive, error, deleted lines |
| `--line` | ink at 10% | | Dividers (decorative, no contrast need) |
| `--line-control` | ink at 38% | | Input and control borders |

| Text on | ground | panel | raised | over |
|---|---|---|---|---|
| `--ink` | 15.7 | 14.7 | 13.5 | 11.9 |
| `--ink-2` | 9.9 | 9.3 | 8.5 | 7.5 |
| `--ink-3` | 6.3 | 5.9 | 5.4 | 4.8 |
| `--honey` | 10.6 | 10.0 | 9.1 | 8.1 |
| `--work` | 10.6 | 10.0 | 9.1 | 8.1 |
| `--review` | 9.3 | 8.7 | 8.0 | 7.0 |
| `--stop` | 8.2 | 7.7 | 7.1 | 6.3 |
| `--danger` | 7.6 | 7.1 | 6.5 | 5.7 |

Other pairs: `--honey-ink` on `--honey` 10.4. `--ink` on `--honey-soft` 11.2, `--honey` on it 7.6, `--ink-3` on it 4.5, which is the floor, so meta text inside a needs row uses `--ink-2`. `--line-control` on panel 3.18 and on raised 3.14 (WCAG 1.4.11 asks 3). `--ink-off` is 3.6 on panel, which is why it may only mark disabled controls. Today's `--faint #5c5c66` is 2.9 on surface-1 and `--idle` is 4.26: both go.

Rules:
- Honey fills at most one button per view, and never decorates. If a screen has nothing for you to do, it has no honey except the logo.
- A CLI's brand colour lives only in its logo mark. Rows, cards and pills never take it.
- Terminal colours are what the CLI sends. The theme never touches them, and the terminal well is `--ground`.
- Colours live in `tokens.css` and `palette.ts` reads the same values, with `palette.test.ts` extended to the table above, so the office and the app cannot drift.
- The focus ring is not honey. Honey means your turn, and a honey ring disappears on a lit office wall: `--ink` on the `wallLit` colour is only 2.9. The ring is 2px `--ink` with a 2px `--ground` halo outside it, and the halo gives 5.4 on that wall. This changes the "honey ring" row in Task 11.

### 5.2 Type

Geist and Geist Mono stay. Reason: the owner compared them closely in the native round, they already carry the look he approved, and a change buys nothing against the real problem, which is the sizes. Two fixes come with them:

- **Bundle the fonts.** `index.html:12-14` loads them from Google at every start. Offline or on a slow link the first paint falls back. Ship woff2 with the Vietnamese subset (the native build proved those files). Weights 400, 500, 600 only: the link asks for ten weights across the two families, and the design needs three plus mono regular.
- **One scale, whole pixels, nothing under 12.** Today 28 distinct sizes, 145 of 334 declarations are fractional, 85 are under 12px.

| Token | Size / line | Weight | Use |
|---|---|---|---|
| `--t-meta` | 12 / 16 | 400 | Timestamps, counts, helper text, status words |
| `--t-body` | 13 / 20 | 400 | Row second line, form help, menu items |
| `--t-row` | 13 / 20 | 500 | Row title, button, tab, field label |
| `--t-lead` | 14 / 22 | 400 | Dialog body, plan text, prose in panels |
| `--t-title` | 16 / 24 | 600 | Section titles, agent name in the header |
| `--t-page` | 20 / 28 | 600 | Page title (Project, Settings, dialog) |
| `--t-home` | 28 / 36 | 600 | Only the Home sentence, one per app |
| `--t-mono` | 12 / 18 | 400 | Paths, branches, shas, keycaps, diff at 13/20 |

Numbers use `font-variant-numeric: tabular-nums`. Letter-spacing -0.01em from 16px up, 0 below. No uppercase eyebrows, no italics. Line length in prose panels caps at 72ch. Check every size with Vietnamese diacritics on real strings, since the UI will quote agent text in Vietnamese.

Terminal: measured 17px with a 1.5 line height (26px rows), 25 rows at 1440x900, beside 12.5px chrome. The terminal should not shout over the app, and a 1.5 row wastes a third of the lines. Default 14px with line height 1.25, kept in Settings. Same window, new frame: about 44 rows and about 135 columns.

### 5.3 Space, radius, elevation

- **Spacing:** 4, 8, 12, 16, 24, 32, 48. Nothing else. Rail gutter 8, panel padding 16, section gap 24, Home column gap 32.
- **Radius:** 6 for controls (button, input, chip, keycap), 10 for rows and cards, 14 for dialogs and popovers. Pills only for the status word. Today there are 28 radii and nearly everything is a pill, which is a large part of why buttons, tabs, tags and states all look alike.
- **Elevation:** three levels. Flat (a 1px `--line`), lifted (`0 8px 24px rgb(0 0 0 / .4)`, for menus, palette, dialogs), scrim (`rgb(8 10 14 / .6)`, no blur, what Windows calls Smoke). 97 distinct shadows become 1. All 10 `backdrop-filter` blurs go: they cost GPU on a UHD 630 next to a WebGL scene and DOM terminals, and the craft rules ban glass as a default. The office addendum's "translucent warm panels" becomes solid or 92% tinted panels with no blur.
- **Z-index:** one scale of 6 named steps (rail 10, sheet 20, popover 30, scrim 40, dialog 50, toast 60) in place of 31 values.

### 5.4 Icons

One set, one grid: Lucide at 16px, stroke 1.5, inlined once as an SVG sprite. Today's icons are hand-pasted with stroke widths from 1 to 2.6 and mixed with text glyphs (✕ ⤢ ◫ ⋯). CLI logos stay as they are. State has its own small glyph set, because state must never rely on colour:

| State | Glyph (12px) | Word | Colour |
|---|---|---|---|
| Needs you | filled hexagon, the honey cell's shape | Needs you | honey |
| Ready to review | circle with a dot | To review | review |
| Working | ring with a moving gap | Working | work |
| Idle, done | hollow circle | Done / Waiting | stop |
| Stopped | hollow square | Stopped | stop |
| Out of usage | hollow square with a clock | Back 6:17 PM | stop |
| Error | triangle | Stopped with an error | danger |
| Merged | check | Merged | stop |

### 5.5 Motion

| Token | Time | Use |
|---|---|---|
| `--m-press` | 83ms | Hover, press, focus |
| `--m-swap` | 167ms | Row state, popover, toast, tab underline |
| `--m-panel` | 250ms | Sheet, tray card rising, dialog |
| `--m-level` | 333ms | Level change, the office camera |

These are Windows' own timings, so the app moves like the desktop it lives on. Entrances use Windows' `cubic-bezier(0, 0, 0, 1)`, exits `cubic-bezier(1, 0, 1, 1)`, and nothing else. Today: 48 durations, 6 curves, 32 keyframe sets, 9 infinite loops. Only `transform` and `opacity` move. The single loop that stays is the Working ring, and it pauses when the window is not focused (`body.anim-paused` already does this). Every entrance moves something already visible, so a failed trigger cannot hide content. `prefers-reduced-motion`: position changes become instant, a 167ms opacity swap stays as feedback, the working ring holds still. The splash no longer exists, so its exemption from reduced motion goes too.

Motion earns its place in three moments only: an answered item leaves the tray, an agent lands (the row settles into its new state), and the level change. The rest is instant.

### 5.6 Density

Comfortable, fixed. Row 48 with two lines, 32 with one, button 32, small button 24, input 32, title bar 48 (Windows' size for a bar with a search box), agent header 44. Hit targets never under 24 (WCAG 2.5.8). No density setting: two densities double the states to test.

### 5.7 States

Every interactive thing has the same seven, from tokens:

| State | Treatment |
|---|---|
| Rest | Flat, `--ink-2` label |
| Hover | Fill `--raised`, label `--ink`, cursor pointer, 83ms |
| Press | Fill `--over`, scale .98 on buttons only |
| Focus | Two-layer ring, on keyboard focus only. Opening a dialog puts focus on the first field, never on Close, which is why a heavy white ring sits on Close in every drawer shot today |
| Selected | Fill `--over`, `--ink`, `aria-current`. Never honey |
| Disabled | `--ink-off`, with the reason in a tooltip or in words beside it |
| Busy | Label stays, a 12px ring replaces the icon, `aria-busy` |

Loading: skeleton rows in the real shape after 300ms, nothing before that. Empty: one sentence and one action. Error: section 7.5.

### 5.8 Light theme

No light theme in this overhaul, and no following the OS. Terminals stay dark whatever we do, so a light frame round a dark well would be a mess; one tested theme beats two half-tested ones. The tokens are semantic so one could be added later. What ships instead: `forced-colors` support, so Windows contrast themes work, and a test that no information survives only in colour.

## 6. Surface by surface

### 6.1 One set of parts

Everything below is built from these. If a new screen needs something else, it changes this list first.

**Row** (rail, lists, Home sections). Height 48, or 32 with one line. Radius 10, padding 8. Left: 24px CLI mark or portrait, with the state glyph on its corner. Line 1: title in `--t-row`, truncated, time or count on the right in `--t-meta` with tabular numbers. Line 2: state word first, then detail, in `--t-body`, `--ink-3`. A row that needs you gets `--honey-soft` fill and `--ink-2` on line 2: a fill, not a side stripe (craft ban 1). Hover shows `⋯` on the right. Row, not card, wherever things are comparable.

**Card** (tray, Project › Agents, Board). Fill `--raised`, 1px `--line`, radius 10, padding 12. Head: mark, name, role or CLI, state word. Body: at most three lines. Foot: at most three actions, right aligned, one primary. A card never contains a card.

**Button.** Height 32, or 24 small. Radius 6, `--t-row`, padding 0 12. Three kinds only: **Primary** honey fill with `--honey-ink`, once per view, and only when it is your move. **Secondary** `--raised` fill with 1px `--line-control`. **Quiet** no fill, for toolbars. Danger is Secondary with `--danger` text, filled only inside the confirm dialog. Icon-only buttons have a tooltip with the name and key. 44 button classes become one `.btn` with modifiers.

**Field.** Height 32, `--raised`, 1px `--line-control`, radius 6, label above in `--t-row`, help below in `--t-meta`, error below in `--danger` with a triangle, linked by `aria-describedby`. Placeholder is an example, never the label. Toggles are for settings that apply at once; a chip menu, not native `select`, where it opens over the app. `enhanceSelects` stays.

**Dialog.** Scrim, no blur. `--panel`, radius 14, width 440 or 640. Title 20/28, one paragraph, actions bottom right with Cancel left of the action. Focus lands on the first field or Cancel for danger, never on Close. Esc closes. A dialog never opens another dialog.

**Sheet.** Slides over the right of the main area only, 420 wide, used for a Board card and a plan. The rail and title bar stay live. 250ms.

**Menu.** `--over`, radius 10, row 32. At most 9 items in 3 groups. Today's agent menu shows 13 in the shot and can reach 17.

**Toast.** Bottom right. One line, one action, 6s, pauses on hover.

### 6.2 Title bar

Hierarchy: where am I, what needs me, how do I jump. Layout in 4.2. States: all clear (`All clear` in `--ink-3`), work running (`2 working` with the ring), waiting (`⬢ 2 need you`, honey fill, the only filled element in the bar). Keyboard: the bar is the first Tab stop group. Change: the 68px sentence, house icon and `Resume all` leave. Reference: Windows title bar guidance for the size and the search box. Warp's per-tab status and bell for the pill.

### 6.3 Rail

Top: `+ New agent`, Secondary, full width. Tree: project header 32 high with name, count, and a needs count as a honey hexagon; click selects the Project, the chevron folds its stopped agents (keep that behaviour). The memory icon on each header goes: Memory is a tab now. Rows as above. Founder is pinned first, in its own group, when there is a Company. Jobs, one line each with a thin progress of tokens, sit under Founder. Foot: tray, then one line with the tightest limit (`Claude 91% · resets 6:17 PM`), Settings gear, and `Resume 2` when anything is stopped.

States: no projects shows only `Open a folder`. Stopped rows dim to `--ink-3` but stay readable. The `+148 −455` stays on review rows, cost on hover only, since CLI costs are not comparable and a dollar figure on every row is noise. A 40-row rail needs no virtualizing; keyed updates are enough (`keyeddom.ts` exists, use it, stop rebuilding with `innerHTML`).

Keyboard: arrow or J/K, Enter opens and gives the terminal the keys, `Shift+F10` the menu. Reference: Linear's inbox keys, Conductor's workspace per task, Cursor 2.0 putting agents rather than files first, Cursor's diff counts on the row (repo Mobbin notes), Vercel's move to a resizable sidebar. Sections here are projects, nothing else.

### 6.4 Tray

Card per waiting agent not on stage, newest at the bottom, at most 3 shown then `+2 more`. A permission card: mark, `Ana · bytehive`, the command in mono, two lines at most, then **Allow** (primary), Always, Deny, Open. A question card: the question, then its numbered options as Secondary buttons, then Open. A Company item: `Approve plan: Add a pricing page`, `3 cards, 2 hires`, **Read plan**. A browser ask joins here too.

After answering the card leaves in 167ms, and the agent's row goes back to Working. If the prompt changed before you pressed, the press is dropped and the card says so. This needs new code: `answerOption` sends the key blind today, and the task list is only as fresh as the last tick, so a late click could land `1` in a different prompt. Read the screen again and compare `askKey` at press time.

Keyboard: focus lands here from `N` or `Win+Alt+N`. `A`, `D`, `1`-`9`, `Enter` to open. Reference: Warp's Request category and mailbox, Factorio's alerts where a click flies you to the cause, Slack's Activity as one place for what is addressed to you. The Grid card's `miniAnswered` logic moves here as it is.

### 6.5 Composer

One component in three places: Home in place, Project empty state, popover under `+ New agent`. Big text box, then one row of chips: project, CLI, how many, and for a Company user a target chip `Agent | Founder`. Primary `Start` on the right, Enter starts, Shift+Enter adds a line, and it respects IME composition (the code already checks `isComposing`, keep it, Vietnamese typing depends on it). Plan room note sits inside the box on its own padded line, not glued to the border as today. Keep `Don't ask before running commands` as an honest chip that turns `--danger` when on, because that one setting deserves to stay visible. Save as preset moves to the `⋯` on the composer. Chips in place of 9 radio pills.

Error and rate-limit rule: the text stays in the box until the agent has really started. Today `job.value = ""` runs before `spawnAgents` resolves, so a failed start loses the job. Reference: Cursor's new agent and Linear's New issue from the repo's Mobbin notes, Things' Quick Entry for the speed. **Not taken:** a prompt-card grid as a home screen.

### 6.6 Agent › Terminal

Header 44: name 16/600, then the job in `--ink-2`, state word, tabs, `Files`, `⋯`. An agent started without permission prompts shows a `--danger` chip, `Asks nothing`: today that fact lives only in the CLI's footer, in a product about answering permissions. Branch moves to the tooltip and History. Commands and Model move into `⋯`. No `✕` and no pencil: rename is in the menu. The terminal runs edge to edge with an 8px gutter. No card round it, no radius 16 frame: the frame is the window.

States: starting (`Starting Claude Code…` in the well, the only spinner), running, waiting, stopped (a line across the top of the well: `Stopped 12 min ago. **Resume** Hand over`, Resume primary because it is your move), out of usage (`Out of usage until 6:17 PM. **Hand over** Wait`), error (reason in words and Resume). Focus: the 1px frame and `Keys go to Dao`. **Not taken:** Warp's block-based output and any input box of ours over the CLI. Blocks mean owning the prompt, which is exactly what the owner refused.

### 6.7 Agent › Changes

Three zones across the whole main area.

```
Dao's changes  agent/dao-pricing → master   ✓ Merges cleanly   6 files +148 −455   [Ana|Bo|Cid]
┌ files 260 ──┬ diff ───────────────────────────────────────────────┐
│ ☐ queue.css │ unified by default, split at 1400px and up           │
│ ☑ stage.css │ line numbers take a comment, as today                │
├─────────────┴──────────────────────────────────────────────────────┤
│ Note for Dao…                        [Discard ⋯] [Send back] [Merge into master  M] │
└────────────────────────────────────────────────────────────────────┘
```

At 1280x720 the diff gets at least 460px, against about 130px now. The file list is its own column, so 6 files no longer eat half the height. The decision bar stays fixed. `Merge` is primary only when the dry run says clean **and** every file is marked viewed. Before that it is Secondary and reads `Merge without viewing 4 files`, and `M` asks for a second press. Today Merge is the loudest control at 0 of 6 viewed, on one bare key. On conflict the primary becomes `Send back to resolve`. No own branch: the bar has Commit and Discard, from `diffview.ts`. `J` `K` next file, `V` viewed, `C` comment, `M` merge. Viewed state, notes and comments already survive switching; keep that.

Race: the switcher `Ana · Bo · Cid` with each one's size and state, and `Keep this one` as the primary. Reference: Conductor for review inside the task's workspace and comments that go back to the agent, GitHub's Viewed and Graphite's review box from the repo's Mobbin notes, GitHub Desktop's commit box under the file list. Worth trying after this ships: Zed's multibuffer, one continuous scroll instead of per-file swaps.

### 6.8 Agent › History

One timeline, newest last, grouped by day, durable. Lines: state changes, what you answered, hand-offs in and out with the message (Flow), merges, recordings, and for a Company person their card, checklist and evidence (the Report tab). Filter chips only if it passes 100 lines. A link at the end: `Earlier conversations` to Sessions. Reference: GitHub Desktop keeps Changes and History as two tabs of one place, which is the model here.

### 6.9 Project

Header: name, path, `+ New agent`, then tabs.

**Agents.** Cards of this project's agents with what each is doing and last said (the Grid's `gridcards.ts`, read only for visible cards), answer buttons, `List | Office` switch. With a Company the Office view is the floor, signboards and the wall board, and the Message box becomes the composer.

**Board.** Full width, lists 280, so four lists fit at 1440 where today the fourth is cut. Card: title, label dot, assignee mark, checklist count, not a full-width pill per assignee. Detail in a sheet, so the board stays visible. Keys: arrows, Enter, `C` add. Conductor button goes. Pointer Events drag stays (WebView2 kills HTML5 drag).

**Files.** Tree left, editor right, and the same component as the right split in Agent. Lazy loads CodeMirror.

**Memory.** Today's list, as a page. Tabs across projects go: the breadcrumb does that.

**Sessions.** List and reader as today (good), full main area, Replays as a filter, scope `This project | All folders`.

Reference: GitHub Desktop's tabs; Conductor's Checks tab for gathering one project's state in one place.

### 6.10 Home and Office

Layout in 4.5. Office specifics, for Task 11: signboards use `--panel` and `--t-row`, needs count in honey, a leader line to the floor. Job box is the composer. Needs you moves to the tray on the left, Jobs to the rail: the right column goes and the scene gets that width. Plan sheet reads as a proposal: "The Founder wants to hire a Developer on bytehive and a Copywriter on gcare, 3 cards, up to 5M tokens", with Approve as the screen's one primary and Send back beside it, instead of the 60px chip of today. Numbers animate only on change.

From the games:

- **A problem must read at the far zoom.** Factorio draws the status icon on the affected building, and an alert list click moves the map to the cause; Two Point puts event icons over heads. So a status mark over a person stays the same screen size at every level, and every tray card flies the camera to its desk. Today 5 items need you and one `!` is visible on the building: at Building each needs-you person gets the mark, and floors with several get a count on the sign. The earlier "bubbles stay rare" call was right for working people and wrong for people who need you.
- **Show the inside by removing what blocks it.** The Sims' Cutaway hides walls near the camera, per floor. The office's open two walls already do this. Keep it.
- **Progress can be physical.** Game Dev Tycoon floats bubbles from each desk into a meter. That is the honest version of "life" for working agents: small rising marks at a desk that is really producing output, driven by real PTY activity and nothing else. Cheap, and only while someone works.
- **Overlays answer one question.** Two Point and Oxygen Not Included recolour the map by one metric. One overlay is worth having: tokens used per floor. Later, not now.
- **Calm is a feature.** Townscaper's designer calls it radically casual. When nothing needs you the building shows no honey at all, and the eye can rest.

**Not taken:** free camera, build mode as a default, sound on events, a meter on every person, ten F-key overlays. This is a window you glance at, not a game you play. On brand: the reviewer's point stands that a concrete tower says little of a hive. Honey-cell details (hexagon floor tiles, the signboard shape) can carry that without touching the logo. A note for Task 11.

### 6.11 Palette

Keep the structure; it already teaches keys and groups by state, and it is the best screen in the app. Width 640. Top group `Needs you`, then Recent, Agents, Go to, Actions, Projects. Actions show only what applies here. Sub-line shortens to project and state. `Enter` opens, `Ctrl+Enter` opens without moving focus to the terminal. Virtualize over 200 rows. Add text fallback: no match offers to start an agent with that text. Reference: Raycast's contextual action panel with keys shown beside each action, Superhuman Command.

### 6.12 Settings

A page at the Home level, not a modal. Left nav: General, Agents, Notifications, Terminal, Office, Browser, Remote, Automation, Usage, Updates. "Inbox" as a section name goes, and "Sessions", which held three links, goes. 30 rows today become about 22: Grid size, both tips rows, the Director toggle and Open Changes when done are gone. Every change saves at once and says nothing; keep that, and keep it for all rows. New rows: `Office quality: Auto, Low, High`, `Global key`. The Ask before running row sits on top of Agents with its honest warning. 

### 6.13 Help

`?` opens a sheet of keys for the current screen, then a link to the rest, in `Layer` order. Nothing about Chat, which no longer exists. Empty states and the palette carry the teaching.

## 7. Interaction rules

### 7.1 One way to do each thing

There may be several ways *in*, but each job has one *place*, with one name.

| Job | The one place | Ways in |
|---|---|---|
| Answer an agent you are looking at | Its terminal | Type |
| Answer an agent you are not looking at | Rail tray | Click, or keys with focus on the rail |
| Approve a plan | Plan sheet, from the tray | Click |
| See what an agent changed | Agent › Changes | Row, tab, `2`, palette |
| Land, send back, discard | Decision bar in Changes | Button, `M` |
| Start an agent | The composer | Home box, `+` in the rail, `Ctrl+N`, palette |
| Give the Founder a job | The composer, target set to Founder | Same |
| Plan work | Project › Board | Tab, `G B`, palette |
| Stop, restart, rename, hand over, remove | Agent `⋯` menu, the same menu as right-click on a row | Menu |
| Change a setting | Settings | Rail foot, `Ctrl+,`, palette |

`✕` means one thing, close this surface, and never touches an agent. Today it means Remove agent on the header, Take out of Grid on a Grid tile, close panel on the dock and close dialog elsewhere, four meanings a few pixels apart.

### 7.2 Keyboard model

Three layers. Which one you are in is always visible.

**Layer 1: the terminal has the keys.** Every key goes to the CLI. The app reacts to nothing. The stage shows a 1px `--ink` frame and the header says `Keys go to Ana`. The only local keys are what Windows Terminal itself keeps, as the owner settled in v0.12.13: `Ctrl+C` over a selection, `Ctrl+Shift+C`, `Ctrl+Shift+V`, `Ctrl+Shift+F`, plus `Ctrl+V` paste. Implementation: a capture listener at `window` returns at once when `document.activeElement.closest(".xterm")`, and `attachCustomKeyEventHandler` keeps only those. Support win32-input-mode (`ESC[?9001h`) again, as the native build's `term_win32.rs` did. Ctrl+Tab, Alt+digit and Ctrl+Shift+T/B lose their pass in `terminal.ts:278-287`. Ctrl +/-/0 zoom stops firing over a terminal too; Ctrl+wheel stays, it is the mouse.

**Layer 2: the app has the keys.** Focus is anywhere but a terminal. Single keys work, as in Linear and Superhuman, and nothing fires while typing in a field.

| Key | Does |
|---|---|
| `Ctrl+K` | Palette |
| `J` `K` or `↑` `↓` | Move in the rail |
| `Enter` | Open the row and give its terminal the keys |
| `Esc` | Up one level: Agent, Project, Home |
| `N` | Next thing that needs you |
| `A` `D` `1`-`9` | Answer the selected tray card: allow, deny, option |
| `1` `2` `3` | Agent tabs: Terminal, Changes, History |
| `Ctrl+N` | Composer |
| `G` then `H` `B` `F` `M` `S` | Home, Board, Files, Memory, Sessions |
| `M` `V` `C` | In Changes: merge, viewed, comment |
| `Ctrl+,` | Settings |
| `F6` | Next region: rail, main, tray. Windows' own convention |
| `?` | Keys for this screen |

That is about 15 keys in place of 26, and none can collide with a CLI, because Layer 1 owns the terminal.

**Layer 0: the operating system.** One global hotkey, default `Win+Alt+N` (for Needs you), registered with the OS. It is the one thing that crosses into the app from a focused terminal, and it does not sit over the terminal: the Win key never reaches a CLI, and it works from any app, including when ByteHive is behind others. Press it: the window comes forward, focus moves to the rail on the next waiting item. Press it again: the keys go back to the terminal you were in. Default chosen from Microsoft's list, where `Win+Alt` with B, D, H, K, arrows, Enter and digits is taken. `Win+Alt+Space` is out because PowerToys Command Palette uses it. Settings lets you change or turn it off, and a failed registration says so there in words.

The mouse always works as the plain way out: clicking anything outside the terminal moves focus to the app. Clicking the terminal gives it the keys.

Zed works the same way inside: keymap contexts form a tree and the Terminal node beats its parents, so the rule is an ordinary design, not a special case. Rejected: a tmux-style prefix key (a key over the terminal, the owner's rule forbids it), VS Code's skip-shell list (the app wins by default, the reverse of what the owner decided), and `Esc Esc` (Claude Code uses it for rewind).

### 7.3 Undo and confirm

| Action | Treatment |
|---|---|
| Answer Allow or Deny, approve a plan | No confirm. The exact command or plan is on screen when you press |
| Merge | No confirm once every file is viewed and the dry run says clean. With files unviewed, the button says so and `M` needs a second press. Toast with the sha |
| Mark done, clear merged, close a menu | No confirm, Undo in the toast for 6s where the state allows it |
| Remove agent, close project | Confirm naming what stops and what stays: "Its branch and the changes on it stay in git" is good copy, keep it |
| Discard branch | Confirm with the branch name and file count. Danger button, focus on Cancel |
| Cancel a job | Its own item in the job's `⋯` menu with a confirm. Today an empty Send back note cancels the job, with a second press as the only guard: a destructive path hidden in an empty field |
| Allow with a truncated command | Not offered in the tray. If the detail does not fit in two lines, the only button is Open |

"Don't ask again" exists on one confirm today and goes: a confirm is either worth showing or it should not exist.

### 7.4 Feedback timing

Limits are Nielsen's: 0.1s feels instant, 1s keeps the flow, 10s holds attention.

| Time | What the user sees |
|---|---|
| Under 100ms | Pressed state, selection, focus. Always, from the input event, before any IPC |
| 100ms to 1s | The control shows busy, nothing else moves |
| 1s to 10s | Words: "Merging into master", "Starting 3 agents" |
| Over 10s | It goes to the background, the row shows the state, a toast says when it ends |

One toast stack bottom right, in `--raised`, at most 2 at once, as Warp does, pausing on hover. Today's `topNote`, `paneToast`, tips and update toast become one component. A toast never holds the only copy of information: what it says is also in History. Windows notifications fire only when the window is not focused, as now, and a click opens that agent. While something waits, the tray icon carries the count and the tooltip says who.

### 7.5 Errors in plain words

Form: what happened, what is safe, what to do. Never a raw exception in the first line; the detail sits behind a Details disclosure. Today `inboxreview.ts:155` prints `Couldn't read the changes: TypeError: ...` straight to the user.

| Today | New |
|---|---|
| `Couldn't read the changes: TypeError: Cannot read properties of null` | Can't read Cid's changes. Git did not answer in `D:\...\x-cid`. Nothing was touched. **Try again** · Details |
| `The 3D office could not start, so you are in the Inbox.` | The office can't draw on this graphics card. Everything still works here. **Try again** |
| `Node 22.13 needed` style errors | The Company needs Node 22.13 or newer. This PC has 20.11. **How to update** |
| `Couldn't start a Codex agent` | Codex did not start: `codex` is not on PATH. **Open Settings** |
| `Leave empty to cancel the job` in a placeholder | Send back needs a note. Cancel job is its own item |
| A pane that fails to start | The row says Stopped with an error and the reason, and Resume stays |

Text stays English and plain, in the voice the app already has at its best ("Its branch and the changes on it stay in git"). No em dash. Words to stop using in the UI: Fleet, Queue, Stage, Split, Grid, Inbox, Director, Conductor, Crew, Hive as a noun for a screen. Terms that stay: Home, Project, Agent, Changes, History, Sessions, Board, Memory, Files, Company, Founder, Lead, Job.

## 8. Performance budget

Owner's words: must not lag, 1 to 2 GB at most. What counts: `bytehive.exe` plus its WebView2 processes. CLI agents are their own processes and are not counted. Terminals always win.

### 8.1 Where it stands

`MEASURED` on this machine, 2026-11-07, sampling live processes, read only. Conditions were not controlled: these are what was running, not a test.

| Process tree | Consoles | Working set | Private | Biggest part |
|---|---|---|---|---|
| Installed `bytehive.exe` 0.14.1 | 62 | 638 MB | 603 MB | Renderer 257 MB, GPU private 249 MB |
| Dev build, idle-looking | 2 | 586 MB | 389 MB | Renderer 174 MB |
| Dev build with 12 terminals, gate run in progress | 12 | 735 then 758 MB | 778 then 944 MB | **GPU private 551 then 706 MB** |

Two readings. There is room under 1 GB today. And the last row's GPU private bytes rose 155 MB between two samples some minutes apart. That may be a harmless test phase, or the start of the growth the addendum calls a failure. It is for the gate agent to settle over an hour, with `renderer.info.memory`, and private bytes rather than working set are the line to watch.

Browser side, `MEASURED` in headless Chrome with 9 terminals: JS heap 17 MB, 2,276 DOM nodes, 236 buttons, palette open to paint 33ms. Chrome is not WebView2, so treat these as a floor.

### 8.2 The budget

| Part | Normal use, 20 agents, office open | Ceiling |
|---|---|---|
| `bytehive.exe` | 60 MB | 100 MB |
| WebView2 browser, utility, crashpad | 200 MB | 250 MB |
| Renderer: JS, DOM, terminal buffers | 350 MB | 700 MB |
| GPU process | 300 MB, 150 with no scene | 600 MB |
| **Tree** | **under 900 MB** | **never over 2 GB, and alarms at 1.5 GB** |

Report working set and private bytes both. Checks: 1 hour with a streaming fleet, 50 level changes, and no upward trend after the first 10 minutes.

Terminal memory: xterm does not document a per-line cost, and the one figure found is a third-party estimate of about 8 MB for 5,000 lines at 200 columns. Measure it in the gate. Today's 5,000 shown and 1,000 hidden stays until a number says otherwise. On that estimate 20 agents with 2 on screen hold about 45 MB, small next to the rest, so scrollback is probably not where the RAM goes.

Other timings: key to first paint under 100ms always. Level or tab change under 100ms without the scene, and at 30fps or better with it. No frame over 100ms. Cold start to a usable window under 800ms, against 2,100ms by timer today. Idle CPU under 2% of all cores, working with ambient motion under 5%, both from the addendum.

### 8.3 Rules

**Rendering**
- Hidden terminals parse but never paint. In the new frame exactly one terminal is on screen. A pane out of view gets no paint work and no `fit` until it is shown. This audit did not measure what hidden panes cost today, so check it in package J before assuming there is a win.
- Keep `WEBGL_BUDGET = 0`. The DOM renderer is what survived the 15 to 20s freeze. Do not reopen that for speed nobody asked for.
- `contain: strict` on the terminal well and the rail. `content-visibility:auto` with `contain-intrinsic-size` on Board lists, Sessions and History. It keeps content in find, tab order and the accessibility tree.
- Animate `transform` and `opacity` only. No `transition: width` (4 today). Never animate `box-shadow` or `filter`: a card glow was the trigger of the old freeze.
- No `backdrop-filter` anywhere.

**No layout thrash**
- One writer: state changes mark what is dirty, one `requestAnimationFrame` writes. Today `render()` runs every second and on every task change, and rebuilds whole lists through `innerHTML` (102 sites). It compares strings to skip work, which helps, but it still builds them.
- Reads before writes, once per frame. `getComputedStyle(body)` and `clientWidth` inside `layoutSplit` go with the Grid. Sizes come from one `ResizeObserver`.
- Keyed updates for the rail and tray with `keyeddom.ts`. A row updates its own text; time labels update alone.

**Timers**
- 18 `setInterval`s become one scheduler with three lanes: 1s while visible, 5s hidden, and slow polls. `hiddenGate` already has the right idea. Git diff polls run for agents that printed recently, not on a fixed clock for all.
- The screen snapshot behind `deriveTaskState` runs after output, not on every tick. An agent that has printed nothing has not changed its screen.

**Lazy loading.** Main chunk 1,683 KB raw today. Target under 600 KB.

| Load when first used | Today |
|---|---|
| CodeMirror and its 8 languages | Eager, `editor.ts` |
| Board (`kanban.ts`, 52 KB source) | Built at start by `initDock` |
| `marked`, DOMPurify, Sessions reader | Eager |
| Settings pages | In `index.html` on every start |
| Browser live view | Eager |
| Office scene | Already lazy (531 KB). Keep |
| Fonts | Bundled, `font-display: swap`, only 4 files |

CSS built 175 KB, target under 90 KB once the token system lands and the dead surfaces go. A grep finds up to 80 classes no code names, though some of those are built at runtime.

**Virtualize:** diff lines (no more `MAX_LINES` cut-off), Changes file list over 200, Sessions list (up to 300), palette over 200, History over 200, transcripts (the `WINDOW` of 200 stays). Not the rail: 40 keyed rows are cheap, and windowing would hurt find and focus.

**Office.** From the addendum, unchanged: draw calls under 300, stop when not visible, free the renderer after 2 minutes away, auto quality stepping down shadows, then glow, then ambient. Add: at Agent level the scene is not mounted at all, and `dispose()` runs on geometry, materials, textures and targets by hand, as three.js says removing a mesh frees nothing. Do not lean on `forceContextLoss`. In the tray, set WebView2's `MemoryUsageTargetLevel` to Low while the window is hidden and back to Normal on show, if Tauri exposes it. Do **not** use `TrySuspend`: it pauses scripts, and the app must keep reading agents while hidden. Not verified in Tauri's API, so check before planning on it.

**Verify:** extend `tools/office_gate.mjs` with the table in 8.2 and run it after each package, on real WebView2, not Chrome.

## 9. Roadmap

Each package ships alone, leaves the app working, and can be reverted on its own. Effort: S up to 1 day of agent work, M 2 to 4, L 5 or more. Version numbers follow the plan's where they can.

### 9.1 The order

| # | Package | When | Replaces | Effort | Risk | Verify |
|---|---|---|---|---|---|---|
| **A** | **Keys and truth.** Terminal passthrough by window capture gate. Drop `paneStatus` and the Fleet view's own classification. The row's changes come from the same diff as Review, committed work counted, untracked junk not (P3). Global `Win+Alt+N`. Fix Help and the false lines on Start. | Now, before 0.15.0 | `onKey`, `onCtrlK`, the shortcut list in `main.ts`, passes in `terminal.ts` | S | Low. Owner loses Alt shortcuts in the terminal, which is what he asked for | The key probe from this audit as `tools/ux_probe.mjs`: every combo gives its bytes to the terminal and no app action. Fleet count equals the sentence, or Fleet is gone |
| **B** | **One look.** New `tokens.css`, old names aliased so nothing breaks. Bundled Geist. Honey primary, 3 button kinds, row and card, focus ring, radii, one shadow, no blur, Lucide sprite, state glyphs. No layout change. | Inside the Task 11 pass | White primary, pills, 28 radii, 97 shadows, Google Fonts link | M | Medium: wide but shallow, so visual regressions, not logic | Screenshot set before and after, render-check with `contrast_failures=0`, `palette.test.ts` on the new pairs, offline start shows Geist |
| **C** | **Office chrome.** Cards with portraits, plan as a proposal, signboards, job box as hero, all from B's parts. | Inside the Task 11 pass | Rail's chips and field lists | M | Low | Task 11's rows, with the two edits in 9.2 |
| | **Release 0.15.0** | | | | | Task 12 unchanged |
| **D** | **The big delete.** Phase 4's Tasks 18 and 19, moved up, plus Pomodoro, Fleet, tips, background picker, broadcast keys, dead CSS. | Right after 0.15.0, before Phase 3 | Conductor, Director, `orchestrator/`, sprites (4.8 MB) | M | Low: the plan already lists every dependency | `tsc`, `npm test`, one note for Conductor users as Task 18 says |
| **E** | **The frame.** Title bar, rail on every screen, breadcrumb, three-level router. Office mounts in the main area. Board, Files, Memory, Sessions become Project tabs. Bottom dock and dock panel go. | After D | `inbox-dock`, `dock.ts`, `of-layer` cover, `tb-*` | L | **High**: touches `inbox.ts` (73 KB), `office.ts`, `workspace.ts`. Split it into E1 frame and rail, E2 office mount, E3 tabs, each releasable | Walkthrough scripts for J1 to J6. Back and Esc through all levels. 12 agents at 1280x720 |
| **F** | **Agent tabs.** Changes full width with decision bar, Compare inside, Commit from `diffview.ts`. History durable and merged with Flow and Report. | After E1 | `inboxreview.ts` layout, `diffview.ts`, `flow.ts` panel, Compare modal, Desk's own tabs | M to L | Medium: merge path must not change. Layout only, `review_land` untouched | Merge, conflict, send back, discard on a scratch repo. Diff at least 460px at 1280x720 |
| **G** | **Tray.** Off-stage answers on every screen, pill, tray icon count, stale-press guard. Grid goes. | After E1 | Grid, `gridfit.ts`, pins, Start's Waiting list, Answer button | M | Medium: a wrong key into a CLI is the worst failure here, so the guard is the core of this package | Test the stale press. Answer 20 prompts across 3 CLIs by tray |
| **H** | **Home and composer.** One composer, Home without a Company, first run, no splash. | After E | `inboxstart.ts`, `inboxnew.ts`, splash | M | Low | Five-second test on a fresh profile, job text survives a failed start |
| **I** | **Settings page, words, README.** | After E | Settings modal, Scheduled, Usage, Replays modals, all stale copy | M | Low | Glossary grep: no Fleet, Inbox, Queue, Stage, Grid in UI strings |
| **J** | **Weight.** Lazy chunks, one scheduler, keyed rail, virtual lists, CSS split by component and pruned. | Lazy loading any time. CSS last, after E to I | 1,683 KB main chunk, 18 timers, 175 KB CSS | M | Low | Section 8's table, on real WebView2, in the gate tool |

Phase 3 fits round this. Tasks 14, 15 and 16 are engine work and can run in parallel with E to I, on their own. **Task 13, character editing, waits for E and B**, or its panel gets built twice. Task 17's release follows.

### 9.2 What changes in the running plan

1. **Phase 4's removals move earlier: yes.** Tasks 18 and 19 run straight after 0.15.0. E would otherwise rebuild the Board header round a Conductor button and Settings round a Director toggle that are about to die, and Task 18's table already proves these are safe. Task 20's release folds into the next one.
2. **Task 11's brief, four edits.** The focus ring is ink with a dark halo, not honey (5.1). Panels are solid or tinted without blur, so "translucent" means no `backdrop-filter`. Do not polish the Desk's left column or the Terminal/Changes/Report strip: E and F replace them. Spend the pass on Building, Floor and the cards. Every person who needs you gets the mark at Building, reversing "bubbles stay rare" for that one state.
3. **The plan's global constraint "people who never use the Company see no change" ends with 0.15.0.** The owner has now asked for the whole app to change. A note for the plan, since that line would otherwise block A, D and E.
4. **The gate agent should look at one number**, in 8.1: GPU private bytes rising between two samples.

### 9.3 What stays unproven

Evidence levels: the key probe, the state mismatch, geometry, counts, contrast and memory samples are `MEASURED`. Screens are `OBSERVED` from headless Chrome with a fake backend, so colour and antialiasing on real WebView2 differ, as the plan warns. Step counts are `SIMULATED` walkthroughs. That the tray is faster than the Grid, that nobody misses Pomodoro, and that honey reads as "your turn" are `ASSUMED`.

What only people can answer, and how: after E and G, give three developers who have never seen the app the job "start an agent, answer it, merge its work" and watch without helping. Count wrong first clicks and time to the first answer. Ask the owner to live on the new frame for a day, and to note each time he wanted a key inside a terminal and could not reach what he wanted, since that is the one decision here that trades speed for his rule.

## 10. Sources

### 10.1 Patterns taken

| Pattern | What it solves here | Source |
|---|---|---|
| Workspace per task: branch, terminal, diff, review path | One tree, Agent level with its tabs | Conductor |
| Agents, not files, as primary navigation | The rail on every screen | Cursor 2.0 |
| Complete, Request, Error categories; per-session status; 2 toasts at most; click jumps | Knowing which of N agents is blocked. Pill, tray, toasts | Warp |
| Terminal context beats parents in a keymap tree | Layer 1 of the keyboard model | Zed |
| Multibuffer review of all edits | Later step for Changes | Zed |
| Changes and History as tabs of one place, commit box under the list | Agent tabs, the decision bar | GitHub Desktop |
| Inbox keys, J/K, one-chord go-to (G then I) | Layer 2 of the keyboard model | Linear |
| Few theme inputs instead of 98 variables | Small token set | Linear redesign |
| Contextual action panel, keys shown beside actions | Palette | Raycast, Superhuman Command |
| Quick Entry: capture without going anywhere | Composer popover, global key | Things |
| One place for what is addressed to you | Tray | Slack's Activity tab |
| Docked, resizable panels after floating ones failed | Bottom dock and floating sheets go | Figma UI3 reversal |
| Horizontal tabs moved to a resizable, collapsible sidebar | Rail, resizable, folding when narrow | Vercel |
| Status icon on the object, alert click flies to the cause | Marks in the office, tray card to desk | Factorio |
| Icons over heads, overlays by one metric | Office marks, later token overlay | Two Point Hospital, Oxygen Not Included |
| Cutaway per floor | Keep the open walls | The Sims (forum sources) |
| Bubbles rise from desks to a meter | Honest signs of life | Game Dev Tycoon |
| Radically casual calm | A quiet building has no honey | Townscaper |
| 32 or 48px title bar, 83/167/250ms, Smoke, F6 next pane | Native feel on Windows | Microsoft |

### 10.2 Famous patterns that would be wrong here

| Pattern | Why not |
|---|---|
| VS Code: app shortcuts skip the shell by default | The reverse of the owner's rule. Its own escape hatch costs features like Ctrl+F |
| tmux prefix key | A key over the terminal |
| Warp blocks, a universal input over the CLI | Means owning the prompt. ByteHive shows the CLI, untouched |
| Conversation UI as the main view (Claude, ChatGPT, Codex apps) | Chat was tried here and removed in v0.9.9. The CLI is the conversation |
| Superhuman-style auto-advance after an action | The next agent's terminal jumping under you while you type is how the wrong key reaches the wrong agent |
| Sort by urgency (Linear, mail) | The owner fixed the order so nothing jumps. The tray gets urgency without moving the tree |
| Arc-style invented vocabulary and hidden chrome | Its founder called this a novelty tax and said Arc was bloated. Fleet, Flow, Stage and Conductor are the same tax |
| Card-grid dashboards, KPI tiles | Already rejected in `design/redesign/references.md`: too little signal per pixel at 10 agents |
| Floating command dock | Figma tried floating and came back |
| Sims or Two Point depth of simulation | The office is a glance, not a game |

### 10.3 Links

All read for this work on 2026-11-07 unless marked.

- Conductor: <https://www.conductor.build/docs/concepts/workflow>, <https://conductor.build/docs/reference/diff-viewer>
- Warp: <https://docs.warp.dev/agents/capabilities/agent-notifications/>, <https://www.warp.dev/blog/reimagining-coding-agentic-development-environment>
- Cursor: <https://cursor.com/blog/2-0>
- Zed: <https://zed.dev/docs/ai/agent-panel>, <https://zed.dev/docs/key-bindings>, <https://zed.dev/blog/videogame>
- VS Code: <https://code.visualstudio.com/docs/terminal/advanced>, <https://code.visualstudio.com/docs/copilot/agents/overview>
- GitHub Desktop: <https://docs.github.com/en/enterprise-server@3.9/desktop/making-changes-in-a-branch/committing-and-reviewing-changes-to-your-project-in-github-desktop>
- GitHub Agent HQ: <https://github.blog/news-insights/company-news/welcome-home-agents/>
- Codex app (press): <https://itbrief.co.nz/story/openai-unveils-codex-macos-app-for-multi-agent-coding>
- Claude Code keys: <https://code.claude.com/docs/en/interactive-mode>
- Linear: <https://linear.app/docs/inbox>, <https://linear.app/now/how-we-redesigned-the-linear-ui>
- Superhuman: <https://help.superhuman.com/hc/en-us/articles/43658258433299-Desktop-Shortcuts>
- Raycast: <https://manual.raycast.com/action-panel>
- Things: <https://culturedcode.com/things/support/articles/2785159/>
- Arc: <https://www.thurrott.com/cloud/web-browsers/321435/the-browser-company-discusses-the-future-of-arc-dia-and-the-web>, <https://techcrunch.com/2025/11/03/dias-ai-browser-starts-adding-arcs-greatest-hits-to-its-feature-set>
- Slack: <https://slack.com/intl/en-au/help/articles/16764236868755-An-overview-of-Slacks-new-design>, <https://slack.com/intl/en-au/blog/productivity/a-redesigned-slack-built-for-focus>
- Figma: <https://www.figma.com/blog/figma-2024-we-shipped-it-you-shaped-it/>
- Vercel: <https://vercel.com/changelog/dashboard-navigation-redesign-rollout>
- Two Point Hospital: <https://www.magicgameworld.com/two-point-hospital-ui-and-reading-your-menus/>, <https://caniplaythat.com/2019/02/24/deaf-game-review-two-point-hospital/>
- The Sims (forum): <https://forums.ea.com/discussions/the-sims-4-gameplay-en/invisible-wall-problems-/8639777>
- Game Dev Tycoon: <https://thenerdstash.com/review-game-dev-tycoon>
- Townscaper: <https://pcgamer.com/townscapers-developer-on-how-its-radically-casual-design-is-inspiring-a-new-wave-of-low-stress-builders-to-adapt-the-blueprint>
- Factorio: <https://wiki.factorio.com/Alerts>
- Oxygen Not Included: <https://oxygennotincluded.wiki.gg/wiki/Overlay>
- Nielsen limits: <https://www.nngroup.com/articles/response-times-3-important-limits/>
- WCAG 2.2: <https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html>, <https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html>
- WebView2: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance>
- xterm.js: <https://xtermjs.org/docs/api/terminal/interfaces/iterminaloptions/>, third-party memory estimate <https://github.com/clawterm/ClawTerm/issues/305>
- three.js: <https://threejs.org/manual/en/cleanup>, <https://github.com/mrdoob/three.js/issues/27100>
- `content-visibility`: <https://developer.mozilla.org/en-US/docs/Web/CSS/content-visibility>
- Windows: <https://learn.microsoft.com/en-us/windows/apps/design/basics/titlebar-design>, <https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing>, <https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography>, <https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/materials>, <https://learn.microsoft.com/en-us/windows/apps/develop/input/keyboard-accelerators>
- Windows shortcuts: <https://support.microsoft.com/en-us/windows/keyboard-shortcuts-in-windows-dcc61a57-8ff0-cffe-9796-cb9706c75eec>
- PowerToys Command Palette: <https://learn.microsoft.com/en-us/windows/powertoys/command-palette/overview>
- In this repo: `design/redesign/references.md`, about 40 Mobbin screens read on 2026-09-23 (Linear, Cursor, Superhuman, GitHub, Graphite, Devin, Vercel and others)

Evidence: `shots/audit-*.png` in this folder (43 files), and `shots/task-9-*.png` for the office in a real run.

### 10.4 What could not be reached

- **Mobbin: every call failed with "requires a paid plan".** No fresh Mobbin research was possible. The repo's earlier Mobbin notes stand in, and they are six weeks old.
- No primary source was read for: Notion, the Claude desktop app, Superhuman's 100ms rule, Conductor's checkpoints, Arc's own help pages (a third-party guide and the founder's letter via Thurrott), the Sims' plumbob, RimWorld beyond its wiki page, and Zed's speed figures, which conflict.
- Things was seen through search summaries. Two Point, The Sims, Game Dev Tycoon and Townscaper come from press and forums, not their makers. Only Factorio and Oxygen Not Included are from their wikis.
- The Windows-key default `Win+Alt+N` is free on Microsoft's published list. It has not been tried on this machine.
- Screens are from headless Chrome and a fake backend. The fixture's numbers disagree in places the real app would not (rail against drawer totals). Findings resting only on that were left out.
