// Pane lifecycle: build a pane's DOM, boot/attach/kill its PTY, session
// recording (replay), and the "needs you" attention heuristic. Split from
// main.ts; main-only helpers (error formatting, the live counters, home/tray
// bookkeeping, find-in-output wiring) are injected via configurePane to avoid
// a circular import.

import { mountTerminal } from "./terminal";
import {
  sendInput,
  resizePty,
  killPty,
  spawnPty,
  claudeSessionExists,
  museSessions,
  museSessionExists,
  codexLatestSession,
  codexSessionExists,
  opencodeLatestSession,
  opencodeSessionExists,
  attachPty,
  openExternal,
  worktreeAdd,
  worktreeLeave,
  notifyAgent,
  recordStart,
  recordStop,
} from "./ipc";
import { branchName } from "./worktree";
import { launchSpec } from "./crew";
import { type Pane, type Workspace, type AgentSpec } from "./panetypes";
import { layoutGrid, wirePaneRename, toggleMax } from "./panelayout";
import { saveSession } from "./session";
import { openReplays, REC_DIR_REL } from "./replay";
import { workspaces, newId } from "./appstate";
import { taskLine, taskOf } from "./tasks";
import { getPref } from "./prefs";
import { basename, runDir } from "./workspaces";
import { homeDir } from "@tauri-apps/api/path";

let home: Promise<string> | null = null;
/** The user's home folder, asked once. */
function homeFolder(): Promise<string> {
  home ??= homeDir().catch(() => "");
  return home;
}
import { paneLook } from "./background";
import { MAESTRO_LAWS, DIRECTOR_LAWS } from "./laws";
import { getZoom, setZoom, paneFont, autoFitRows } from "./zoom";
import { openMenu } from "./ctxmenu";
import { cliLogo } from "./clilogos";
import { replayEarlyExit, takeEarlyExit } from "./ptyexit";

/** Re-theme every live pane in `ws` — called when the background, the tone, or
 *  the opacity changes, since all three decide the same palette. */
export function retheme(ws: Workspace): void {
  const look = paneLook();
  for (const pane of ws.panes.values()) pane.term.setLook(look);
}

/** Re-render every pane in `ws` at `next` zoom (or the saved one, when called
 *  with nothing — e.g. after restoring a workspace). Returns the zoom applied.
 *
 *  Order matters: the auto-fit floor is set BEFORE the font, so the new size is
 *  measured under the rule that will govern it rather than under the old one.
 *  See autoFitRows for why zooming stands the floor down. */
export function applyZoom(ws: Workspace, next?: number): number {
  const zoom = next === undefined ? getZoom(ws) : setZoom(ws, next);
  const rows = autoFitRows(PANE_MIN_ROWS, zoom);
  const font = paneFont(ws);
  for (const pane of ws.panes.values()) {
    pane.term.setAutoFit(rows);
    pane.term.setFontSize(font);
    // setFontSize already refits and reports cols/rows through mountTerminal's
    // onResize, which is the path that resizes the PTY — no second call needed.
  }
  return zoom;
}

let onErrMsg: (e: unknown) => string = (e) => String(e);
let onUpdateCount: () => void = () => {};
let onShowWorkspace: () => void = () => {};
let onWirePaneSearch: (pane: Pane) => void = () => {};
export function configurePane(deps: {
  errMsg: (e: unknown) => string;
  updateCount: () => void;
  showWorkspace: () => void;
  wirePaneSearch: (pane: Pane) => void;
}): void {
  onErrMsg = deps.errMsg;
  onUpdateCount = deps.updateCount;
  onShowWorkspace = deps.showWorkspace;
  onWirePaneSearch = deps.wirePaneSearch;
}

const enc = new TextEncoder();

/** Row floor every pane auto-fits to: an agent CLI's own chrome (prompt box,
 *  status line, tips) claims ~7 rows, so this leaves ~13 for real output.
 *  Deliberately modest — the font drop needed to clear a taller floor made the
 *  text too small to read, which is worse than scrolling. */
export const PANE_MIN_ROWS = 20;

const RESTART_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg>';
const KILL_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>';
const SEARCH_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>';
const MAX_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3H5a2 2 0 0 0-2 2v3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M3 16v3a2 2 0 0 0 2 2h3"/></svg>';
// A filled dot — the record button; the ".rec" class pulses it red while active.
const REC_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="12" r="6"/></svg>';
// Back-to-canvas arrow — swaps in for MAX_SVG while a pane is focused (the
// mockup's .stage-back). And a pencil for the on-hover rename affordance.
const BACK_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l-6 6 6 6M3 12h13"/></svg>';
const EDIT_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>';
// "⋯ more" — collapses the app's extra controls (search/record/restart) so the
// header shows the mockup's exact three (rename · focus · kill) by default.
const MORE_SVG =
  '<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="2"/><circle cx="12" cy="12" r="2"/><circle cx="19" cy="12" r="2"/></svg>';

function buildPaneEl(
  id: string,
  name: string,
  _sub: string,
  badge: string,
  color: string,
  role?: AgentSpec["role"],
): HTMLElement {
  const el = document.createElement("section");
  el.className = "pane";
  el.dataset.id = id;
  // The one pane that dispatches work has to be findable at a glance, so the
  // role reaches CSS as a plain attribute. "director" rather than "conductor":
  // the word on screen should be the one the person using this says out loud.
  if (role === "conductor") el.dataset.role = "director";
  // Word, not icon. A badge that only makes sense once someone explains it is
  // decoration; the label is two syllables and needs no legend.
  const roleChip = role === "conductor" ? `<span class="pb-role">Director</span>` : "";
  // Slim draggable title bar: status dot · editable name · CLI badge · controls.
  // Controls keep their data-* attributes so the existing wiring in createAgent
  // still binds. `[data-drag]` on the bar is the canvas move handle.
  el.innerHTML = `
    <div class="pane-bar" data-drag>
      <span class="pb-dot"></span>
      <span class="pb-av" style="--mk:${color}" aria-hidden="true">${cliLogo(badge)}</span>
      <span class="pb-name pane-name">${name}</span>
      ${roleChip}
      <span class="pb-cli">${badge}</span>
      <span class="pb-where" data-where></span>
      <span class="pb-sp"></span>
      <div class="pb-ctrls ctrls">
        <div class="pb-more">
          <button class="pctrl" data-more aria-haspopup="menu" aria-expanded="false" aria-label="More actions" title="More: search, record, restart">${MORE_SVG}</button>
          <div class="pb-more-menu" hidden>
            <button class="pctrl" data-search aria-label="Search output" title="Search output">${SEARCH_SVG}</button>
            <button class="pctrl rec-btn" data-record aria-label="Record session" title="Record">${REC_SVG}</button>
            <button class="pctrl" data-restart aria-label="Restart agent" title="Restart">${RESTART_SVG}</button>
          </div>
        </div>
        <button class="pctrl" data-edit aria-label="Rename agent" title="Rename">${EDIT_SVG}</button>
        <button class="pctrl" data-max aria-label="Focus pane" title="Focus"><span class="ic-max">${MAX_SVG}</span><span class="ic-back">${BACK_SVG}</span></button>
        <button class="pctrl danger" data-kill aria-label="Remove agent" title="Remove agent (stops it and closes its terminal)">${KILL_SVG}</button>
      </div>
    </div>
    <div class="pane-find" data-find hidden>
      <input class="pane-find-in" data-find-in type="text" placeholder="Find in output…" spellcheck="false" />
      <span class="pane-find-count" data-find-count></span>
      <button class="pane-find-btn" data-find-prev aria-label="Previous match">&#8249;</button>
      <button class="pane-find-btn" data-find-next aria-label="Next match">&#8250;</button>
      <button class="pane-find-btn" data-find-close aria-label="Close search">${KILL_SVG}</button>
    </div>
    <div class="term-host" data-host></div>`;
  return el;
}

/** A stopped pane keeps its last output with a centered Resume card over the terminal. Visibility follows the stopped class: restored panes carry it from mount, exits add it, boot clears it. */
export function updateStoppedOverlay(pane: Pane): void {
  const host = pane.el.querySelector<HTMLElement>(".term-host");
  let veil = pane.el.querySelector<HTMLElement>(".pane-veil");
  if (!pane.el.classList.contains("stopped") || !host) { veil?.remove(); return; }
  if (!veil) {
    veil = document.createElement("div");
    veil.className = "pane-veil";
    veil.innerHTML = `<div class="pane-veil-card"><div class="pane-veil-name"></div><div class="pane-veil-sub"></div><button type="button" class="pane-resume">Resume</button></div>`;
    veil.querySelector("button")!.addEventListener("click", () => void pane.restart?.());
    host.append(veil);
  }
  veil.querySelector(".pane-veil-name")!.textContent = `${pane.spec.name} stopped`;
  veil.querySelector(".pane-veil-sub")!.textContent = pane.exitCode ? `exit code ${pane.exitCode} · Resume to carry on` : "Resume to carry on";
  veil.querySelector("button")!.setAttribute("aria-label", `Resume ${pane.spec.name}`);
}

export function setStatus(p: Pane, text: string, cls: "" | "run" | "err" | "wait") {
  const s = p.el.querySelector<HTMLElement>("[data-status]");
  if (s) {
    s.textContent = text;
    s.className = "pane-stat" + (cls ? " " + cls : "");
  }
  p.el.classList.toggle("err", cls === "err");
  p.el.classList.toggle("run", cls === "run");
  updateStoppedOverlay(p);
}

// Mount a pane immediately (status "queued…"); return a thunk that boots the
// real process. Splitting mount from boot lets the caller throttle booting so a
// big fleet doesn't spike the CPU all at once.
// When `restore` is true the pane is mounted in a STOPPED state (no PTY spawn) —
// session restore uses this so reopening doesn't auto-launch a heavy fleet.
// `attach` re-binds an ALREADY-RUNNING agent (tab detached from another window):
// the pane keeps the original agent id and the thunk calls pty_attach instead
// of spawning a new process.
export function createAgent(
  ws: Workspace,
  spec: AgentSpec,
  restore = false,
  attach?: { id: string; spawnedAt: number | null },
): () => Promise<void> {
  // Make the (sized) workspace grid visible BEFORE mounting xterm, otherwise
  // fit() measures a display:none container as 0×0 and ConPTY paints the prompt
  // at the wrong size (blank pane).
  onShowWorkspace();
  const id = attach?.id ?? newId();
  const sub = spec.cwd ? basename(spec.cwd) : "";
  const el = buildPaneEl(id, spec.name, sub, spec.badge, spec.color, spec.role);
  ws.gridEl.insertBefore(el, ws.gridEl.lastElementChild); // before the spawn tile

  const host = el.querySelector<HTMLElement>("[data-host]")!;
  const term = mountTerminal(
    host,
    (data) => {
      // Always forward keystrokes AND xterm's automatic answers (e.g. the ConPTY
      // cursor-position-report reply that unblocks the very first render) as long
      // as the pane still exists — never gate on `running`, or the early reply is
      // dropped and ConPTY stalls (blank pane).
      if (ws.panes.has(id)) void sendInput(id, data).catch(() => {});
      pane.lastInputAt = Date.now();
      clearAttention(pane); // the user is interacting → not waiting on them
    },
    (cols, rows) => {
      if (ws.panes.has(id)) void resizePty(id, cols, rows).catch(() => {});
    },
    {
      openLink: (url) => void openExternal(url).catch(() => {}),
      fontSize: paneFont(ws),
      look: paneLook(),
    },
  );
  // Keep a tiled pane readable: an agent CLI spends ~7 rows on its prompt box
  // and status line, so anything under this floor shows no conversation at all.
  // Stands down once the user has zoomed — see autoFitRows.
  term.setAutoFit(autoFitRows(PANE_MIN_ROWS, getZoom(ws)));

  // The persona name owns the title bar now; surface the terminal's own title
  // as a hover tooltip on the name (not the whole pane, or it hangs over the chat).
  term.onTitleChange((title) => {
    const nameEl = el.querySelector<HTMLElement>(".pb-name");
    if (title.trim() && nameEl) nameEl.title = title;
  });

  const pane: Pane = { id, el, term, running: false, spawnedAt: null, lastOutputAt: 0, lastInputAt: 0, attention: false, attentionClearedAt: 0, attentionNotified: false, color: spec.color, spec };
  ws.panes.set(id, pane);
  layoutGrid(ws);
  onUpdateCount();

  // A restored pane is parked as "stopped" — no PTY is spawned until the user
  // hits ⟳ (pane.restart boots it in place).
  if (restore) {
    setStatus(pane, "stopped", "");
    el.classList.add("stopped"); // dims the parked pane (cleared on boot)
    term.write(enc.encode("\r\n\x1b[90m  [stopped: Resume to carry on]\x1b[0m\r\n"));
    updateStoppedOverlay(pane);
  }

  el.querySelector("[data-kill]")?.addEventListener("click", () => void removeAgent(ws, id));
  el.querySelector("[data-record]")?.addEventListener("click", () => void toggleRecord(ws, pane));
  el.querySelector("[data-restart]")?.addEventListener("click", () => void pane.restart?.());
  el.querySelector("[data-max]")?.addEventListener("click", (e) => toggleMax(ws, pane, e as MouseEvent));
  // "⋯ more" opens the extra controls (search / record / restart) as a menu
  // with words, keyboard and Esc included; the hidden buttons still do the work.
  const moreBtn = el.querySelector<HTMLElement>("[data-more]");
  moreBtn?.addEventListener("click", (e) => {
    e.stopPropagation();
    const r = moreBtn.getBoundingClientRect();
    const press = (sel: string) => () => el.querySelector<HTMLElement>(sel)?.click();
    moreBtn.setAttribute("aria-expanded", "true");
    openMenu(r.right - 200, r.bottom + 6, [
      { label: "Search output", run: press("[data-search]") },
      { label: pane.recording ? "Stop recording" : "Record session", run: press("[data-record]") },
      { label: pane.running ? "Restart" : "Resume", run: press("[data-restart]") },
    ], `${pane.spec.name} actions`);
    const off = () => { moreBtn.setAttribute("aria-expanded", "false"); window.removeEventListener("pointerdown", off, true); window.removeEventListener("keydown", off, true); };
    window.addEventListener("pointerdown", off, true);
    window.addEventListener("keydown", off, true);
  });
  el.querySelector<HTMLElement>("[data-drag]")?.addEventListener("dblclick", (e) => {
    const tgt = e.target as HTMLElement;
    if (tgt.closest(".pctrl") || tgt.closest(".pb-name") || tgt.closest("[data-edit]")) return; // buttons + rename aren't focus triggers
    toggleMax(ws, pane);
  });
  onWirePaneSearch(pane);
  wirePaneRename(ws, pane);
  // Clicking / focusing into a flagged pane means the user is now looking at it.
  el.addEventListener("pointerdown", () => clearAttention(pane));
  el.addEventListener("focusin", () => clearAttention(pane));

  saveSession();

  // Start the CLI in this pane. Also how a stopped or restored agent starts
  // again: in place, so the pane keeps its id, its place on the stage, its chat
  // and its spot in the Grid (see restart below).
  const boot = async () => {
    if (!ws.panes.has(id)) return; // killed before its turn to boot
    const { cols, rows } = term.fit();
    // Held from a fresh capture's lock to its find; the finally lets go.
    let releaseCapture: () => void = () => {};
    try {
      // Isolated agents get their own worktree+branch; point the PTY cwd there.
      let cwd = spec.cwd;
      if (spec.resumeIn) {
        // Carrying on a conversation from another folder: Claude only resumes it there.
        cwd = spec.resumeIn;
      } else if (ws.isolated && ws.repoRoot && !spec.worktree) {
        try {
          spec.branch = branchName(spec.name, id.slice(-6));
          spec.worktree = await worktreeAdd(ws.repoRoot, spec.branch);
          cwd = spec.worktree;
          const subEl = el.querySelector<HTMLElement>("[data-sub]");
          if (subEl) subEl.textContent = spec.branch;
          saveSession();
        } catch (e) {
          term.write(enc.encode(`\r\n\x1b[33m[worktree failed, using project dir: ${onErrMsg(e)}]\x1b[0m\r\n`));
        }
      } else if (spec.worktree) {
        cwd = spec.worktree;
        // Agents work in the project folder unless a worktree each is on: one
        // made before leaves it, if it holds no work of its own.
        if (!getPref("worktree")) {
          const home = await worktreeLeave(spec.worktree, spec.badge === "claude" ? spec.sessionId ?? null : null).catch(() => null);
          if (home) {
            delete spec.worktree;
            delete spec.branch;
            cwd = spec.cwd ?? home;
            const subEl = el.querySelector<HTMLElement>("[data-sub]");
            if (subEl) subEl.textContent = "";
            saveSession();
          }
        }
      }
      // Start the CLI in a folder Maestro chose and wrote down (never the PTY's
      // silent default), so the chat view reads the transcript of this run.
      cwd = runDir({ worktree: spec.worktree, cwd }, await homeFolder());
      spec.ranIn = cwd;
      // Enforce Maestro's protocol at the system-prompt level so a Claude agent
      // MUST follow it (not a soft MCP hint, not a button). The director gets the
      // dispatch prompt; every other Claude gets the plan-first worker one.
      // Only claude exposes --append-system-prompt; other CLIs still get the MCP
      // tools + server instructions. New array — never mutate spec.args, or a
      // restart would append the flag again and again.
      const laws = spec.role === "conductor" ? DIRECTOR_LAWS : MAESTRO_LAWS;
      const own =
        spec.badge === "claude" ? await claudeSessionArgs(spec, cwd)
        : spec.badge === "muse" ? await museSessionArgs(spec)
        : spec.badge === "codex" ? await codexSessionArgs(spec)
        : spec.badge === "opencode" ? await opencodeSessionArgs(spec)
        : [];
      if (own.length) saveSession(); // History finds this run's transcript after a restart too
      // A Muse/Codex resume is a subcommand (`muse resume <id>`); root options
      // (--yolo, …) may stand on either side of it, so appending is fine.
      const args =
        spec.badge === "claude" ? [...spec.args, ...own, "--append-system-prompt", laws]
        : own.length ? [...spec.args, ...own]
        : spec.args;
      // A fresh run of these CLIs can't be given its session id (no such
      // flag): learn it from the newest session begun after we lock, so two
      // agents of one CLI booting together can't claim each other's.
      const captureBadge = (spec.badge === "muse" || spec.badge === "codex" || spec.badge === "opencode") &&
        !spec.sessionId && !picksSession(spec.badge, spec.args) ? spec.badge : null;
      let spawnTs = 0;
      if (captureBadge) {
        releaseCapture = await lockSessionCapture(captureBadge);
        spawnTs = Date.now();
      }
      // Resolve npm/script CLIs (claude, codex, …) through cmd.exe /c so Windows
      // can actually launch them — see launchSpec.
      const launch = launchSpec(spec.program, args);
      // Identity for the child process: maestro-mcp uses MAESTRO_AGENT to
      // stamp who moved/finished a board card (see mcp/src/server.ts).
      const envPairs: Array<[string, string]> = [
        ["MAESTRO_AGENT", spec.name],
        ["MAESTRO_WORKSPACE", cwd ?? ""],
      ];
      pane.spawning = true;
      pane.run = await spawnPty(id, launch.program, launch.args, cwd, cols, rows, envPairs, (bytes) => {
        pane.lastOutputAt = Date.now();
        if (pane.attention) clearAttention(pane); // agent is producing output again
        // After a tab detach this xterm is disposed but the PTY lives on (the
        // new window owns it) — never write into a dropped pane.
        if (ws.panes.has(id)) term.write(bytes);
      });
      pane.spawning = false;
      pane.running = true;
      pane.error = undefined;
      pane.exitCode = undefined;
      pane.spawnedAt = Date.now();
      pane.lastOutputAt = Date.now();
      setStatus(pane, "running", "run");
      onUpdateCount();
      // It may already be gone: a CLI that fails at once can exit before
      // spawnPty returned. Its exit was held; end the pane now.
      if (replayEarlyExit(id, pane.run)) return;
      if (captureBadge) await detectSpawnedSession(spec, captureBadge, cwd, spawnTs, () => pane.running && ws.panes.has(id));
      // Re-fit once the grid layout has settled; correct the PTY size if it moved.
      requestAnimationFrame(() => {
        const s = term.fit();
        if (s.cols !== cols || s.rows !== rows) void resizePty(id, s.cols, s.rows);
      });
    } catch (e) {
      if (pane.spawning) {
        pane.spawning = false;
        takeEarlyExit(id, undefined); // nothing started: drop anything held
      }
      pane.error = onErrMsg(e);
      setStatus(pane, "spawn failed", "err");
      term.write(enc.encode(`\r\n\x1b[31m[spawn failed: ${pane.error}]\x1b[0m\r\n`));
    } finally {
      releaseCapture();
    }
  };

  /** Start again in this same pane: end the old process, clear the screen,
   *  boot. The old process's exit event carries its run number, so it can't
   *  mark the new run as exited (bridges.ts). */
  let restarting = false;
  pane.restart = async (opts = {}) => {
    if (restarting) return;
    restarting = true;
    delete pane.handedTo;
    try {
      // A new conversation, or a given earlier one; otherwise its own again.
      if (opts.fresh) { delete spec.sessionId; delete spec.resumeIn; }
      else if (opts.session) {
        spec.sessionId = opts.session;
        // One from another folder runs there; one from its own folder brings it home.
        if (opts.dir) spec.resumeIn = opts.dir; else delete spec.resumeIn;
      }
      if (pane.recording) await stopRecording(pane);
      await killPty(id).catch(() => {});
      pane.running = false;
      pane.spawnedAt = null;
      clearAttention(pane);
      el.classList.remove("stopped");
      term.reset();
      setStatus(pane, "starting", "");
      onUpdateCount();
      await boot();
      saveSession();
    } finally {
      restarting = false;
    }
  };

  // Detach hand-off: the agent is already alive in the backend — just point
  // its output at this window. The backend replays buffered scrollback first.
  if (attach) {
    return async () => {
      if (!ws.panes.has(id)) return;
      const { cols, rows } = term.fit();
      try {
        await attachPty(id, (bytes) => {
          pane.lastOutputAt = Date.now();
          if (pane.attention) clearAttention(pane); // agent is producing output again
          if (ws.panes.has(id)) term.write(bytes);
        });
        pane.running = true;
        pane.spawnedAt = attach.spawnedAt ?? Date.now();
        pane.lastOutputAt = Date.now();
        setStatus(pane, "running", "run");
        onUpdateCount();
        void resizePty(id, cols, rows).catch(() => {});
      } catch {
        // Died between hand-off and attach — its pty-exit fired before we
        // were listening, so park it the way a normal exit would.
        pane.running = false;
        setStatus(pane, "exited", "");
        onUpdateCount();
      }
    };
  }

  return boot;
}

/** Claude's session for this run, remembered on the spec: its own one again
 *  (`--resume`) when it exists, else a new one (`--session-id`); none when the
 *  preset already picks or resumes a session itself. */
export async function claudeSessionArgs(
  spec: AgentSpec,
  dir: string,
  exists: (dir: string, id: string) => Promise<boolean> = claudeSessionExists,
): Promise<string[]> {
  const picks = spec.args.some((a) => /^(--session-id|--resume|--continue|-r|-c)(=|$)/.test(a));
  if (picks) { delete spec.sessionId; return []; }
  // Its own conversation, when Claude still has it: a restart, an app restart
  // or Resume all carries on where it was instead of starting blank.
  if (spec.sessionId && (await exists(dir, spec.sessionId).catch(() => false))) return ["--resume", spec.sessionId];
  spec.sessionId = crypto.randomUUID();
  return ["--session-id", spec.sessionId];
}

/** True when the agent's own args already choose a Muse session (`resume …`,
 *  `exec …`): Maestro must neither append its own nor capture one. */
export function musePicksSession(args: string[]): boolean {
  return args.some((a) => a === "resume" || a === "--last" || a === "exec");
}

/** Muse's session for this run, remembered on the spec: its own one again
 *  (`muse resume <id>`) when the store still has it, else a fresh boot whose
 *  id the capture after spawning learns (see boot). None when the args
 *  already pick a session themselves. */
export async function museSessionArgs(
  spec: AgentSpec,
  exists: (id: string) => Promise<boolean> = museSessionExists,
): Promise<string[]> {
  if (musePicksSession(spec.args)) { delete spec.sessionId; return []; }
  // Its own conversation, when Muse still has it: a restart, an app restart
  // or Resume all carries on where it was instead of starting blank.
  if (spec.sessionId && (await exists(spec.sessionId).catch(() => false))) return ["resume", spec.sessionId];
  delete spec.sessionId; // gone or never had one: boot fresh, capture after
  return [];
}

/** True when the agent's own args already choose a Codex session (`resume …`,
 *  `exec …`, `fork …`): Maestro must neither append its own nor capture one. */
export function codexPicksSession(args: string[]): boolean {
  return args.some((a) => a === "resume" || a === "--last" || a === "exec" || a === "fork");
}

/** Codex's session for this run, remembered on the spec: its own one again
 *  (`codex resume <id>`) when Codex still has it, else a fresh boot whose id
 *  the capture after spawning learns (see boot). None when the args already
 *  pick a session themselves. */
export async function codexSessionArgs(
  spec: AgentSpec,
  exists: (id: string) => Promise<boolean> = codexSessionExists,
): Promise<string[]> {
  if (codexPicksSession(spec.args)) { delete spec.sessionId; return []; }
  // Its own conversation, when Codex still has it: a restart, an app restart
  // or Resume all carries on where it was instead of starting blank.
  if (spec.sessionId && (await exists(spec.sessionId).catch(() => false))) return ["resume", spec.sessionId];
  delete spec.sessionId; // gone or never had one: boot fresh, capture after
  return [];
}

/** True when the agent's own args already choose an opencode session
 *  (`--continue`, `--session …`, `run …`): Maestro must neither append its
 *  own nor capture one. */
export function opencodePicksSession(args: string[]): boolean {
  return args.some((a) => a === "--continue" || a === "-c" || a === "--session" || a === "-s" ||
    a.startsWith("--session=") || a === "--fork" || a === "run" || a === "session" || a === "export" || a === "import");
}

/** opencode's session for this run, remembered on the spec: its own one again
 *  (`opencode --session <id>`) when opencode still has it, else a fresh boot
 *  whose id the capture after spawning learns (see boot). None when the args
 *  already pick a session themselves. */
export async function opencodeSessionArgs(
  spec: AgentSpec,
  exists: (id: string) => Promise<boolean> = opencodeSessionExists,
): Promise<string[]> {
  if (opencodePicksSession(spec.args)) { delete spec.sessionId; return []; }
  // Its own conversation, when opencode still has it: a restart, an app
  // restart or Resume all carries on where it was instead of starting blank.
  if (spec.sessionId && (await exists(spec.sessionId).catch(() => false))) return ["--session", spec.sessionId];
  delete spec.sessionId; // gone or never had one: boot fresh, capture after
  return [];
}

/** Whether the agent's own args already choose this CLI's session. */
function picksSession(badge: string, args: string[]): boolean {
  if (badge === "muse") return musePicksSession(args);
  if (badge === "codex") return codexPicksSession(args);
  if (badge === "opencode") return opencodePicksSession(args);
  return false;
}

const captureTails = new Map<string, Promise<void>>();
/** One session capture per CLI at a time: the lock spans a fresh boot's
 *  spawn and capture, so two agents of one CLI starting together each learn
 *  their own session. Resolves to the release. */
function lockSessionCapture(badge: string): Promise<() => void> {
  let release!: () => void;
  const taken = new Promise<void>((res) => { release = res; });
  const prev = captureTails.get(badge) ?? Promise.resolve();
  captureTails.set(badge, prev.then(() => taken));
  return prev.then(() => release);
}

/** The newest session of `badge` begun at or after `sinceMs`: a Muse session
 *  anywhere, a Codex/opencode one in `dir`. Null when none exists yet. */
async function latestSession(badge: string, dir: string | null, sinceMs: number): Promise<string | null> {
  if (badge === "muse") {
    const fresh = (await museSessions()).filter((s) => s.modified_ms >= sinceMs);
    fresh.sort((a, b) => b.modified_ms - a.modified_ms);
    return fresh[0]?.id ?? null;
  }
  if (!dir) return null;
  if (badge === "codex") return codexLatestSession(dir, sinceMs);
  if (badge === "opencode") return opencodeLatestSession(dir, sinceMs);
  return null;
}

/** Learn a fresh run's session id: the session begun since `sinceMs` while
 *  the capture lock was held. Gives up silently past the deadline or when the
 *  pane is gone — the next restart then just boots fresh again, as before. */
async function detectSpawnedSession(spec: AgentSpec, badge: string, dir: string | null, sinceMs: number, alive: () => boolean): Promise<void> {
  const deadline = Date.now() + 12_000;
  for (;;) {
    try {
      const id = await latestSession(badge, dir, sinceMs);
      if (id) {
        spec.sessionId = id;
        saveSession();
        return;
      }
    } catch { /* the store read hiccuped: retry until the deadline */ }
    if (!alive() || Date.now() >= deadline) return;
    await new Promise((r) => setTimeout(r, 400));
  }
}

export async function removeAgent(ws: Workspace, id: string) {
  const p = ws.panes.get(id);
  if (!p) return;
  if (p.recording) await stopRecording(p); // flush the recording before the PTY dies
  try {
    await killPty(id);
  } catch {
    /* ignore */
  }
  p.term.dispose();
  p.el.remove();
  ws.panes.delete(id);
  layoutGrid(ws);
  onUpdateCount();
  saveSession();
}

/* ---------------- session recording (replay) ---------------- */

// Recordings live under the workspace's own .maestro folder so the player can
// list them per project (see openReplays). Path uses forward slashes — Rust's
// Path handles them on Windows and create_dir_all makes the folder.

/** Reflect a pane's REC button state (pulsing dot + label). */
function setRecUi(p: Pane): void {
  const btn = p.el.querySelector<HTMLElement>("[data-record]");
  if (!btn) return;
  const on = !!p.recording;
  btn.classList.toggle("rec", on);
  btn.setAttribute("aria-label", on ? "Stop recording" : "Record session");
  p.el.classList.toggle("recording", on);
}

/** Transient bottom-right toast (reuses the kanban toast style). */
export function paneToast(text: string, onClick?: () => void): void {
  const t = document.createElement("div");
  t.className = "kb-toast";
  t.textContent = text;
  if (onClick) {
    t.style.cursor = "pointer";
    t.addEventListener("click", () => {
      onClick();
      t.remove();
    });
  }
  document.body.appendChild(t);
  requestAnimationFrame(() => t.classList.add("on"));
  window.setTimeout(() => {
    t.classList.remove("on");
    window.setTimeout(() => t.remove(), 400);
  }, 5000);
}

/** Stop a pane's recording (flush the file) and reset its UI. Safe to call when
 *  not recording. Returns the finished recording's path (or null). */
export async function stopRecording(p: Pane): Promise<string | null> {
  const path = p.recording;
  if (!path) return null;
  p.recording = undefined;
  setRecUi(p);
  try {
    await recordStop(p.id);
  } catch {
    /* the agent may already be gone — the file was flushed on drop */
  }
  return path;
}

/** Toggle recording for a pane. Start writes to
 *  `<workspace>/.maestro/recordings/<agent>-<epochms>.jsonl`. */
export async function toggleRecord(ws: Workspace, p: Pane): Promise<void> {
  if (p.recording) {
    const path = await stopRecording(p);
    paneToast("Recording saved — click to replay", () => {
      if (path) openReplays(ws, path);
    });
    return;
  }
  if (!p.running) {
    paneToast("Agent isn't running — nothing to record");
    return;
  }
  const dir = ws.dir ?? p.spec.cwd;
  if (!dir) {
    paneToast("Open a project folder before recording");
    return;
  }
  const safe = (p.spec.name || "agent").replace(/[^a-zA-Z0-9._-]+/g, "-").slice(0, 40);
  const path = `${dir}/${REC_DIR_REL}/${safe}-${Date.now()}.jsonl`;
  try {
    await recordStart(p.id, path);
    p.recording = path;
    setRecUi(p);
  } catch (e) {
    paneToast(`Couldn't start recording: ${onErrMsg(e)}`);
  }
}

/* ---------------- attention ---------------- */
// Heuristic "agent needs you": a RUNNING pane that produced output recently and
// then went silent is probably waiting at a prompt for the user. We flag it,
// light up the pill + tab, and (if the window is in the background) fire one OS
// notification. The flag clears when the user types/clicks into the pane or the
// agent starts producing output again. attentionClearedAt stops a quiet shell
// prompt from re-flagging forever: only a NEW burst of output (after the clear)
// that then goes silent can flag again.
const ATTN_SILENCE_MS = 10_000; // output, then this much quiet ⇒ probably waiting

/** Drop a pane's attention flag and restore its normal pill/tab styling. */
export function clearAttention(pane: Pane) {
  if (!pane.attention) return;
  pane.attention = false;
  pane.attentionNotified = false;
  pane.attentionClearedAt = Date.now();
  pane.el.classList.remove("attention");
  pane.el.querySelector(".pb-dot")?.classList.remove("work");
  // Let the next tick re-derive the run/idle status; set a sane default now.
  setStatus(pane, pane.running ? "running" : "idle", pane.running ? "run" : "");
}

/** Raise a pane's attention flag (pill + tab + optional OS notification). */
function setAttention(pane: Pane, ws: Workspace) {
  if (pane.attention) return;
  pane.attention = true;
  pane.el.classList.add("attention");
  pane.el.querySelector(".pb-dot")?.classList.add("work");
  setStatus(pane, "needs you", "wait");
  // Notify only while the window is unattended, once per flag.
  if (!pane.attentionNotified && getPref("notifyNeeds") && (document.hidden || !document.hasFocus())) {
    pane.attentionNotified = true;
    // Say what it wants when the screen shows it ("Ana wants to run npm test").
    const line = taskOf(pane.id)?.status.state === "needs" ? taskLine(pane.id) : null;
    void notifyAgent(line ?? `${pane.spec.name} needs you`, ws.name, pane.id).catch(() => {});
  }
}

/** Per-tick attention sweep. Cheap no-op when nothing is running. */
export function updateAttention(now: number) {
  for (const w of workspaces.values())
    for (const pane of w.panes.values()) {
      if (!pane.running || pane.attention) continue;
      // Flag when: had output, that output is now stale, and it arrived AFTER
      // both the last user input and the last clear (so a parked prompt that
      // we already dismissed can't immediately re-flag).
      if (
        pane.lastOutputAt > 0 &&
        now - pane.lastOutputAt > ATTN_SILENCE_MS &&
        pane.lastOutputAt > pane.lastInputAt &&
        pane.lastOutputAt > pane.attentionClearedAt
      ) {
        setAttention(pane, w);
      }
    }
}
