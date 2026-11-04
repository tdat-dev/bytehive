// Local web dashboard (remote fleet view): serves a JSON snapshot of every
// agent + a text tail of its screen to the phone-facing HTTP page, and relays
// keystrokes/messages from that page back into the right PTY. Split from
// main.ts; reads the fleet via appstate, formats errors via an injected helper.

import { workspaces } from "./appstate";
import { hiddenGate } from "./hiddengate";
import { paneStatus } from "./fleet";
import { taskOf } from "./tasks";
import { lastFinish } from "./finishwatch";
import { finishedLabel } from "./finishnote";
import { projectOf, orderForDash } from "./dashorder";
import {
  sendInput,
  sendMessage,
  dashboardStart,
  dashboardStop,
  dashboardStatus,
  dashboardPush,
  dashboardPasswordStatus,
  dashboardPasswordSet,
  dashboardPasswordClear,
  onDashboardSend,
} from "./ipc";
import { confirmModal } from "./confirmmodal";
import { passwordNotes, passwordProblem } from "./dashpw";

// The remote page reads each agent's on-screen text from its xterm buffer; the
// emulator has already collapsed in-place repaints to the current screen.
const DASH_OUTPUT_ROWS = 40;
const DASH_PORT = 8477;

let onErrMsg: (e: unknown) => string = (e) => String(e);
export function configureDashboard(deps: { errMsg: (e: unknown) => string }): void {
  onErrMsg = deps.errMsg;
}

function fmtUptimeShort(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  return h > 0 ? `${h}h${m % 60}m` : m > 0 ? `${m}m` : `${s}s`;
}

/** JSON snapshot the dashboard HTTP page renders: the roster plus a recent
 *  plain-text tail of each agent's output so it can be managed, not just seen. */
function fleetSnapshotJson(): string {
  const now = Date.now();
  const agents = [];
  const output: Record<string, string> = {};
  for (const ws of workspaces.values())
    for (const p of ws.panes.values()) {
      const t = taskOf(p.id);
      const project = projectOf(p.spec, ws);
      agents.push({
        id: p.id,
        name: p.spec.name,
        wsId: ws.id,
        wsName: ws.name,
        // the page groups agents under their project
        project: project.name,
        projectKey: project.key,
        projectDir: project.dir,
        status: paneStatus(
          {
            id: p.id, name: p.spec.name, color: p.color, wsId: ws.id, wsName: ws.name,
            running: p.running, attention: p.attention, spawnedAt: p.spawnedAt, lastOutputAt: p.lastOutputAt,
          },
          now,
        ),
        uptime: p.spawnedAt && p.running ? fmtUptimeShort(now - p.spawnedAt) : "",
        // through with its work: "done" or "review" (Ready to review), and how many files
        finished: t ? finishedLabel(t.status.state, !!t.worked, !!t.status.limit) : null,
        files: t?.changedFiles ?? null,
        // the run that just ended, so the page can tell you once (a browser notification)
        finish: lastFinish(p.id),
      });
      // a stopped agent is hidden on the page until it runs again: no need to
      // read its screen every second
      if (!p.running) continue;
      const screen = p.term.snapshot(DASH_OUTPUT_ROWS);
      if (screen.trim()) output[p.id] = screen;
    }
  return JSON.stringify({ agents: orderForDash(agents), output });
}

let dashboardOn = false;
function pushDash(): void {
  if (dashboardOn) void dashboardPush(fleetSnapshotJson()).catch(() => {});
}

const dashToggle = document.getElementById("setDashOn") as HTMLInputElement | null;
const dashLan = document.getElementById("setDashLan") as HTMLInputElement | null;
const dashUrl = document.getElementById("setDashUrl");

function paintDash(info: { running: boolean; lan: boolean; urls: string[] }): void {
  dashboardOn = info.running;
  if (dashToggle) dashToggle.checked = info.running;
  if (dashLan) dashLan.checked = info.lan;
  paintPassword();
  if (dashUrl)
    dashUrl.textContent = info.running
      ? "Open on any device on your network: " + (info.urls.join("  ·  ") || `http://127.0.0.1:${DASH_PORT}`)
      : "Off — turn on to view the fleet from your phone.";
}

/* ---- the dashboard's password (Settings > Remote) ---- */
const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T | null;
let hasPassword = false;

/** Show whether a password is set; other devices wait for one. */
function paintPassword(): void {
  const notes = passwordNotes(hasPassword);
  const note = $("setDashPwNote");
  if (note) note.textContent = notes.pw;
  const lanNote = $("setDashLanNote");
  if (lanNote) lanNote.textContent = notes.lan;
  const set = $("setDashPwSet");
  if (set) set.textContent = hasPassword ? "Change" : "Set password";
  const clear = $("setDashPwClear");
  if (clear) clear.hidden = !hasPassword;
  if (dashLan) dashLan.disabled = !hasPassword && !dashLan.checked;
}

function showPasswordForm(open: boolean): void {
  const form = $<HTMLFormElement>("setDashPwForm");
  if (!form) return;
  form.hidden = !open;
  for (const id of ["setDashPw1", "setDashPw2"]) { const i = $<HTMLInputElement>(id); if (i) i.value = ""; }
  const err = $("setDashPwErr");
  if (err) err.textContent = "";
  const title = $("setDashPwTitle");
  if (title) title.textContent = hasPassword ? "Change the password" : "Set a password";
  if (open) $<HTMLInputElement>("setDashPw1")?.focus();
}

async function savePassword(): Promise<void> {
  const a = $<HTMLInputElement>("setDashPw1")?.value ?? "";
  const b = $<HTMLInputElement>("setDashPw2")?.value ?? "";
  const err = $("setDashPwErr");
  const problem = passwordProblem(a, b);
  if (problem) { if (err) err.textContent = problem; return; }
  const save = $<HTMLButtonElement>("setDashPwSave");
  if (save) save.disabled = true;
  try {
    const changed = hasPassword;
    await dashboardPasswordSet(a);
    hasPassword = true;
    showPasswordForm(false);
    paintPassword();
    // a new password signs every device out
    const note = $("setDashPwNote");
    if (note && changed) note.textContent = "Changed. Devices that were signed in have to sign in again.";
  } catch (e) {
    if (err) err.textContent = `Couldn't save it: ${onErrMsg(e)}`;
  } finally {
    if (save) save.disabled = false;
  }
}

async function removePassword(): Promise<void> {
  const lanOn = dashboardOn && (dashLan?.checked ?? false);
  const r = await confirmModal({
    title: "Remove the dashboard password?",
    message: lanOn
      ? "Other devices can't connect without one, so the dashboard goes back to this computer only."
      : "The dashboard on this computer opens without signing in. Other devices can't connect until you set one again.",
    okLabel: "Remove",
    danger: true,
  });
  if (!r.ok) return;
  try {
    const info = await dashboardPasswordClear();
    hasPassword = false;
    if (dashLan) dashLan.checked = false;
    paintPassword();
    // it was shared on the network: bring it back here, for this computer
    if (lanOn && dashToggle?.checked && !info.running) await applyDash();
    else paintDash(info);
  } catch (e) {
    const note = $("setDashPwNote");
    if (note) note.textContent = `Couldn't remove it: ${onErrMsg(e)}`;
  }
}

async function refreshDash(): Promise<void> {
  try {
    paintDash(await dashboardStatus());
    hasPassword = await dashboardPasswordStatus();
    paintPassword();
  } catch {
    /* backend not ready */
  }
}
async function applyDash(): Promise<void> {
  const lan = dashLan?.checked ?? false;
  try {
    if (dashToggle?.checked) paintDash(await dashboardStart(DASH_PORT, lan));
    else paintDash(await dashboardStop());
  } catch (e) {
    if (dashUrl) dashUrl.textContent = `Couldn't start on port ${DASH_PORT}: ${onErrMsg(e)}`;
    if (dashToggle) dashToggle.checked = false;
    dashboardOn = false;
  }
}

/** Start the 1s push loop, the inbound key/message relay, and wire the toggles.
 *  Call once at startup. */
export function initDashboard(): void {
  // The phone dashboard is most use while this window is hidden: it keeps
  // updating then, every 2 s instead of every second.
  const gate = hiddenGate(2);
  window.setInterval(() => { if (gate()) pushDash(); }, 1000);
  // A message OR a raw key from the dashboard page → deliver into the pane's PTY.
  // `keys` is a raw escape sequence sent as-is (arrows, Enter, Esc, ^C, Tab) so
  // an interactive menu can be driven; `message` is a hand-off, so it goes
  // through sendMessage — chunked, with Enter as its own keystroke. Writing
  // `message + "\r"` in one go is what leaves the text sitting unsent in Claude
  // Code's composer (see typing.ts).
  void onDashboardSend((body) => {
    try {
      const o = JSON.parse(body) as { paneId?: unknown; message?: unknown; keys?: unknown };
      if (typeof o.paneId !== "string") return;
      const keys = typeof o.keys === "string" && o.keys ? o.keys : null;
      const message =
        !keys && typeof o.message === "string" && o.message.trim() ? o.message : null;
      if (!keys && message === null) return;
      for (const ws of workspaces.values()) {
        const pane = ws.panes.get(o.paneId);
        if (pane && pane.running) {
          if (keys) void sendInput(pane.id, keys).catch(() => {});
          else void sendMessage(pane.id, message as string).catch(() => {});
          window.setTimeout(pushDash, 150);
          window.setTimeout(pushDash, 450);
          return;
        }
      }
    } catch {
      /* malformed body — ignore */
    }
  });
  dashToggle?.addEventListener("change", () => void applyDash());
  dashLan?.addEventListener("change", () => {
    paintPassword(); // turned off without a password: it can't come back on
    if (dashToggle?.checked) void applyDash(); // re-bind on the new interface
  });
  $("setDashPwSet")?.addEventListener("click", () => showPasswordForm(true));
  $("setDashPwCancel")?.addEventListener("click", () => showPasswordForm(false));
  $("setDashPwForm")?.addEventListener("submit", (e) => { e.preventDefault(); void savePassword(); });
  $("setDashPwClear")?.addEventListener("click", () => void removePassword());
  void refreshDash();
}
