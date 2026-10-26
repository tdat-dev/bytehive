# Changelog

All notable changes to Maestro are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.7] - 2026-10-26

### Added

- **Project memory.** Each project keeps what every agent working in it
  should know: conventions, decisions, where things live, gotchas. Agents
  read it when they start and add to it as they learn, whatever CLI they
  run. Open it with the book beside a project's name (or Ctrl K, or an
  agent's right-click menu) to add, edit and delete facts yourself, or to
  send one fact, or all of them, to a running agent of the project to check
  against the code and fix. It lives in .maestro/memory.json and never
  takes keys or passwords.

## [0.10.6] - 2026-10-26

### Fixed

- **An agent's browser tab keeps going when its window is covered.**
  Windows tells Chrome a covered window is hidden, and Chrome then stopped
  drawing the page and slowed it to once a minute, so the agent waited for
  you to bring the window up. Maestro for Chrome (0.1.11) now keeps every tab
  an agent drives running behind other windows.
- **Maestro uses far less of the GPU in the background.** An agent's spinner
  repainted its terminal on every update, competing with a video on the same
  screen. While Maestro is not the window you are using, terminals draw four
  times a second and looping animations pause; click back in and everything
  catches up at once.

## [0.10.5] - 2026-10-26

### Changed

- **The Grid says which project each agent is in.** Its cards are grouped
  by project, under the project's name, with the terminal's own project
  first, so three agents called Ana in three repos no longer look alike.
  The big terminal names its project too, before the job.

## [0.10.4] - 2026-10-25

### Removed

- **The question card over the terminal.** When an agent asked something,
  Maestro floated its own card with the question and buttons over the
  terminal (and a small answer box under the terminal in the Grid). It is
  gone, with its Settings toggle and the Alt+1..9 and Alt+A shortcuts: the
  agent asks in its own terminal and you answer there. Grid cards of the
  other agents still answer with a click, and the agent that needs you
  still moves to the top of the list.

## [0.10.3] - 2026-10-25

### Changed

- **The phone dashboard shows the agents that are running, by project.**
  Stopped agents no longer fill the list: they show up again once you resume
  them in Maestro, and Show N stopped brings them back on demand. Agents sit
  under a heading per project (its folder beside it), the projects that need
  you first, and rows no longer jump around as agents go active and idle.
  The terminal tabs leave stopped agents out too.

## [0.10.2] - 2026-10-24

### Fixed

- **Remote works from another network, through Tailscale.** The dashboard
  answered only to 127.0.0.1, localhost and this computer's Wi-Fi address, so
  a phone on 4G or another Wi-Fi coming in through Tailscale (at the
  computer's 100.x.y.z address) was turned away. With Let other devices in
  on, every address of this computer is accepted now. Put Tailscale on the
  computer and the phone, then open http://<the computer's Tailscale
  address>:8477; Settings > Remote says so too.

## [0.10.1] - 2026-10-24

### Fixed

- **Remote: letting other devices in no longer fails with "cannot bind port
  8477".** Turning on Let other devices in while Remote was on restarted its
  server before the old one had let go of the port, so the start failed and
  nothing was left running, and your phone could not reach it. The restart
  now waits for the old server to close (a server other devices could reach
  could also keep its port after being turned off).

## [0.10.0] - 2026-10-22

Agents of every CLI share what they did, and one carries on another's task.

### Added

- **agent_context: every agent can read what the others did.** A new tool of
  the maestro MCP lists the conversations of Claude Code, Codex and OpenCode in
  the project (and its worktrees) for the last 14 days: who, running or
  stopped, the task, how far its plan got. With `who` it gives one agent's
  asks, plan, changed files and last words, so another agent can carry the
  task on. It reads each CLI's own files, so it works for agents run outside
  Maestro too, and while Maestro is closed: any agent with the maestro MCP
  installed can use it (the MCP README has the Codex and OpenCode lines).
  Claude agents in Maestro are told to look at it before they start a task.
- **Hand over to an agent that is already open.** The agent menu offers the
  free agents of another CLI working in the same folder ("Hand over to Bo
  (OpenCode)"), before a new agent. The one taking over reads the first
  one's context itself. A stopped agent has a Hand over button next to
  Resume.
- **An agent that runs out of usage hands its task over by itself**, at
  once: to a free agent in its folder whose plan has room, else to a new
  agent of a CLI whose plan has room. When every plan is out, it waits for
  the reset as before. Settings: Hand over when an agent runs out of usage
  (on by default).

## [0.9.13] - 2026-10-19

Many agents in the Grid, and every one of them readable.

### Changed

- **The Grid is one big terminal and a card for every other agent.** A terminal
  only reads at about 80 columns, so a screen fits two to four of them. The
  Grid now shows the agent you work with as a real terminal, as big as your
  screen allows (its text grows until a line holds about 90 characters), and
  every other agent as a card beside it: its state, its job, what it is doing
  right now and the end of its last reply, in plain text that reads at any
  width. A card that asks you something has Allow / Deny (or its choices) on
  it. Click a card to work with that agent in the terminal. A wide screen gets
  two columns of cards.
- The Grid holds up to 9 agents by default. A saved 4 or 6 is lifted to 9 once;
  Settings still sets it.

## [0.9.12] - 2026-10-19

### Fixed

- **The Grid sizes its terminals right on a 1080p screen.** 0.9.11 guessed a
  terminal line shorter than it is, so a 1920 x 1080 screen got four cards
  whose text then shrank to 14px. It now shows two at full size there (about
  82 x 38 characters each); a 2560 x 1440 screen still shows four at 18px.
- When the list steps aside for the Grid, its note says Ctrl+K swaps in another
  agent, instead of pointing at the list you can't see.

## [0.9.11] - 2026-10-19

The Grid you can read, and Resume that picks up where you left off.

### Changed

- **The Grid shows only as many terminals as you can read.** It used to split
  the screen evenly between up to six agents, which left each one about 40
  columns on a laptop and 75 on a 27" screen. Now it shows as many as fit at
  80 x 20 characters or more, in the arrangement that leaves them the most
  room, and gives their text a bigger font when there is space to spare. The
  others wait in the list: pick one to swap it in. On a screen where not even
  two fit beside the list, the list steps aside while the Grid is on.

### Added

- **A Resume button on every stopped agent**, in its row of the list and on
  its stage, instead of only in the right-click menu.

### Fixed

- **Resume carries on the conversation the agent was last in.** After a
  /clear (or switching conversation inside Claude Code), Resume, and a restart
  of the computer, brought back the conversation the agent was first started
  on. Maestro now follows the conversation each Claude agent is in and saves it.

## [0.9.10] - 2026-10-15

Maestro stops working the graphics card when you are not looking at it.

### Fixed

- **Much less GPU while idle.** A minimized Maestro now tells its view it is
  hidden, so it stops drawing; running agents that are not in focus no longer
  re-blur the wallpaper on every terminal update, and the status dots stopped
  breathing. Measured on a laptop: about 63% GPU visible and 45% minimized
  before, about 4% visible and 0% minimized after.

## [0.9.9] - 2026-10-13

The stage is the terminal again, and the Grid shows only agents that run.

### Changed

- **The Grid leaves stopped agents out.** An agent that stopped or exited no
  longer holds a tile; resume it from the list and it comes back into the Grid.
  With fewer than two running, the Grid tells you to resume one or start another.
- **The agent on the stage is always its real terminal.** The chat view could do
  much less than the CLI itself, so it is gone, with its Chat / Terminal switch.
  History still reads every earlier conversation back.

### Removed

- **Talk to Claude directly** and **Show one agent as a chat** in Settings.
  Claude agents that had the first on start in their terminal as before.

## [0.9.8] - 2026-10-04

Talk to Claude directly (new, opt-in).

### Added

- **Talk to Claude directly.** Turn it on in Settings, Inbox. Claude agents then
  take messages straight instead of having them typed into their terminal:
  - nothing gets stuck in the CLI's box, and messages sent while it works wait
    their turn;
  - the reply shows as it is written;
  - permissions and questions come as buttons, answered exactly (Allow, Always
    allow here, Deny or your own words; each option of a question);
  - Stop stops it without losing the conversation;
  - the / list opens at once with every command and skill it has, and models
    and permission modes switch without its terminal.

  Its Terminal tab shows a log of the exchange. It applies to an agent the next
  time it starts; off, everything works as before.

### Fixed

- **Hooks never open windows** for an agent talked to directly: it runs with a
  hidden console of its own, so the console programs it starts share it.
- **The phone's remote terminal** shows such an agent as readable text, and
  stray keys typed there can't break it.

## [0.9.7] - 2026-10-04

Read back earlier sessions.

### Added

- **Sessions: every earlier Claude conversation of a project, to read back.**
  Open it from the clock button on the dock, Alt+O or Ctrl K. It lists the
  project's sessions newest first, its agents' worktrees included (or every
  folder's), and searches them by title or folder. Pick one and it shows as
  the chat does: messages, steps, diffs, pictures. It is only for reading,
  nothing starts or resumes an agent. A long session opens at its end, with
  a button to read all of it.

## [0.9.6] - 2026-10-04

Know when agents finish, and lock the dashboard.

### Added

- **Maestro tells you when an agent finishes:** done, ready to review, or
  stopped with an error. Several at once come as one notification, and
  clicking it opens that agent. Short runs and the agent you are looking at
  don't notify. Turn it off in Settings, Inbox.
- **The phone dashboard shows who is done** and can notify you there too.
- **A password for the dashboard.** Set it in Settings, Remote; other devices
  then sign in first. Without one, the dashboard stays on this computer.
- **Replies show as soon as the terminal has them,** without waiting for the
  conversation file. (Claude Code itself shows a reply only once it has
  written it, so the chat now keeps pace with the terminal.)

### Security

- **Only the dashboard's own page gets in.** Another website can no longer
  reach the phone dashboard through your browser (by re-pointing its name at
  this computer, or from a dev server on another port) to read your agents or
  type into them.
- **Guessing the dashboard password is slower,** even with many tries at once,
  and the password is now at least 10 characters.
- **Signing out closes open terminals,** as do changing or removing the
  password.
- **Removing the password stops the LAN server first,** so it is never open
  to other devices without one.
- **A plain warning next to the LAN toggle:** on your Wi-Fi the password goes
  unencrypted. See [docs/remote-dashboard.md](docs/remote-dashboard.md).

## [0.9.5] - 2026-09-30

A smoother chat.

### Fixed

- **Picking a command keeps what you were writing.** "/ Commands" replaced your
  draft; the command now goes first and your words stay after it, as in the
  terminal. Edit and the starter jobs no longer wipe a draft either: after Edit,
  Down brings it back.
- **"/ Commands" opens at once.** It used to open 10 to 20 seconds late, over
  whatever you were doing; now it opens on the click and fills in.
- **A new agent gets its job.** The job typed into a new agent sometimes sat
  unsent in its box while Claude was still starting. Maestro now keeps pressing
  Enter until it takes.
- **A message that didn't go says so,** with "Press Enter again", instead of
  "Sending…" until it quietly disappeared.
- **Line breaks in replies stay,** so a list of lines no longer runs together.
- **"!" commands read as in the terminal:** your command, then what it printed,
  not raw tags.
- **The agent you start is the one you see,** and one that finished says "Done"
  instead of "Waiting for a task".
- **An agent that fails to start says why** instead of "Loading the
  conversation…" for good.
- **Typing Vietnamese:** the Enter that picks a word no longer sends the message.

## [0.9.4] - 2026-09-29

Updates come from a new place.

### Changed

- **Maestro now updates from github.com/tdat-dev/maestro-releases.** Earlier
  versions look for updates somewhere they can no longer reach, so install this
  one by hand, once, from that page. After that Maestro updates itself again.

## [0.9.3] - 2026-09-29

### Fixed

- **Each agent uses the browser of the Maestro that started it.** Before, agents
  went to whichever Maestro was opened last, so with a second Maestro open they
  found no browser.
- **A test build of Maestro leaves your browser alone.** It no longer takes over
  the browser connection of the Maestro you use (set MAESTRO_DEV_BROWSER=1 when
  you do want it to).

## [0.9.2] - 2026-09-29

### Fixed

- **Agents can use the browser again after a second Maestro closes.** With two
  Maestro windows open (or a test build), closing one left agents pointed at it,
  so every browser tool said Maestro wasn't open until you restarted Maestro.
  The one still open now takes the browser back within a few seconds.

## [0.9.1] - 2026-09-29

### Fixed

- **An agent leaving its worktree never touches the project folder.** When the
  folder's path was written another way (a short name, another case), Maestro
  could take the project folder itself for the worktree to remove. Git refused,
  but the agent was left with an error.

## [0.9.0] - 2026-09-29

Merge in one click, comment on the diff, and a smoother app.

### Added

- **Merge in one click.** In Changes, Merge into main (or press M) commits what
  the agent left, merges its branch and removes its worktree. When the merge
  would conflict, nothing is merged and the files are named; Send back asks the
  agent to merge main in and resolve them. Discard branch drops the work after
  asking. Merged agents show as Merged in the list, with a button to clear them.
- **Comments on the diff.** Click a line number in Changes (Shift-click for a
  range) to leave a comment. Send back types them all to the agent in one
  message, each with its file, line and code.

### Fixed

- **A new agent in a new worktree no longer closes itself.** Its job was typed
  while Claude asked whether to trust the folder, with "No, exit" selected. The
  job now waits for your answer.
- **An agent's own git commits no longer fail with "index.lock exists".**
  Maestro looks at what changed without touching the agent's git index.
- **Messages between agents keep arriving.** They stopped for good once the
  message file passed 2 MB, and a message could arrive twice or cut in half.
- **An agent that fails at start shows as stopped** instead of running forever.
- **The send button stays in the message box** when the chat is narrow, and
  Changes keeps its buttons in view on a small window.

### Changed

- **Smoother with many agents.** Starting one agent no longer makes typing lag
  in the others; copying or deleting a big folder no longer freezes the window;
  a long chat redraws only what changed; Codex and opencode chats stop searching
  for their file every second; a stopped agent frees its terminal and memory; and
  the app does less while its window is hidden.

## [0.8.1] - 2026-09-28

Opening a chat is quiet again.

### Fixed

- **No terminal window when you open a chat.** Maestro asks Claude Code
  for its commands in the background, and every hook you have set up in
  Claude Code opened a window of its own while it did. It now asks with
  hooks off, and the list is ready in about half the time.

## [0.8.0] - 2026-09-27

Limit Guard: no agent left sitting at its usage limit.

### Added

- **Agents that run out of usage say so.** When Claude or Codex stops at its
  5-hour or weekly limit, its row reads "Out of usage · back 7:09 PM" and you
  get a notification.
- **They carry on by themselves.** When the limit resets, Maestro tells the
  agent to carry on where it left off. Turn it off in Settings → Inbox.
- **Hand a task to another CLI.** From an agent's menu, or the chat of one
  that ran out, start Codex, opencode or another CLI in the same folder. It
  gets a brief of what the first agent was asked, its plan, the files it
  changed and where it stopped, so it carries on instead of starting over.
- **A warning at 90%.** A notification when Claude's or Codex's limit passes
  90%, and New agent says when the CLI you picked is nearly out and which one
  has room.

### Changed

- **Lighter chats.** A long conversation opens on its latest part (the last
  4 MB) instead of reading all of it; the rest loads when you ask.
- **Lighter terminals.** A terminal out of sight for 5 minutes keeps 1,000
  lines of history instead of 5,000.

## [0.7.3] - 2026-09-27

Every agent has a face.

### Changed

- **Pixel faces instead of letters.** Each agent now has a small pixel face
  of its own, in its CLI's colour, so Ana and Alex no longer both wear an
  "A" and a row of Claude agents is no longer a row of the same orange
  square. The same agent always has the same face; renaming it gives it a new
  one. The status dot stays in the corner.

## [0.7.2] - 2026-09-27

Usage for every CLI, and lighter on your machine.

### Added

- **Usage for every agent, not only Claude.** Codex shows its tokens and how
  full its context is, opencode what it has cost (its own figure), Gemini and
  Qwen their tokens, grok its context. Any other CLI shows what its own status
  line says, such as Aider's session cost.
- **Codex's limits beside Claude's.** Its 5-hour and weekly windows, as its
  last run was told, under the agent list.
- **A nearly full context shows on the agent's row** ("86% context").

### Changed

- **Lighter.** Usage is read only from what an agent has added since the last
  look (a 240 MB conversation: a third of a second once, then next to
  nothing). The chat no longer starts a second copy of the CLI while an agent
  is starting, and only the last few conversations you looked at stay in
  memory.

## [0.7.1] - 2026-09-27

Your Claude limits, always in sight.

### Added

- **5-hour and weekly limits under the agent list.** How much of your Claude
  plan's 5-hour window and week is used, when each resets, and a model's own
  weekly limit once you use it. The bars turn amber at 70% and red at 90%.
  The numbers are the same ones Claude Code's /usage shows; checking them
  costs nothing and nothing but the percentages leaves your machine's
  sign-in. They refresh every 2 minutes and when you come back to Maestro.

## [0.7.0] - 2026-09-27

Answer from the chat, see what each agent costs.

### Added

- **Answer a question in the chat.** When an agent asks you something, its
  options in the conversation pick with a click, the same as in its terminal.
- **How full the context is.** The message box says it as a share ("62%
  context"). Past 80% it turns amber and a Compact button sums the
  conversation up before the CLI has to do it mid-task.
- **Write to a stopped agent.** The message box stays when an agent stops:
  what you send starts it again on the same conversation, then goes in. If it
  can't start, your message goes back in the box.
- **Edit or copy a message you sent.** Edit puts it back in the box to change
  and send again.
- **What each agent has cost.** Each Claude agent's row shows what its
  conversation has used at API prices, and so does the chat's Session panel.

### Fixed

- **Usage counts each reply once.** Claude Code writes a reply over several
  lines, and the Usage window counted every one of them (over twice too
  much). It also priced every Opus at the old $15/$75; it now uses each
  model's current price.
- **A stopped agent's old question** no longer says it is waiting for you.

### Changed

- Releases build for Windows only; the macOS builds never shipped.

## [0.6.5] - 2026-09-26

Bigger terminal text.

### Changed

- **Terminals read bigger.** The default text size is 15 instead of 13, and a
  short tile no longer shrinks the text to squeeze in more lines. Settings and
  Ctrl+wheel still make it bigger or smaller.

## [0.6.4] - 2026-09-26

The agent list stays put.

### Changed

- **The list keeps its order.** Projects stay in the order you opened them and
  agents in the order they were made; clicking or answering an agent no
  longer re-sorts everything. Who needs you is an amber row where it is, and
  a count on its project's name.
- **Projects fold.** Click a project's name to fold its agents away; it stays
  folded, and still shows how many need you.

## [0.6.3] - 2026-09-26

The chat grows up: a work panel beside the conversation, files in every way
you'd expect, and a working line that says what the agent is doing.

### Added

- **A work panel beside the conversation.** Changes (new and edited files,
  the picked one's diff large), Background (what the agent left running, with
  its output) and Session (plan, model, earlier conversations). Drag to
  resize, hide it, and on a narrow window it slides over the chat.
- **@ picks a file or folder** of the agent's folder as you type.
- **Files in by drop, paste or the + button.** Pictures attach with a
  thumbnail; other files go in as @path. Files copied in Explorer paste too.
- **Questions the agent asks you** show as a card in the chat, with the
  options and, once you answer, what you picked.
- **Ctrl+F** finds in the conversation; **Copy** on code blocks; file names in
  answers open in the code panel.
- Up in an empty composer brings back what you sent; drafts stay per agent.

### Changed

- **The working line says what the agent is doing**: the step it runs, or
  what Claude Code shows ("Pondering… 42s · 1.2k tokens"), and for how long.
- What you send shows at once, until the conversation has it.
- Reading further up, new messages no longer pull you down; a button says
  they came.

### Fixed

- A long conversation opens once, at the bottom, instead of scrolling in
  slice by slice.

## [0.6.2] - 2026-09-25

Agents work in your project folder, and the list says which project each one is in.

### Changed

- **Agents work in the project folder.** A git worktree and branch per agent
  (the maestro/... branches) is now off by default; turn it on in Settings >
  Agents if you want it. An agent made before leaves its worktree on its next
  start when the worktree holds no work of its own, and carries its
  conversation over.
- **Agents listed by project.** Each project is a group with its name over its
  agents, the project with an agent that needs you first. The "All projects"
  chips are gone; an agent's state is a dot on its mark and its second line.

## [0.6.1] - 2026-09-25

The chat view does what the agent CLIs do, for Codex and opencode too.

### Added

- **Chat for Codex and opencode agents.** Their conversations read like a
  Claude agent's: what you asked, what they said, each command and edit with
  its result and diff.
- **Effort and permissions in the composer.** Beside the model: how hard the
  agent thinks, and what it may do without asking (Ask first, Accept edits,
  Plan, Bypass…), changed the way the CLI itself does. A mode that never asks
  is tinted.
- **In the background.** The side panel lists the commands an agent left
  running (a dev server, a build, a watch): running, done, failed or stopped,
  with their output a click away.
- **Paste pictures into the chat** with Ctrl+V; click one to see it full size
  before sending.

### Changed

- **Files changed says what is new.** New files and edited ones are listed
  apart, newest first; a dot marks what the latest reply changed; click a file
  to see its diff right there.
- Small pictures in the conversation grow to a readable size.

### Fixed

- Messages you type in the terminal while an agent is working now show in the
  chat.
- Resume a conversation from another project: Maestro starts the agent in that
  conversation's folder instead of Claude saying it is from a different
  directory.
- Maestro accepts the Chrome Web Store build of Maestro for Chrome.

## [0.6.0] - 2026-09-25

A new face for Maestro, agents you can read like a conversation, and agents
that can use your real Chrome while you watch.

### Added

- **Agent Inbox.** The list on the left says who needs you, who is working and
  who has stopped; the stage shows one agent at a time (Focus) or several side
  by side (Grid, up to nine). Right-click an agent for its actions; Ctrl+K jumps
  anywhere; Ctrl+, opens Settings.

- **Chat view.** In Focus, a Claude Code agent reads as a conversation: what you
  asked, what it said, and each step it took, folded to one line until you open
  it. Pick the model and the command from its own CLI, resume, start a new
  conversation, and see the screenshots and images it looked at, full size on a
  click.

- **Maestro for Chrome.** A Chrome extension (Edge and Brave too) that lets your
  agents use your real browser, signed in as you. Each agent works in its own
  tab group, named after it:
  - its cursor glides and clicks on the page, with its name, and a pill says
    "Ana is using this tab" with a Stop button;
  - Maestro shows a live view of the agent's tab, sharp when enlarged, with Stop;
  - clicks that send, post, pay or delete wait for your OK in Maestro;
  - on a login page or a CAPTCHA the agent stops and lets you handle it;
  - agents get browser tools through maestro-mcp: open pages, read them, click,
    type, fill forms, upload files, read the console and network log, answer
    dialogs, record a GIF, resize the window.

  Settings → Browser lists every Chrome, Edge and Brave profile on the computer
  and walks you through adding the extension to one.

### Changed

- **One Cinema look** under every surface: dialogs, menus, tips, toasts, the
  file tree and the tool panels. The app's own menus and drop-downs replace the
  Windows ones.
- The Maestro M logo and its launch animation are back.

### Fixed

- Enter on Cancel no longer confirms a dialog, and every dialog keeps the
  keyboard until it closes.
- Agents start in a folder Maestro picks and records, so Resume finds their
  conversation again; a project whose folder is gone no longer runs agents
  somewhere else.
- The Grid header no longer jumps when you switch to it.

## [0.5.9] - 2026-09-05

The command bar turned into a place you can actually read the conversation, and
the loose buttons around it collected into one.

### Added

- **The Fleet Console.** Focusing the bottom bar (or tapping its grip) slides a
  thread up out of it: what you broadcast and what the Director handed to which
  agent, as chat bubbles — yours on the right, agents' on the left with their
  avatar — over a live members strip that flags who needs you. Click a bubble to
  jump to that agent; a badge counts what arrived while it was closed. Built on
  the hand-off log that was already being kept, so it costs nothing when shut.

- **One "+" for the composer's actions.** The floating gear and the +Agent pill
  are gone; a single button at the left opens one menu — New agents, Settings,
  Clear conversation.

### Changed

- **The composer is one bar now, not a row of pills.** A single glass surface
  with the focus ring around the whole thing, every control a quiet ghost on it
  and Send the one accent disc; the target chip sits inside it. The thread,
  grip, bubbles and members strip were restyled to match — a tighter dark
  system that keeps the canvas-first identity.

## [0.5.8] - 2026-09-05

Finding the right agent — and telling it apart from the other nine — when the
canvas is full. Three pieces of the same problem: knowing which agent you're
looking at, and saying who a message is for.

### Added

- **A fleet switcher (Ctrl+K).** One shortcut opens a palette of every agent in
  every workspace, grouped by whether it needs you, is running, or is idle, each
  row carrying its name, CLI, folder or branch, and a plain-word status (never
  colour alone). Type to filter by name, CLI, workspace or folder; move with the
  arrows; press Enter or Ctrl+1–9 to jump straight there. The agent you're
  looking at is marked "On stage".

- **A target chip on the command bar.** The cryptic "@name to target one" hint is
  now a real chip: it shows Fleet by default, or the one agent you picked, and
  opens a list of everyone with their live status. A plain message goes where the
  chip points; an @mention in the text still overrides it per line.

### Changed

- **A pane opened large now says whose it is.** The focused stage carries a
  bigger avatar with a live status ring, the name at reading size, and a
  workspace · branch subtitle. The "Others" rail beside it is sorted so an agent
  waiting on you rises to the top, counts the fleet, and has its own way into the
  Ctrl+K switcher.

### Fixed

- **Jumping to an agent while another is on stage now works.** Revealing a pane
  in focus mode swaps the stage to it instead of focusing a hidden terminal that
  never comes forward — the message-flow and fleet views gain the same fix.

## [0.5.5] - 2026-08-02

Hand-offs between agents were arriving late, glued together, or with pieces
missing. Watching a real fleet for an afternoon turned up two separate faults
behind it, both in the last inch of the delivery — the part that types into
another agent's terminal.

### Fixed

- **A hand-off no longer sits in the target's composer waiting for someone to
  press Enter.** Any message past one chunk arrives as a burst, and the Enter
  that follows lands while the TUI is still assembling it, so it reads as a
  newline in the text rather than a keypress. The message then waits — until
  the next short message's Enter submits both glued together, in the wrong
  order. Maestro now looks at the composer after pressing Enter and presses
  again while the message it just typed is still sitting there. It only ever
  re-presses for text that matches what it typed: the screen it reads has no
  colour, so a dimmed history suggestion in an empty box looks exactly like
  typed text, and submitting that would put words in your mouth. Works for
  Claude Code's ruled input box and for boxless prompts like Codex.

- **A long hand-off arrives whole.** Chunked typing was not only losing the
  Enter, it was losing content. Measured against a live pane with a 200-line,
  10k-character message: 93 lines arrived, the rest gone in whole runs of one
  to two kilobytes, with one surviving line spliced mid-word. The bytes are not
  lost in transit — a probe recording its own stdin received all 10,000 of
  them — it is the TUI inferring where a paste begins and ends from timing
  alone. Maestro now marks the block the way a terminal does for a real paste,
  and only for a program that asked for that mode, so a plain shell still gets
  plain text. Same message, same pane: 200 of 200 lines.

## [0.5.4] - 2026-08-01

One working language for the fleet, and the last hand-off surface that still
typed a message the way 0.5.3 stopped typing it everywhere else.

### Changed

- **Agents hand work to each other in English.** A `fleet_send` message is read
  by an agent, not by you, so writing it in your language bought nothing and
  cost a translation at both ends — in a mixed fleet where not every CLI reads
  Vietnamese as well as it reads English. The rule now travels with the agent:
  the spawn-time laws for both worker and Director, the MCP server
  instructions, and the `fleet_send` tool description. You are still answered
  in the language you used.

### Fixed

- **A message sent from the phone dashboard no longer sits unsent in the
  composer.** The dashboard was the last sender still writing the whole message
  and its Enter in a single write — the case 0.5.3 fixed for every other
  hand-off. It now types in chunks and sends Enter on its own. Raw keys
  (arrows, Esc, Ctrl+C) still go straight through, which is what they are for.

## [0.5.3] - 2026-07-31

The canvas becomes a place agents talk to each other: a Director pane that
dispatches work to the agents already open, and a Flow panel that keeps every
hand-off readable instead of flashing it for 2.6 seconds.

### Added

- **Director agent** — spawn one from the **+Agent** menu. It reads the fleet
  roster and hands tasks to the agents already open rather than spawning its
  own crew, so you decide who is in the room.
- **Flow panel** — every message that moves between agents is kept with its
  sender, its targets and its full text, with an unread badge on the rail. The
  delegation arc and toast are still there; they are now the notification, not
  the record.
- **Fleet sigil** — an orrery drawn under the panes that tracks the fleet.
- **Wallpaper and sigil apply to all of Maestro**, not just the canvas.

### Fixed

- **A message handed to an agent is no longer swallowed by its composer.** The
  whole message plus Enter was written to the terminal in a single write, which
  Claude Code's TUI read as a paste: it kept only the tail of the text and
  treated the trailing carriage return as a newline inside the composer, so the
  message sat there unsent. Messages are now typed in chunks and Enter is sent
  on its own once the composer has settled, one message at a time per agent.
- **A broadcast no longer dispatches back to the sender.** A Director
  broadcasting an order typed that order into its own pane, read it as a fresh
  instruction, and answered its own dispatch.
- **Tidy and the zoom pill** no longer sit on top of the canvas.

## [0.5.2] - 2026-07-25

### Fixed

- **Tiled panes show real output again.** With three or four agents on screen,
  each pane rendered about twelve terminal rows — and an agent CLI spends
  roughly seven of them on its own prompt box and status line, so almost no
  conversation was visible. Tidy tiling reserved space for the command bar a
  second time when the canvas box had already excluded it, costing about 84px
  (three rows) per row of tiles, and the terminal's inset was wider than it
  needed to be.
- **Pane text shrinks instead of clipping when a tile gets short.** A pane below
  twenty rows now steps its font down to fit, never more than 4px under the size
  set in Settings and never below 12px, so a crowded canvas stays readable
  without overruling the preference.
- **Agents spawned from the command bar honour the saved permission mode.** The
  inline +Agent menu always booted them in manual mode, even right after the
  spawn modal launched one with permissions skipped. The menu now carries the
  toggle itself, shared with the modal and the wizard, and applies each CLI's
  own flag (Claude Code, Codex, Gemini, Qwen, Aider, Copilot).

## [0.5.1] - 2026-07-23

### Fixed

- **Terminal panes no longer cut off their last line.** Every pane sliced its
  bottom row in half — in a Claude Code agent that was the status line
  (`auto mode on · 13 agents`) permanently chopped. The pane's inset sat on the
  terminal's container, where xterm's fit addon can't see it, so the addon kept
  proposing about two more rows and columns than actually fit and the overflow
  was clipped away. The inset now lives on the terminal element itself, where
  the fit maths accounts for it. The same fix applies to the remote dashboard.
- **The top of a pane no longer erases output.** The fade under the title bar
  was sized as a percentage of the pane, so on a normal pane it swallowed
  roughly two full lines of text. It now spans only the padding strip.

## [0.5.0] - 2026-07-23

A workspace redesign: agents now live on a free-form canvas instead of a rigid
tiling grid, and the controls that used to crowd the top bar have moved into a
single command bar at the bottom of the screen.

### Added

- **Canvas workspace** — agent panes are now positioned freely: drag a pane by
  its title bar to move it anywhere, and hit **Tidy** to tile everything back
  into a clean grid that fills the screen. Positions persist per workspace.
  Every agent gets a persona name and a colour it keeps across the pane header,
  the focus rail, and `MAESTRO_AGENT`, so an agent is recognisable at a glance.
- **Focus stage** — zoom a pane to fill the workspace; the other agents collapse
  into a slim avatar rail down the right edge (each with a live/idle/needs-you
  dot) so you can jump between them without leaving focus. The stage grows out
  of the point you clicked, bumps the terminal font, and dims the canvas behind
  it. The zoom button becomes a back arrow to return.
- **Command bar** — one bar at the bottom holds settings, the fleet message
  field, and **+Agent**. Typing `@` opens a picker of your agents, and a single
  line can address several at once (`@Ana run tests @Bob deploy`). **+Agent**
  opens an inline spawn menu — pick how many of each CLI (plus a custom command)
  and spawn straight into the current workspace, without the full modal.
- **Voice input** — push-to-talk in the command bar. The **voice → dispatch**
  panel splits what you said by agent name, so one spoken sentence can hand a
  different task to each agent; review the split before it sends.
- **Delegation visualization** — when an agent hands work to another via
  `fleet_send`, Maestro draws a transient animated link between their panes and
  pops a toast (`Ana → Bob: …`), so hand-offs are visible even when the panes
  involved are off-screen or in another workspace.
- **Full-screen Settings** — reorganised into **Appearance**, **Fleet & Remote**,
  **Schedule & Sessions**, and **System**, with a canvas **background picker**
  (presets, a solid colour, or your own image, saved per workspace).
- **Hint bar** — a top-centre pill for first-run tips and confirmations (e.g.
  how many panes Tidy just arranged).

### Changed

- **Broadcast defaults to the whole fleet.** The per-agent target dropdown is
  gone — it sat empty and inert whenever no agent was running. A message with no
  mention now reaches every running agent, and `@name` targets one.
- **Pane headers are down to three controls** — rename, zoom, and kill — with
  search, record, and restart moved behind a **⋯** overflow. The agent name is
  editable in place (click it or the pencil).
- **Slimmer top bar** — the brand mark is gone (Home anchors the left) and
  "Resume all" is now an accent pill. The settings gear that duplicated the one
  in the command bar has been removed from the window controls.
- **Internal restructuring** — `main.ts` went from 2035 to 496 lines and
  `workspace.ts` from 674 to 474, with panes, canvas layout, broadcast, spawn,
  settings, replay, dashboard, and shared state split into focused modules. No
  behaviour change; it just makes the app far easier to work on.

### Fixed

- **`@` now lists idle agents too.** The mention picker only offered agents with
  a live process, so an agent restored from a session or one whose CLI had
  exited never appeared — even though its name is still a valid target.
- **Transcript saving in agent panes.** Launching Maestro from inside a Claude
  Code session leaked that session's markers into every agent it spawned, so
  each agent believed it was a child session and silently stopped saving its
  transcript. Those markers are now stripped from each agent's environment.
- **A dev build can no longer kill your installed Maestro.** Development and
  production both produced `maestro.exe`, differing only by path, so anything
  matching processes by name while developing could take down the real app. The
  dev build is now `maestro-dev.exe`; the shipped binary is unchanged.

## [0.4.1] - 2026-07-16

### Fixed

- **Copy inside Claude Code panes** — highlighting text in a Claude Code agent
  pane now actually copies it. Claude Code's TUI captures the mouse and copies
  through the OSC 52 escape sequence, which Maestro's terminal silently dropped;
  Maestro now handles OSC 52 and writes the text to the OS clipboard. The
  clipboard *query* form is deliberately ignored so a program in a pane can't
  read your clipboard. Copy-on-select in plain shells is unchanged.

## [0.4.0] - 2026-07-13

### Added

- **Token usage & cost** — Settings → **Token usage & cost** shows a per-model
  breakdown of tokens (input / output / cache) and an estimated USD cost for the
  current workspace. The numbers are *real* — read from Claude Code's own
  session transcripts (`~/.claude/projects/<slug>/*.jsonl`), not scraped from the
  terminal — so they're accurate, but Claude-only and per-workspace (a transcript
  folder isn't split per Maestro agent). Prices are an approximate estimate for
  the Opus/Sonnet/Haiku families; other models show tokens without a cost.
- **Session replay** — every pane header has a **●** record button: hit it to
  record that agent's terminal to `<workspace>/.maestro/recordings/*.jsonl`
  (raw output frames with millisecond timestamps, starting from a snapshot of
  the current screen), hit it again to stop. Settings → **Session replays** (or
  the "Recording saved" toast) opens a player that replays the session in a real
  xterm.js terminal with the original timing — play/pause, a scrubbable
  timeline, and 1×/2×/4×/8× speed. Seeking rewinds and re-renders from the start
  so the screen is always exactly what the agent showed at that moment.
  Recording auto-stops (and flushes) when the agent exits or is killed.
- **Conductor mode** — the spawn modal has a **Conductor** toggle that turns the
  first agent into a conductor (it does *not* add an extra agent): it directs the
  crew instead of coding — plans the board, spawns & assigns workers with
  `agent_spawn`/`fleet_send`, watches them with `agent_output`, marks cards done.
  It uses the **same CLI as your first pick** (pick Codex → a Codex conductor,
  not a hardcoded Claude). Pick 0 workers to run just the conductor and let it
  build its own crew. Off by default.
- **Conductor tools for agents** — maestro-mcp gains `agent_output` (read another
  agent's on-screen text to check its progress) and `agent_spawn` (ask Maestro to
  boot new worker agents into the same workspace, with an optional task). Combined
  with the board and fleet tools, one agent can now act as a conductor: read the
  board, spawn a crew, hand out work, watch each worker's screen, and mark cards
  done. Maestro publishes each agent's screen into `.maestro/fleet.json` and
  watches `.maestro/spawn-requests.jsonl` for the spawn requests.
- **Full web terminal in the remote dashboard** — tapping an agent now offers
  *Open full terminal*: a real xterm.js terminal streamed from the app over
  Server-Sent Events, with keystrokes (including arrows, Enter, Ctrl+C, and
  editing) sent straight to the agent's PTY. Type directly and interactively
  from your phone — no key buttons needed — the same as sitting at the machine.
  xterm.js is bundled and served same-origin so it works offline/LAN. Output is
  streamed over a **WebSocket** (on the dashboard port + 1) with TCP_NODELAY, so
  typing and deleting feel near-direct — bidirectional, ordered by nature, no
  per-keystroke HTTP round-trip. (An earlier SSE + POST transport was too laggy:
  tiny_http buffers streaming responses and can't set TCP_NODELAY.) The terminal
  view has a **tab bar** of all agents (tap to switch without going back to the
  roster) and a **Grid** toggle that tiles every running agent's terminal so you
  can watch many at once and tap one to open it full.

- **Board ⇄ Agent loop** — kanban cards can be dispatched to a specific
  running agent ("Send to agent…" in the card detail, or drag the card onto a
  pane): the agent receives a structured prompt, the card records its
  `assignee` and jumps to Doing, and the card shows a clickable agent chip
  that focuses the pane. Maestro sets `MAESTRO_AGENT` on every spawn so
  maestro-mcp records who moved/finished each card, and the app toasts (+ OS
  notification when unfocused) when an agent lands a card in Done.
- **Right-click copies the terminal selection** — copy-on-select can lose the
  clipboard write when another Windows process holds the clipboard lock;
  right-clicking a selection retries the copy.
- **Fleet monitor** (`Ctrl+Shift+L`) — a dock panel listing every agent across
  all workspaces with live status (needs-you / working / idle / stopped),
  uptime, and owning workspace; click to jump to any agent, and a rail badge
  counts how many are waiting on you.
- **Fleet coordination for agents** — maestro-mcp gains `fleet_status` (see the
  other agents and their status) and `fleet_send` (message another agent, or
  broadcast), delivered through a `.maestro/fleet.json` + `outbox.jsonl` file
  bridge so agents can hand off work to each other.
- **Remote fleet dashboard** — Settings → *Remote fleet dashboard*: Maestro
  serves a small web page (default localhost, LAN opt-in) showing every agent
  and its status. Tap an agent to see a live tail of its terminal output (read
  straight from the rendered screen, so a TUI's spinners don't spam) *and* drive
  it: a row of key buttons (up/down/Enter/Esc/Tab/^C) sends raw keys so you can
  navigate an interactive menu like `/resume` from your phone, plus a message
  box for text. The LAN toggle is off by default since the page can drive an
  agent.
- **Scheduled agents** — Settings → *Manage schedules*: launch a saved crew
  preset automatically at a set time, once or daily. Each schedule shows its
  next run and can be paused or deleted.
- **Conductor** — an in-app auto-dispatch scheduler on the board toolbar,
  cycling Off → Semi → Auto → Pipeline. *Semi* hands each free agent the next
  approved ("To do") card automatically; *Auto* also promotes cards from
  Proposed to keep agents fed; *Pipeline* flows each card through Build → Test
  → Review → Done, handing every stage to a free agent with a stage-specific
  prompt and detecting each hand-off. Deterministic (no extra LLM cost — the
  agents do the work), scoped per workspace, click through to Off to stop.

### Changed

- **Maestro enforces its board protocol on Claude agents** — every Claude Code
  agent Maestro spawns is launched with `--append-system-prompt` carrying the
  plan-first rules, so it *must* plan on the board (call `board_get`, add cards,
  report via `card_move`/`card_done`) rather than treating the MCP hint as
  optional. No button, no terminal noise; other CLIs still get the MCP tools
  and server instructions.
- **Board toolbar simplified** — the maestro-mcp server now ships the plan-first
  convention (few big cards, small steps as each card's checklist, report via
  `card_move`/`card_done`) as its MCP instructions, so an agent with the board
  tools plans and adds cards directly. The **Plan with AI** button (which used
  to drop a rules file and prime the agent) is therefore gone, alongside the
  earlier **Import** and **Send approved** removals — the board toolbar is now
  just the Conductor and a compact Capture-web icon.

### Fixed

- **MCP wrote to the wrong project's board** — maestro-mcp resolved the board
  from the agent's current directory, so an agent whose cwd drifted (a `cd`, a
  subdir, a git worktree) could write cards into a different project's board.
  It now prefers the `MAESTRO_WORKSPACE` env Maestro sets on every spawn.

## [0.3.9] - 2026-07-09

### Removed

- **Merge button** on the topbar in detached windows (workspace fold-back still
  works via drag-and-drop between windows).
- **Isolate each agent in its own git worktree** toggle from the spawn modal
  and workspace wizard — new agents always run in the project folder.

## [0.3.8] - 2026-07-05

### Added

- **Resume all** — reopening a project parks every agent as *stopped*, and you
  used to have to click ⟳ on each pane to bring the fleet back. A new **Resume
  all** button now appears in the topbar whenever the current project has any
  stopped or exited agents (with a live count) — one click boots them all. They
  start one at a time on purpose, so a large crew doesn't hammer the disk and
  freeze the window the way a parallel spawn would.

## [0.3.4] - 2026-07-02

### Fixed

- **App froze while spawning a crew** — booting several isolated agents at once
  locked up the whole window (and dragged the machine down) until every agent
  had started. All Tauri commands ran synchronously on the main/UI thread, so
  each agent's `git worktree add` (a full repo checkout) and ConPTY creation
  blocked rendering and input. Heavy commands (PTY, worktree, git review, PATH
  probing) now run on a background thread pool, and worktree checkouts queue
  one-at-a-time instead of thrashing the disk in parallel. The UI stays
  responsive for the whole boot.

## [0.3.1] - 2026-06-29

### Fixed

- **Worktree disk bloat** — every git worktree that ran `cargo build` created its
  own `src-tauri/target` (~10 GB for a Tauri build), silently filling the disk.
  New worktrees now get a `.cargo/config.toml` pointing `target-dir` at the main
  repo's target, so all worktrees of a repo share one build cache instead of
  duplicating it.

## [0.2.0] - 2026-06-19

### Added

- **Tool dock** — a permanent icon rail on the right edge of the workspace that
  toggles slide-in panels, so the side tools are always one click (or one
  shortcut) away instead of buried in the top bar. Press `Esc` to close.
- **Kanban board** (`Ctrl+Shift+K`) — a Trello-style board scoped to each
  workspace folder: add/rename/delete/reorder lists, drag cards within and
  across lists, and a card detail view with description, colour labels, due
  date, and a checklist. Drag uses Pointer Events (HTML5 drag-and-drop is
  swallowed by the WebView2 OS drag-drop handler). Boards persist per folder.
- **Pomodoro timer** (`Ctrl+Shift+J`) — a per-folder focus/break timer with a
  progress ring, configurable focus/break lengths, session tracking, and an OS
  notification when a phase ends. A live `mm:ss` badge shows on the rail while
  running, and timers keep ticking across tab switches.

### Changed

- **Git diff review moved into the dock** (`Ctrl+Shift+D`) — the old top-bar
  "AI Code" overlay was redesigned as a dock panel with a cleaner diff view,
  repo selector, branch/worktree badge, and commit / discard / merge actions.
  The top-bar "AI Code" button was removed.

## [0.1.10] - 2026-06-15

### Fixed

- **Killing a maximized pane no longer blanks the whole workspace** — the ⤢
  maximize state (`.has-max` on the grid) was left behind when its pane was
  closed/killed, so `.grid.has-max .pane{display:none}` hid every remaining
  pane with nothing maximized to show. `layoutGrid` now drops `has-max`
  whenever no `.maxed` pane remains.

## [0.1.9] - 2026-06-15

### Fixed

- **Browser zoom no longer breaks the terminal panes** — WebView2's built-in
  zoom (Ctrl+scroll / Ctrl +/-/0) is now blocked. Zooming scaled the whole page
  and corrupted xterm's DOM-renderer glyph metrics, so clicking WebView2's
  "Reset" toast left every pane blank/off-screen. Use Settings → terminal font
  size to resize terminal text instead.

## [0.1.8] - 2026-06-11

### Added

- **AI Code Slice 2** — commit an agent's worktree changes, merge its branch
  into the main repo (`--no-ff`, conflicts abort and report the files), discard
  uncommitted work, and clean up the worktree + branch after a merge.
- **"Needs you" attention alerts** — a running agent whose output goes silent
  flags amber on its pane pill and workspace tab, with an OS notification when
  the window is unfocused.
- **Keyboard shortcuts** — Alt+1–9 focus pane, Ctrl(+Shift)+Tab cycle
  workspaces, Ctrl+Shift+T new workspace, Ctrl+Shift+F find in pane,
  Ctrl+Shift+B broadcast.
- Ctrl+Click on a terminal link opens it in the default browser.
- CLIs not found on PATH are grayed out in the spawn wizard and modal.
- Terminal font size setting (Settings → applies live to every pane).
- README and MIT LICENSE; repository metadata and project logo.
- CI (typecheck, tests, cargo) and tag-triggered signed release workflows.

## [0.1.7]

### Added

- Workspace setup wizard for spinning up fleets of agents.
- Multi-select model picker and reusable presets.
- Floating pane pills for quick navigation between agents.
- Frameless grid layout.

### Changed

- Merge a detached agent window back into the main grid.

### Fixed

- Dev server port mismatch.

## [0.1.6]

### Fixed

- Restore Ctrl+V paste — xterm was swallowing the keydown so the native paste never fired.

## [0.1.5]

### Added

- Tab reorder and rename.
- Detached agent windows.
- Home screen mascot.

### Fixed

- Paste no longer duplicates input; copy-on-select.

## [0.1.4]

### Fixed

- Spawn git with `CREATE_NO_WINDOW` — no more flashing console windows on Windows.

## [0.1.3]

### Added

- Drop a file onto a pane to type its path into that agent's terminal.
- Pretty in-app update toast with download progress.

### Changed

- Performance: dropped redundant repaint-on-modal-open that caused a reflow hitch.

## [0.1.2]

### Added

- "Check for updates" button in Settings.

### Changed

- Stream PTY terminal output as raw bytes instead of a JSON `number[]`.

### Fixed

- Disable xterm WebGL renderer — fixes WebView2 GPU stall.
- Clear WebView2 compositor lag while a modal is open.

## [0.1.1]

### Added

- AI Code review (work in progress): read-only multi-repo unified-diff view wired to an AI Code toggle.
- Mascot, terminal clipboard support, and idle black-screen fix.
- Per-agent worktree isolation on spawn.

## [0.1.0]

- Initial release: spawn and orchestrate fleets of AI agent CLIs with real terminals and real tree-kill.
