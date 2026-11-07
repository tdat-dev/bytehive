// Start screen (Home): say what an agent should do and start it, in one step.
//
// After Codex's "What should we code next?", Grok's single box, Plane/Fabric's
// greeting homes and Devin/Fabric's first-run checklists on Mobbin: a greeting
// with one live sentence about what waits on you, a composer (job, project,
// CLI, how many), a few starter jobs, the agents waiting on you, and recent
// projects. First run swaps the projects for three steps.

import { workspaces } from "./appstate";
import { roomNote } from "./planmeter";
import { activateWorkspace, createWorkspace } from "./workspace";
import { pickFolder } from "./ipc";
import { addRecent, getRecents } from "./recents";
import { allTasks, type Task } from "./tasks";
import { openNewAgent } from "./inboxnew";
import { loadCrew, presetAvailable, quickTerminal, spawnAgents } from "./spawnmodal";
import { CLI_PRESETS, CUSTOM_PREFIX } from "./crew";
import { getPref } from "./prefs";
import { sameFolder } from "./workspaces";
import { revealPane } from "./agentbridge";
import { focusPane } from "./panelayout";
import { enhanceSelects } from "./selectmenu";
import { openHelpPage } from "./inboxhelp";
import type { Workspace } from "./panetypes";

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const FOLDER_SVG = `<svg class="i" aria-hidden="true"><use href="#i-folder"/></svg>`;
const LAST_CLI = "maestro.inbox.lastCli";
const LAST_CUSTOM = "maestro.inbox.lastCustom";
const CHOOSE = "__choose__";
/** Agent select value for the free-text command row. */
const CUSTOM_OPT = "__custom__";

import { STARTERS } from "./starters";
import { APP_NAME } from "./appname";
import { glyph } from "./icons";
import { hiveLogo } from "./hivelogo";
export { STARTERS };


/** The last path segment, for a project's name. */
export function folderName(dir: string): string {
  return dir.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || dir;
}

/** "Good morning" and friends. */
export function greeting(hour: number): string {
  if (hour < 5) return "Working late";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

/** The one sentence under the greeting: what, if anything, waits on you. */
export function statusSentence(tasks: Pick<Task, "name" | "status">[], firstRun: boolean): string {
  const needs = tasks.filter((t) => t.status.state === "needs");
  const working = tasks.filter((t) => t.status.state === "working").length;
  const review = tasks.filter((t) => t.status.state === "review").length;
  if (needs.length === 1) return `${needs[0].name} is waiting on you.`;
  if (needs.length > 1) return `${needs.length} agents are waiting on you.`;
  if (review) return `${review === 1 ? "One agent has" : `${review} agents have`} finished and ${review === 1 ? "is" : "are"} ready to review.`;
  if (working) return `${working === 1 ? "One agent is" : `${working} agents are`} working. Nothing needs you yet.`;
  if (firstRun) return "Pick a folder, say what you want done, and an agent starts on it.";
  return "Nothing is running. What should an agent do next?";
}

/** "4 agents · 1 needs you" for a recent project that is open. */
export function projectLine(dir: string, tasks: Task[]): { agents: number; needs: number } {
  const ws = [...workspaces.values()].find((w) => sameFolder(w.dir, dir));
  const mine = ws ? tasks.filter((t) => t.wsId === ws.id) : [];
  return { agents: mine.length, needs: mine.filter((t) => t.status.state === "needs").length };
}

/** Switch to a folder's project, opening it when it isn't open yet. */
function projectFor(dir: string): Workspace {
  const open = [...workspaces.values()].find((w) => sameFolder(w.dir, dir));
  if (open) { activateWorkspace(open); addRecent(dir); return open; }
  const ws = createWorkspace(dir);
  addRecent(dir);
  return ws;
}

/** Open a folder as a project, then offer New agent when it has no agents. */
export function openProject(dir: string, count?: number): void {
  const ws = projectFor(dir);
  if (!ws.panes.size || count) openNewAgent({ count });
}

export async function chooseFolder(count?: number): Promise<void> {
  const dir = await pickFolder();
  if (dir) openProject(dir, count);
}

let el: HTMLElement | null = null;
let sig = "";

function lastCli(): string {
  const set = getPref("defaultCli");
  if (set !== "last") return set;
  try { return localStorage.getItem(LAST_CLI) || "claude"; } catch { return "claude"; }
}

const NOT_INSTALLED = "Not installed";

/** A path as people say it: the home folder reads as ~. */
export function shortPath(dir: string): string {
  const m = /^([a-z]:[\\/]users[\\/][^\\/]+)(?=[\\/]|$)/i.exec(dir) ?? /^(\/(?:home|Users)\/[^/]+)(?=\/|$)/.exec(dir);
  return m ? "~" + dir.slice(m[1].length) : dir;
}

/** The folder list: recent projects with their path, then a way to pick another. */
function dirOptions(recents: string[]): string {
  return recents.map((d, i) => `<option value="${esc(d)}" data-sub="${esc(shortPath(d))}"${i === 0 ? " selected" : ""}>${esc(folderName(d))}</option>`).join("") +
    `<option value="${CHOOSE}"${recents.length ? " data-sep" : " selected"}>Choose a folder…</option>`;
}

function composerHTML(): string {
  const recents = getRecents();
  const last = lastCli();
  const isCustom = last.startsWith(CUSTOM_PREFIX);
  const cli = isCustom ? CUSTOM_OPT : last;
  let customCmd = "";
  try {
    customCmd = isCustom ? last.slice(CUSTOM_PREFIX.length) : localStorage.getItem(LAST_CUSTOM) || "";
  } catch { /* private window */ }
  const count = getPref("defaultCount");
  const clis = CLI_PRESETS.filter((p) => !p.shell);
  return `
    <form class="st-compose" aria-label="Start an agent">
      <label class="ia-sr" for="stJob">What should an agent do?</label>
      <textarea id="stJob" rows="3" placeholder="What should an agent do?"></textarea>
      <div class="st-bar">
        <label class="st-chip" title="The folder it works in">${FOLDER_SVG}
          <span class="ia-sr">Project</span>
          <select id="stDir">${dirOptions(recents)}</select></label>
        <label class="st-chip"><span class="ia-sr">Agent</span>
          <select id="stCli">${clis.map((p) => `<option value="${esc(p.id)}"${p.id === cli ? " selected" : ""}${presetAvailable(p.program) ? "" : ` disabled data-note="${NOT_INSTALLED}"`}>${esc(p.label)}</option>`).join("")}<option value="${CUSTOM_OPT}"${isCustom ? " selected" : ""}>Custom…</option></select></label>
        <input id="stCustom" class="st-custom" placeholder="Command, e.g. ollama run llama3.1" value="${esc(customCmd)}" aria-label="Custom command"${isCustom ? "" : " hidden"}>
        <label class="st-chip"><span class="ia-sr">How many agents</span>
          <select id="stCount">${[1, 2, 3].map((n) => `<option value="${n}"${n === count ? " selected" : ""}>${n === 1 ? "1 agent" : `${n} agents, same job`}</option>`).join("")}</select></label>
        <span class="st-sp"></span>
        <button type="submit" class="btn primary st-go" aria-keyshortcuts="Enter">Start<kbd aria-hidden="true">↵</kbd></button>
      </div>
      <p class="st-room" data-room hidden></p>
    </form>
    <div class="st-try" role="group" aria-label="Jobs to try">${STARTERS.map((s) => `<button type="button" class="st-starter" data-starter="${esc(s.job)}" title="${esc(s.job)}">${esc(s.label)}</button>`).join("")}</div>
    <button type="button" class="st-help" data-st="help">See everything ByteHive can do <kbd>?</kbd></button>`;
}

/** The parts that follow live state; rebuilt only when that state changes, so
 *  nothing you type in the composer is touched. */
export function renderStart(): void {
  if (!el) return;
  const h1 = el.querySelector("h1");
  const hello = `${greeting(new Date().getHours())}.`;
  if (h1 && h1.textContent !== hello) h1.textContent = hello;
  const tasks = allTasks();
  const recents = getRecents();
  const firstRun = recents.length === 0 && workspaces.size === 0;
  const rows = recents.map((dir) => ({ dir, ...projectLine(dir, tasks) }));
  const waiting = tasks.filter((t) => t.status.state === "needs");
  const next = JSON.stringify([statusSentence(tasks, firstRun), rows, waiting.map((t) => [t.paneId, t.status.ask?.prompt, t.name]), firstRun]);
  syncChoices(recents);
  if (next === sig) return;
  sig = next;
  const status = el.querySelector<HTMLElement>(".st-status");
  if (status) {
    status.textContent = statusSentence(tasks, firstRun);
    status.classList.toggle("needs", waiting.length > 0);
  }
  const live = el.querySelector<HTMLElement>(".st-live");
  if (!live) return;
  live.innerHTML =
    (waiting.length ? `<section class="st-sec" aria-label="Waiting on you"><h2>Waiting on you</h2><div class="st-wait">${waiting.map((t) => `
      <button class="st-w" data-pane="${esc(t.paneId)}" data-ws="${esc(t.wsId)}">
        ${glyph("needs")}<b>${esc(t.name)}</b><span class="st-wp">${esc(t.project)}</span>
        <span class="st-wl">${esc(askLine(t))}</span><span class="st-open">Answer</span></button>`).join("")}</div></section>` : "") +
    (firstRun ? `<section class="st-sec" aria-label="Getting started"><h2>Getting started</h2><ol class="st-steps">
        <li><b>Choose a folder</b><span>Any project on your disk. Agents work right in that folder. Settings can give each one its own git branch instead.</span></li>
        <li><b>Say what you want done</b><span>In plain words, in the box above. Pick Claude Code, Codex, Gemini or another CLI you have installed.</span></li>
        <li><b>Answer when it asks</b><span>When an agent wants to run a command or has a question, the line at the top says so and its row shows what it wants. Answer it in its terminal.</span></li>
      </ol></section>`
      : `<section class="st-sec" aria-label="Projects"><h2>Projects</h2><div class="st-projects">${rows.map((r) => `
        <button class="st-proj" data-dir="${esc(r.dir)}">
          <span class="st-ic" aria-hidden="true">${FOLDER_SVG}</span><b>${esc(folderName(r.dir))}</b>
          <span class="st-p" title="${esc(r.dir)}">${esc(shortPath(r.dir))}</span>
          <span class="st-n${r.needs ? " needs" : ""}">${r.needs ? `${r.needs} waiting on you` : r.agents ? `${r.agents} agent${r.agents === 1 ? "" : "s"}` : "Not open"}</span></button>`).join("")}
        <button class="st-proj st-add" data-st="choose"><span class="st-ic" aria-hidden="true">+</span><b>Open a folder</b><span class="st-p">Start a new project</span></button>
        <button class="st-proj st-add" data-st="shell"><span class="st-ic" aria-hidden="true">›_</span><b>Plain terminal</b><span class="st-p">A shell, no agent</span></button>
      </div></section>`);
}

/** Keep the composer's lists current without touching what is selected: a
 *  folder opened elsewhere joins the project list, and CLIs the install probe
 *  found missing are greyed out. */
function syncChoices(recents: string[]): void {
  const dirSel = el?.querySelector<HTMLSelectElement>("#stDir");
  if (dirSel) {
    const have = [...dirSel.options].map((o) => o.value).filter((v) => v !== CHOOSE);
    if (have.join("\n") !== recents.join("\n")) {
      const keep = dirSel.value;
      dirSel.innerHTML = dirOptions(recents);
      dirSel.value = keep === CHOOSE || recents.includes(keep) ? keep : recents[0] ?? CHOOSE;
    }
  }
  syncRoom();
  el?.querySelectorAll<HTMLOptionElement>("#stCli option").forEach((o) => {
    const p = CLI_PRESETS.find((x) => x.id === o.value);
    const off = !!p && !presetAvailable(p.program);
    if (o.disabled !== off) o.disabled = off;
    if (off) o.dataset.note = NOT_INSTALLED; else delete o.dataset.note;
  });
}

/** Show the custom command box only while Custom is the chosen CLI. */
function syncCustomInput(): void {
  const sel = el?.querySelector<HTMLSelectElement>("#stCli");
  el?.querySelector<HTMLElement>("#stCustom")?.toggleAttribute("hidden", sel?.value !== CUSTOM_OPT);
}

/** How much of the chosen CLI's plan is left, when little. */
function syncRoom(): void {
  const n = el?.querySelector<HTMLElement>("[data-room]");
  const id = el?.querySelector<HTMLSelectElement>("#stCli")?.value;
  const cli = CLI_PRESETS.find((p) => p.id === id);
  if (!n) return;
  const note = cli ? roomNote(cli.badge, CLI_PRESETS.filter((p) => !p.shell && presetAvailable(p.program))) : "";
  if (n.textContent !== note) n.textContent = note;
  n.hidden = !note;
}

/** What a waiting agent wants, in a few words. */
function askLine(t: Task): string {
  const a = t.status.ask;
  if (!a) return "Needs you";
  if (a.kind === "run") return a.detail ? `Wants to run ${a.detail}` : "Wants to run a command";
  if (a.kind === "edit") return a.detail ? `Wants to edit ${a.detail}` : "Wants to edit a file";
  return a.prompt;
}

async function start(form: HTMLFormElement): Promise<void> {
  const job = form.querySelector<HTMLTextAreaElement>("#stJob")!;
  const dirSel = form.querySelector<HTMLSelectElement>("#stDir")!;
  let cli = form.querySelector<HTMLSelectElement>("#stCli")!.value;
  const n = Number(form.querySelector<HTMLSelectElement>("#stCount")!.value) || 1;
  if (cli === CUSTOM_OPT) {
    const cmd = form.querySelector<HTMLInputElement>("#stCustom")?.value.trim() ?? "";
    if (!cmd) { form.querySelector<HTMLInputElement>("#stCustom")?.focus(); return; }
    cli = CUSTOM_PREFIX + cmd;
    try { localStorage.setItem(LAST_CUSTOM, cmd); } catch { /* private window */ }
  }
  let dir = dirSel.value;
  if (dir === CHOOSE) {
    const picked = await pickFolder();
    if (!picked) return;
    dir = picked;
  }
  try { localStorage.setItem(LAST_CLI, cli); } catch { /* private window */ }
  const task = job.value.trim() || null;
  job.value = "";
  const ws = projectFor(dir);
  void spawnAgents(ws, cli, n, task, loadCrew().skipPerms);
}

export function mountStart(): void {
  const home = document.getElementById("home");
  if (!home || el) return;
  el = document.createElement("section");
  el.className = "inbox-start";
  el.setAttribute("aria-label", "Start");
  const now = new Date();
  el.innerHTML = `<div class="st-in">
    <div class="st-brand"><span class="logo-glow" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none">${hiveLogo("st")}</svg></span>${APP_NAME}</div>
    <h1>${greeting(now.getHours())}.</h1>
    <p class="st-status" aria-live="polite"></p>
    ${composerHTML()}
    <div class="st-live"></div>
  </div>`;
  home.appendChild(el);
  enhanceSelects(el);
  sig = "";
  const form = el.querySelector<HTMLFormElement>(".st-compose")!;
  const job = el.querySelector<HTMLTextAreaElement>("#stJob")!;
  form.addEventListener("submit", (e) => { e.preventDefault(); void start(form); });
  form.addEventListener("change", () => { syncRoom(); syncCustomInput(); });
  // Enter starts, Shift+Enter is a new line, as in a chat box.
  job.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); form.requestSubmit(); }
  });
  el.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    const starter = t.closest<HTMLElement>("[data-starter]");
    if (starter) { job.value = starter.dataset.starter ?? ""; job.focus(); return; }
    const w = t.closest<HTMLElement>(".st-w");
    if (w?.dataset.ws && w.dataset.pane) {
      revealPane(w.dataset.ws, w.dataset.pane);
      const ws = workspaces.get(w.dataset.ws);
      const pane = ws?.panes.get(w.dataset.pane);
      if (ws && pane) focusPane(ws, pane);
      return;
    }
    const proj = t.closest<HTMLElement>(".st-proj[data-dir]");
    if (proj?.dataset.dir) { openProject(proj.dataset.dir); return; }
    switch (t.closest<HTMLElement>("[data-st]")?.dataset.st) {
      case "choose": void chooseFolder(); break;
      case "shell": quickTerminal(getRecents()[0] ?? null); break;
      case "help": openHelpPage(); break;
    }
  });
  renderStart();
}

export function unmountStart(): void {
  el?.remove();
  el = null;
  sig = "";
}
