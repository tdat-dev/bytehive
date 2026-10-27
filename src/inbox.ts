// Agent Inbox interface (preview, behind Settings → "New interface").
//
// It does not rebuild the app: the canvas, panes, PTYs and every existing
// feature stay where they are. With the flag on it adds a Queue down the left
// (every agent in every project, ordered by who needs you), a sentence in the
// top bar that says what needs you, and a decision card that answers an agent's
// prompt with buttons. The "stage" is simply the chosen pane in the canvas's
// existing focus mode, so the terminal is the real xterm you can type into.

import { workspaces, activeWs } from "./appstate";
import { focusPane, renamePane } from "./panelayout";
import { revealPane } from "./agentbridge";
import { type Area, type Tile } from "./canvas";
import { railPlan } from "./gridfit";
import { glanceFor, keepGlances, refreshGlance } from "./gridcards";
import { killPty, resizePty, sendInput, sendMessage } from "./ipc";
import { openMemory } from "./memoryview";
import { paneFont } from "./zoom";
import { createReviewDrawer } from "./inboxreview";
import { openSettings } from "./settingsmodal";
import { openNewAgent } from "./inboxnew";
import { openPalette, closePalette, paletteOpen, type PaletteItem } from "./inboxpalette";
import { createHistoryDrawer } from "./inboxhistory";
import { createSessionsView } from "./sessionsview";
import { mountStart, renderStart, unmountStart } from "./inboxstart";
import { getPref } from "./prefs";
import { activateWorkspace, removeWorkspace, renameWorkspace } from "./workspace";
import { confirmModal } from "./confirmmodal";
import { openMenu, type MenuItem } from "./ctxmenu";
import { openHelpPage, openTool, isHelpKey } from "./inboxhelp";
import { showTip, tipSeen } from "./tips";
import { tokens } from "./transcript";
import { sourceOf } from "./chatsource";
import { agentUsage, onAgentCost } from "./agentcost";
import { faceSvg } from "./agentface";
import { cliLogo } from "./clilogos";
import { planHtml, startPlanMeter, stopPlanMeter } from "./planmeter";
import { keepLines } from "./terminal";
import { handOff, handOverToAgent, handoffTargets, openTargets, startLimitGuard, stopLimitGuard } from "./limitguard";
import { startFinishWatch, stopFinishWatch } from "./finishwatch";
import { startSessionFollow, stopSessionFollow } from "./sessionfollow";
import { money } from "./pricing";
import { topNote } from "./hint";
import { profileOf } from "./cliprofile";
import { allTasks, answerOption, onTasksChange, taskOf, type Task } from "./tasks";
import type { AskOption } from "./askparse";
import { clockOf, type TaskState } from "./taskstate";
import type { Pane, Workspace } from "./panetypes";
import { hiddenGate } from "./hiddengate";

/* ---------------- pure helpers (tested) ---------------- */

const ORDER: TaskState[] = ["needs", "review", "working", "idle", "stopped", "merged"];
export const GROUP_LABEL: Record<TaskState, string> = {
  needs: "Needs you", review: "Ready to review", working: "Working", idle: "Idle", stopped: "Stopped", merged: "Merged",
};

/** Tasks grouped in queue order; empty groups dropped except "Needs you",
 *  which stays so an empty inbox can say so. */
export function groupTasks(list: Task[]): Array<{ state: TaskState; tasks: Task[] }> {
  return ORDER.map((state) => ({ state, tasks: list.filter((t) => t.status.state === state) }))
    .filter((g) => g.tasks.length || g.state === "needs");
}

/** Where things sit in the list: projects in the order you opened them, agents
 *  in the order they were made. A state change never moves anything. */
export interface ListOrder { ws: Map<string, number>; pane: Map<string, number> }

/** The queue by project, in a fixed order: nothing jumps when an agent's state changes. */
export function projectGroups(list: Task[], order?: ListOrder): Array<{ wsId: string; name: string; tasks: Task[]; needs: number }> {
  const byWs = new Map<string, { wsId: string; name: string; tasks: Task[]; needs: number }>();
  for (const t of list) {
    const g = byWs.get(t.wsId) ?? { wsId: t.wsId, name: t.project, tasks: [], needs: 0 };
    g.tasks.push(t);
    if (t.status.state === "needs") g.needs++;
    byWs.set(t.wsId, g);
  }
  const at = (m: Map<string, number> | undefined, id: string, fallback: number) => m?.get(id) ?? fallback;
  const groups = [...byWs.values()];
  // Unknown ones keep the order they came in, after the known ones.
  for (const g of groups) {
    const was = new Map(g.tasks.map((t, k) => [t.paneId, k]));
    g.tasks.sort((a, b) => at(order?.pane, a.paneId, 1e6 + was.get(a.paneId)!) - at(order?.pane, b.paneId, 1e6 + was.get(b.paneId)!));
  }
  const came = new Map(groups.map((g, k) => [g.wsId, k]));
  return groups.sort((a, b) => at(order?.ws, a.wsId, 1e6 + came.get(a.wsId)!) - at(order?.ws, b.wsId, 1e6 + came.get(b.wsId)!));
}

/** Projects whose stopped agents you folded away in the list, remembered. */
const FOLDED_KEY = "maestro.inbox.folded";
const foldedProjects = new Set<string>((() => {
  try { return JSON.parse(localStorage.getItem(FOLDED_KEY) || "[]") as string[]; } catch { return []; }
})());
function saveFolded(): void {
  try { localStorage.setItem(FOLDED_KEY, JSON.stringify([...foldedProjects])); } catch { /* storage blocked */ }
}
/** What a project's rows show: all of them, or folded, only the agents still
 *  going (a stopped or merged one is out of the way until you unfold it). */
function shownTasks(g: { wsId: string; tasks: Task[] }): Task[] {
  return foldedProjects.has(g.wsId) ? g.tasks.filter((t) => t.status.state !== "stopped" && t.status.state !== "merged") : g.tasks;
}

/** The fixed order of the projects and agents open now. */
export function listOrder(): ListOrder {
  const ws = new Map<string, number>();
  const pane = new Map<string, number>();
  let n = 0;
  [...workspaces.keys()].forEach((id, k) => ws.set(id, k));
  for (const w of workspaces.values()) for (const id of w.panes.keys()) pane.set(id, n++);
  return { ws, pane };
}

/** The top-bar sentence: "2 agents need you. 1 is ready to review, 3 are working." */
export function headline(list: Task[]): { lead: string; rest: string } {
  const n = (s: TaskState) => list.filter((t) => t.status.state === s).length;
  const needs = n("needs"), review = n("review"), working = n("working");
  const are = (k: number) => (k === 1 ? "is" : "are");
  if (!list.length) return { lead: "No agents yet.", rest: " Start one with New agent." };
  const lead = needs ? `${needs} agent${needs === 1 ? " needs" : "s need"} you.` : "Nothing needs you.";
  // Only what is so: the clauses with nobody in them are left out.
  const parts = [
    review ? `${review} ${are(review)} ready to review` : "",
    working ? `${working} ${are(working)} working` : "",
  ].filter(Boolean);
  if (!parts.length) {
    const idle = n("idle"), stopped = n("stopped"), merged = n("merged");
    // Idle after working is done; idle before its first job is waiting for one.
    const done = list.filter((t) => t.status.state === "idle" && t.worked).length;
    if (done) parts.push(`${done} ${are(done)} done`);
    if (idle - done) parts.push(`${idle - done} ${are(idle - done)} waiting for a task`);
    if (stopped) parts.push(`${stopped} ${are(stopped)} stopped`);
    if (merged) parts.push(`${merged} ${are(merged)} merged`);
  }
  return { lead, rest: parts.length ? ` ${parts.join(", ")}.` : "" };
}

/** Short second line of a queue row, from the agent's side. */
export function rowLine(t: Task): string {
  const a = t.status.ask;
  const lim = t.status.limit;
  if (lim) return lim.resetsAt ? `Out of usage · back ${clockOf(lim.resetsAt)}` : "Out of usage";
  switch (t.status.state) {
    case "needs":
      if (!a) return "Needs you";
      if (a.kind === "run") return a.detail ? `Wants to run ${a.detail}` : "Wants to run a command";
      if (a.kind === "edit") return a.detail ? `Wants to edit ${a.detail}` : "Wants to edit a file";
      return `Asks: ${a.prompt}`;
    case "review": return `${t.changedFiles ?? 0} file${t.changedFiles === 1 ? "" : "s"} changed`;
    case "working": return "Working";
    case "stopped": return "Stopped";
    case "merged": return `Merged into ${t.mergedInto ?? "the base branch"}`;
    default: return t.worked ? "Done" : "Waiting for a task";
  }
}

/** "+148 −455" at the end of a row that has changes to review. */
export function lineCounts(t: Pick<Task, "added" | "removed">): string {
  if (t.added == null || t.removed == null || (t.added === 0 && t.removed === 0)) return "";
  return `<span class="iq-ln"><b class="a">+${t.added}</b> <b class="d">−${t.removed}</b></span>`;
}

export function ago(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.round(s / 60);
  return m < 60 ? `${m}m` : `${Math.round(m / 60)}h`;
}

/** The short word a permission button wears ("Allow", "Always allow here",
 *  "Deny"); the CLI's full wording stays in the tooltip. */
export function shortLabel(o: AskOption): string {
  if (o.deny) return "Deny";
  if (o.always) return "Always allow here";
  return "Allow";
}

/** Permission prompts (run a command, edit a file) with the usual three
 *  choices get one row of buttons; anything else keeps the numbered list. */
export function isYesNo(a: { kind: string; options: AskOption[] }): boolean {
  return a.kind !== "question" && a.options.length <= 3 &&
    a.options.filter((o) => !o.deny && !o.always).length === 1;
}

export const MAX_SPLIT = 9;

/** Pin or unpin an agent for Split. A fifth pin pushes out the oldest one,
 *  never `keep` (the agent on the stage). */
export function togglePin(pins: string[], id: string, keep?: string, max: number = MAX_SPLIT): string[] {
  if (pins.includes(id)) return pins.filter((x) => x !== id);
  const next = [...pins, id];
  while (next.length > max) {
    const i = next.findIndex((x) => x !== keep && x !== id);
    next.splice(i < 0 ? 0 : i, 1);
  }
  return next;
}


/** Who is in Split: the agents already there (so nobody jumps around), topped
 *  up from the queue order to the Split size in Settings, skipping the ones you closed. */
export function fillPins(pins: string[], order: string[], closed: Set<string>, max: number = MAX_SPLIT): string[] {
  const out = pins.filter((id) => order.includes(id) && !closed.has(id));
  for (const id of order) {
    if (out.length >= max) break;
    if (!out.includes(id) && !closed.has(id)) out.push(id);
  }
  return out.slice(-max);
}

/** Who may sit in the Grid, in queue order: agents that are alive. A stopped
 *  one leaves it and comes back once it is resumed. */
export function gridOrder(list: Pick<Task, "paneId" | "status">[]): string[] {
  return list.filter((t) => t.status.state !== "stopped" && t.status.state !== "merged").map((t) => t.paneId);
}

/** The Grid's cards by project, the terminal's own project first and the rest
 *  by name, so agents of one project sit together (several projects can each
 *  have an "Ana"). Within a project they keep the order they came in. */
export function railGroups<T extends Pick<Task, "wsId" | "project">>(list: T[], homeWs: string | null): { wsId: string; project: string; tasks: T[] }[] {
  const groups: { wsId: string; project: string; tasks: T[] }[] = [];
  for (const t of list) {
    const g = groups.find((x) => x.wsId === t.wsId);
    if (g) g.tasks.push(t); else groups.push({ wsId: t.wsId, project: t.project, tasks: [t] });
  }
  return groups.sort((a, b) => Number(b.wsId === homeWs) - Number(a.wsId === homeWs) || a.project.localeCompare(b.project) || a.wsId.localeCompare(b.wsId));
}

/* ---------------- DOM ---------------- */

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

let queueEl: HTMLElement | null = null;
let headEl: HTMLElement | null = null;
/** The chip over the stage that opens Review when an agent finished with changes. */
let askEl: HTMLElement | null = null;
/** What the stage says when the shown project has no agents. */
let emptyEl: HTMLElement | null = null;
let timer: number | null = null;
let offTasks: (() => void) | null = null;
/** Split (the Grid): one agent's terminal with the others as cards. Only the pins in
 *  the shown project can be on screen, since each project has its own canvas. */
let splitOn = false;
let pins: string[] = [];
let splitSig = "";
let splitWs: Workspace | null = null;
/** The agent in the Grid's terminal; the others are cards beside it. */
let bigId: string | null = null;
/** Agents of other projects shown in Split, with the project they belong to. */
const borrowed = new Map<string, Workspace>();
/** Agents closed out of Split with ✕; they stay out until Split is turned off. */
const splitClosed = new Set<string>();
/** Grid cards answered from a button: which question, so its buttons go. */
const miniAnswered = new Map<string, string>();
let splitBtn: HTMLButtonElement | null = null;
let reviewer: ReturnType<typeof createReviewDrawer> | null = null;
let historian: ReturnType<typeof createHistoryDrawer> | null = null;
const sessions = createSessionsView();
let dockEl: HTMLElement | null = null;
function paneOf(t: Task): Pane | undefined {
  return workspaces.get(t.wsId)?.panes.get(t.paneId);
}

function wsOf(id: string): Workspace | undefined {
  for (const ws of workspaces.values()) if (ws.panes.has(id)) return ws;
  return undefined;
}

function paneById(id: string): Pane | undefined {
  return wsOf(id)?.panes.get(id);
}

/** Panes on screen: the shown project's own, plus agents of other projects
 *  that Split has borrowed onto this screen. */
function screenPanes(): Pane[] {
  if (!activeWs) return [];
  const out = [...activeWs.panes.values()];
  for (const id of borrowed.keys()) {
    const p = paneById(id);
    if (p && !out.includes(p)) out.push(p);
  }
  return out;
}

/** The pane on the stage: the focused one on screen. */
function stagePane(): Pane | undefined {
  return screenPanes().find((p) => p.el.classList.contains("focused"));
}

/** Put a task's agent on the stage (switching project if needed). */
export function openTask(t: Task): void {
  const ws = workspaces.get(t.wsId);
  const pane = ws?.panes.get(t.paneId);
  if (!ws || !pane) return;
  // In Split, picking an agent (from any project) adds it next to the others
  // instead of replacing the whole view.
  if (splitOn && stagePane()) {
    splitClosed.delete(pane.id);
    if (!pins.includes(pane.id)) pins = togglePin(pins, pane.id, stagePane()?.id, getPref("splitMax"));
    render(); // borrows it onto this screen if it lives in another project
    setCurrent(pane);
    return;
  }
  revealPane(t.wsId, t.paneId);
  focusPane(ws, pane);
  render();
}

/** Make a pane the one the decision card and Alt+number talk to, without
 *  moving or refitting anything (Split keeps every terminal where it is). */
function setCurrent(pane: Pane): void {
  // in the Grid it is the one in the terminal: the layout moves it there
  if (splitOn) bigId = pane.id;
  for (const p of screenPanes()) p.el.classList.toggle("focused", p === pane);
  render();
  pane.term.focus();
}

/* ---------------- Split ---------------- */

function refit(pane: Pane, font: number): void {
  pane.term.setFontSize(font);
  const s = pane.term.fit();
  if (pane.running) void resizePty(pane.id, s.cols, s.rows).catch(() => {});
}

const SPLIT_VARS = ["--sx", "--sy", "--sw", "--sh"];

/** Give a borrowed pane back to its own project's canvas. */
function giveBack(id: string): void {
  const home = borrowed.get(id);
  borrowed.delete(id);
  const p = paneById(id);
  if (!home || !p) return;
  p.el.classList.remove("split-pin", "ib-needs");
  for (const v of SPLIT_VARS) p.el.style.removeProperty(v);
  // Its project may already have a pane on its own stage.
  if ([...home.panes.values()].some((x) => x !== p && x.el.classList.contains("focused"))) p.el.classList.remove("focused");
  home.gridEl.appendChild(p.el);
}

function clearSplit(ws: Workspace): void {
  ws.gridEl.classList.remove("inbox-split");
  for (const id of [...borrowed.keys()]) giveBack(id);
  for (const p of ws.panes.values()) {
    if (!p.el.classList.contains("split-pin")) continue;
    p.el.classList.remove("split-pin");
    for (const v of SPLIT_VARS) p.el.style.removeProperty(v);
  }
  const stage = [...ws.panes.values()].find((p) => p.el.classList.contains("focused"));
  requestAnimationFrame(() => { if (stage) refit(stage, paneFont(ws, 2)); });
}

/** The list stepped aside for the Grid: said once while the app runs. */
let wideNoted = false;

/** The list steps aside while the Grid's terminal can't be read beside it and
 *  the cards (a laptop screen). Judged as if the list were there, so it can't
 *  flip back and forth once it has gone. True when it just moved. */
function widenGrid(on: boolean, area: Area, font: number, cards: number): boolean {
  const body = document.body;
  const wide = body.classList.contains("ib-grid-wide");
  let next = false;
  if (on) {
    // the list's column, less the gutter the stage gets back when it is gone
    const list = (parseFloat(getComputedStyle(body).getPropertyValue("--ib-q")) || 320) - 16;
    next = !railPlan(cards, wide ? { width: area.width - list, height: area.height } : area, font).readable;
  }
  if (next === wide) return false;
  body.classList.toggle("ib-grid-wide", next);
  requestAnimationFrame(() => render());
  return true;
}

/** The Grid: the agent you work with as a big real terminal, every other one
 *  in it as a card beside it (gridfit.ts, gridcards.ts), from every project.
 *  Runs on every render but only moves the terminal (and resizes its PTY) when
 *  which agent it is, or the space, changed. */
function layoutSplit(): void {
  const ws = activeWs;
  if (!ws) return;
  if (splitOn) pins = fillPins(pins, gridOrder(allTasks()), splitClosed, getPref("splitMax"));
  const inGrid = splitOn ? pins.map(paneById).filter((p): p is Pane => !!p) : [];
  const on = inGrid.length > 1;
  const cur = stagePane();
  const big = on ? inGrid.find((p) => p.id === bigId) ?? (cur && inGrid.includes(cur) ? cur : inGrid[0]) : undefined;
  bigId = big?.id ?? bigId;
  const cards = big ? inGrid.filter((p) => p !== big) : [];
  const area = { width: ws.gridEl.clientWidth, height: ws.gridEl.clientHeight };
  const font = paneFont(ws);
  const moved = widenGrid(on, area, font, cards.length);
  const plan = big ? railPlan(cards.length, area, font) : null;
  const sig = big && plan ? `${ws.id}|${big.id}|${plan.cols}|${area.width}x${area.height}` : "";
  if (sig !== splitSig) {
    splitSig = sig;
    // Leaving the Grid, or moving to another project, puts the old canvas back.
    if (splitWs && (!big || splitWs !== ws)) clearSplit(splitWs);
    splitWs = big ? ws : null;
    if (big && plan) {
      for (const id of [...borrowed.keys()]) if (id !== big.id) giveBack(id);
      const home = wsOf(big.id);
      if (home && home !== ws && !borrowed.has(big.id)) {
        borrowed.set(big.id, home);
        ws.gridEl.appendChild(big.el);
      }
      ws.gridEl.classList.add("inbox-split", "has-focus");
      for (const p of screenPanes()) {
        const isBig = p === big;
        p.el.classList.toggle("split-pin", isBig);
        p.el.classList.toggle("focused", isBig);
        if (!isBig) { for (const v of SPLIT_VARS) p.el.style.removeProperty(v); continue; }
        p.el.style.setProperty("--sx", `${plan.big.x}px`);
        p.el.style.setProperty("--sy", `${plan.big.y}px`);
        p.el.style.setProperty("--sw", `${plan.big.w}px`);
        p.el.style.setProperty("--sh", `${plan.big.h}px`);
      }
      requestAnimationFrame(() => refit(big, plan.font));
      // the list is about to step aside: the note waits for the layout that follows
      if (!moved && !wideNoted && document.body.classList.contains("ib-grid-wide")) {
        wideNoted = true;
        topNote("The list steps aside so the terminal has room to read · <kbd>Alt+S</kbd> brings it back");
      }
    }
  }
  renderRail(ws, big, cards, plan?.rail ?? null, plan?.cols ?? 0);
}

/* ---------------- the Grid's cards ---------------- */

let railEl: HTMLElement | null = null;
let railTimer: number | null = null;
/** What the cards show now, so an unchanged render leaves them (and a hovered button) alone. */
let railHtml = "";
/** The agents on cards now, for the reader. */
let railPanes: Pane[] = [];
/** How often a card reads what its agent wrote. */
const RAIL_READ_MS = 2000;

/** One card: who, its state and job, what it is doing and last said, and its
 *  answer buttons while it asks you something. */
function cardHtml(t: Task, p: Pane): string {
  const g = glanceFor(p.id);
  const a = t.status.state === "needs" ? t.status.ask : null;
  let body: string;
  if (a && miniAnswered.get(p.id) !== askKey(a)) {
    const lead = a.kind === "run" ? "Wants to run" : a.kind === "edit" ? "Wants to edit" : "Asks";
    const what = a.kind !== "question" && a.detail ? `<code title="${esc(a.detail)}">${esc(a.detail)}</code>` : `<span class="rc-q">${esc(a.prompt)}</span>`;
    body = `<div class="rc-ask"><p><span class="lb">${lead}</span>${what}</p><div class="rc-opts">${askButtons(a).join("")}</div></div>`;
  } else {
    const doing = t.status.state === "working" && !t.status.limit ? g.doing || (g.asked ? "Working on what you asked" : "Working") : rowLine(t);
    const said = g.said ? `<p class="rc-say">${esc(g.said)}</p>` : g.asked ? `<p class="rc-say rc-you"><span>You</span> ${esc(g.asked)}</p>` : "";
    body = `<p class="rc-do">${esc(doing)}${lineCounts(t)}</p>${said}`;
  }
  const color = p.color ?? "#888";
  return `<div class="rc st-${t.status.state}${t.status.limit ? " limited" : ""}" role="listitem" tabindex="0" data-card="${esc(p.id)}" aria-label="${esc(t.name)}, in ${esc(t.project)}" title="${esc(t.name)} in ${esc(t.project)} · click to work with it in the terminal, right-click for more">
    <div class="rc-h"><span class="iq-mk" style="--mk:${esc(color)}" aria-hidden="true">${faceSvg(t.name, color)}</span><b class="rc-n">${esc(t.name)}</b>${p.spec.badge ? `<span class="rc-cli">${cliLogo(p.spec.badge)}${esc(p.spec.badge)}</span>` : ""}<span class="ib-pill st-${t.status.state}">${GROUP_LABEL[t.status.state]}</span></div>
    ${t.title ? `<p class="rc-t" title="${esc(t.title)}">${esc(t.title)}</p>` : ""}${body}
  </div>`;
}

function renderRail(ws: Workspace, big: Pane | undefined, cards: Pane[], tile: Tile | null, cols: number): void {
  if (!tile || !cards.length) {
    railEl?.remove();
    railEl = null;
    railPanes = [];
    if (railTimer !== null) { window.clearInterval(railTimer); railTimer = null; }
    keepGlances(new Set());
    return;
  }
  if (!railEl || railEl.parentElement !== ws.gridEl) {
    railEl?.remove();
    railHtml = "";
    railEl = document.createElement("div");
    railEl.className = "ib-rail";
    railEl.setAttribute("role", "list");
    railEl.setAttribute("aria-label", "The other agents in the Grid");
    railEl.addEventListener("click", onRailClick);
    railEl.addEventListener("keydown", onRailKey);
    railEl.addEventListener("contextmenu", onRailMenu);
    ws.gridEl.appendChild(railEl);
  }
  const st = railEl.style;
  st.left = `${tile.x}px`; st.top = `${tile.y}px`; st.width = `${tile.w}px`; st.height = `${tile.h}px`;
  st.setProperty("--rail-cols", String(cols));
  const list = allTasks();
  const home = big ? list.find((x) => x.paneId === big.id)?.wsId ?? null : null;
  const shown = cards.flatMap((p) => { const t = list.find((x) => x.paneId === p.id); return t ? [{ ...t, pane: p }] : []; });
  const groups = railGroups(shown, home);
  // A heading names the project above its cards, unless every card shares the
  // terminal's project: then there is nothing to tell apart.
  const heads = groups.length > 1 || (groups[0] && groups[0].wsId !== home);
  const html = groups.map((g) => {
    const head = heads ? `<div class="rc-g" role="none"><b>${esc(g.project)}</b>${g.wsId === home ? "<span>the terminal's project</span>" : ""}</div>` : "";
    return head + g.tasks.map((t) => cardHtml(t, t.pane)).join("");
  }).join("");
  if (railHtml !== html) { railHtml = html; railEl.innerHTML = html; }
  railPanes = groups.flatMap((g) => g.tasks.map((t) => t.pane));
  keepGlances(new Set(cards.map((p) => p.id)));
  if (railTimer === null) {
    const read = () => void Promise.all(railPanes.map((p) => refreshGlance(p))).then((ch) => { if (ch.some(Boolean)) render(); });
    read();
    railTimer = window.setInterval(read, RAIL_READ_MS);
  }
}

/** A card makes its agent the one in the terminal; its buttons answer it. */
function onRailClick(e: MouseEvent): void {
  const target = e.target as HTMLElement;
  const card = target.closest<HTMLElement>("[data-card]");
  const pane = card ? paneById(card.dataset.card ?? "") : undefined;
  if (!pane) return;
  const opt = target.closest<HTMLElement>(".rc-opts [data-n]");
  if (opt) {
    const a = taskOf(pane.id)?.status.ask;
    const o = a?.options.find((x) => x.n === Number(opt.dataset.n));
    if (!a || !o) return;
    miniAnswered.set(pane.id, askKey(a));
    void answerOption(pane.id, o).then(() => render());
    return;
  }
  setCurrent(pane);
}

function onRailKey(e: KeyboardEvent): void {
  if (e.key !== "Enter" && e.key !== " ") return;
  const card = (e.target as HTMLElement).closest<HTMLElement>("[data-card]");
  if (!card || e.target !== card) return;
  e.preventDefault();
  const pane = paneById(card.dataset.card ?? "");
  if (pane) setCurrent(pane);
}

function onRailMenu(e: MouseEvent): void {
  const card = (e.target as HTMLElement).closest<HTMLElement>("[data-card]");
  const t = card ? taskOf(card.dataset.card ?? "") : undefined;
  if (!t) return;
  e.preventDefault();
  openMenu(e.clientX, e.clientY, agentMenu(t), `${t.name} actions`);
}

export function setSplit(on: boolean): void {
  splitOn = on;
  if (on && gridOrder(allTasks()).length < 2) {
    const parked = allTasks().some((t) => t.status.state === "stopped");
    topNote(parked
      ? "The Grid shows a running agent with the others beside it: resume a stopped one or start another with <b>New agent</b>"
      : "The Grid shows an agent with the others beside it: start another with <b>New agent</b>");
  }
  if (!on) { splitClosed.clear(); miniAnswered.clear(); }
  splitBtn?.setAttribute("aria-pressed", String(on));
  dockEl?.querySelector('[data-dock="queue"]')?.setAttribute("aria-pressed", String(!on));
  render();
}

/** ✕ on a Split card: take that agent out, hand the focus to another one. */
function closeFromSplit(id: string): void {
  splitClosed.add(id);
  pins = pins.filter((x) => x !== id);
  if (stagePane()?.id === id) {
    const next = pins.map(paneById).find(Boolean);
    if (next) { render(); setCurrent(next); return; }
  }
  render();
}

function pinCurrent(): void {
  const cur = stagePane();
  if (!cur) return;
  pins = togglePin(pins, cur.id, cur.id, getPref("splitMax"));
  if (!splitOn) topNote(pins.includes(cur.id) ? `<b>${esc(cur.spec.name)}</b> is in the Grid · <kbd>Alt+S</kbd> shows it` : `<b>${esc(cur.spec.name)}</b> is out of the Grid`);
  if (!pins.includes(cur.id) && splitOn) {
    splitClosed.add(cur.id);
    // Taking the agent on the stage out hands the stage to another card.
    const next = pins.map(paneById).find(Boolean);
    if (next) { render(); setCurrent(next); return; }
  }
  render();
}

/** Make sure something is on the stage: the first task that needs you in the
 *  shown project, else its first pane. */
function ensureStage(): void {
  if (!activeWs) return;
  const cur = stagePane();
  // A pane can come back from Split (or another project) still marked as the
  // stage while its grid never became one; without this it shows as a canvas tile.
  if (cur) {
    if (!activeWs.gridEl.classList.contains("has-focus") && activeWs.panes.has(cur.id)) focusPane(activeWs, cur);
    return;
  }
  // The task list is re-derived on its own tick, so right after a pane goes it
  // can still name it; only a pane that exists can take the stage.
  const inWs = allTasks().filter((t) => t.wsId === activeWs!.id && activeWs!.panes.has(t.paneId));
  const first = (inWs[0] && activeWs.panes.get(inWs[0].paneId)) ?? activeWs.panes.values().next().value;
  if (first) focusPane(activeWs, first);
}

/** What the right-click menu on an agent offers. */
/** Hand an agent's task to another CLI and say so. */
async function handOverTo(paneId: string, from: string, presetId: string, label: string): Promise<void> {
  topNote(`Starting a ${esc(label)} agent to carry on <b>${esc(from)}</b>'s task…`);
  const name = await handOff(paneId, presetId, taskOf(paneId)?.status.limit ? "it hit its plan's usage limit" : "you asked another CLI to carry on");
  topNote(name ? `<b>${esc(name)}</b> (${esc(label)}) carries on <b>${esc(from)}</b>'s task, with a brief of what it did` : `Couldn't start a ${esc(label)} agent`);
  render();
}

/** Why an agent's task is being handed over, in the words the next agent reads. */
const handoverWhy = (paneId: string) => (taskOf(paneId)?.status.limit ? "it hit its plan's usage limit" : "you asked another agent to carry on");

/** An open agent takes the task over, and says so. */
async function handOverToOpen(paneId: string, from: string, toId: string): Promise<void> {
  const name = await handOverToAgent(paneId, toId, handoverWhy(paneId));
  topNote(name ? `<b>${esc(name)}</b> carries on <b>${esc(from)}</b>'s task, reading what it did` : `Couldn't hand <b>${esc(from)}</b>'s task over`);
  render();
}

/** Who can carry on an agent's task: the agents already open in its folder
 *  first, then a new agent of another installed CLI. */
function handoverItems(pane: Pane, t: Task): MenuItem[] {
  const open = openTargets(pane).slice(0, 3).map((o) => ({ label: `Hand over to ${o.name} (${o.cli})`, run: () => { void handOverToOpen(pane.id, pane.spec.name, o.paneId); } }));
  const fresh = handoffTargets(pane).slice(0, Math.max(2, 5 - open.length)).map((c) => ({ label: `Hand over to a new ${c.label} agent`, run: () => { void handOverTo(pane.id, pane.spec.name, c.id, c.label); } }));
  return [...open, ...fresh].map((it, k) => ({ ...it, sep: k === 0, hint: k === 0 && t.status.limit ? "Out of usage" : undefined }));
}

export function agentMenu(t: Task): MenuItem[] {
  const pane = paneOf(t);
  const ws = workspaces.get(t.wsId);
  if (!pane || !ws) return [];
  const pinned = pins.includes(pane.id);
  const branch = pane.spec.branch;
  const click = (sel: string) => pane.el.querySelector<HTMLElement>(sel)?.click();
  const onStage = () => { openTask(t); };
  // Alt+R / Alt+H / Alt+P act on the agent on the stage, so they are only
  // worth showing on that agent's menu.
  const here = stagePane()?.id === pane.id;
  const key = (k: string) => (here ? k : undefined);
  return [
    ...(t.status.state === "merged" ? [{ label: "Clear from list", run: () => clearMerged([t.paneId]) }] : []),
    { label: "Open", run: onStage },
    { label: "Rename…", run: () => void confirmModal({ title: "Rename agent", message: "What should this agent be called?", okLabel: "Rename", input: { value: pane.spec.name } })
      .then((r) => { if (r.ok) { renamePane(pane, r.value); render(); } }) },
    { label: pinned ? "Take out of Grid" : "Add to Grid", hint: key("Alt+P"), run: () => {
      if (pinned) { pins = pins.filter((id) => id !== pane.id); if (splitOn) splitClosed.add(pane.id); }
      else { splitClosed.delete(pane.id); pins = togglePin(pins, pane.id, stagePane()?.id, getPref("splitMax")); if (!splitOn) topNote(`<b>${esc(pane.spec.name)}</b> is in the Grid · <kbd>Alt+S</kbd> shows it`); }
      render();
    } },
    { label: "Changes", hint: key("Alt+R"), sep: true, disabled: !pane.spec.worktree && !pane.spec.cwd, run: () => { onStage(); openReview(); } },
    { label: "History", hint: key("Alt+H"), run: () => { onStage(); openHistory(); } },
    { label: "Project memory", run: () => openProjectMemory(t.wsId) },
    { label: "Copy branch name", disabled: !branch, run: () => { if (branch) void navigator.clipboard?.writeText(branch).catch(() => {}); } },
    // Another agent carries on its task, in the same folder, from its conversation.
    ...handoverItems(pane, t),
    // Starts it again in place; a Claude agent carries on its own conversation.
    { label: pane.running ? "Restart" : "Resume", sep: true, run: () => void pane.restart?.() },
    ...(sourceOf(pane) ? [{ label: "New conversation", run: () => void pane.restart?.({ fresh: true }) }] : []),
    { label: "Stop", disabled: !pane.running, run: () => { void killPty(pane.id).catch(() => {}); } },
    { label: "Remove agent…", danger: true, run: () => void confirmModal({
      title: `Remove ${pane.spec.name}?`,
      message: pane.running ? "It stops and its terminal closes. Its branch and the changes on it stay in git." : "Its terminal closes. Its branch and the changes on it stay in git.",
      okLabel: "Remove",
      danger: true,
    }).then((r) => { if (r.ok) click("[data-kill]"); }) },
  ];
}

function renderQueue(list: Task[], current: string | undefined, now: number): void {
  if (!queueEl) return;
  // One group per project, in a fixed order (projects as you opened them,
  // agents as they were made): nothing moves when a state changes. Who needs
  // you is marked where it is, and counted on its project, which folds its
  // stopped agents away and keeps the running ones.
  const groups = projectGroups(list, listOrder());
  const html = `<div class="iq-list">` +
    (groups.length ? groups.map((g, gi) => {
      const folded = foldedProjects.has(g.wsId);
      const rows = shownTasks(g);
      const away = g.tasks.length - rows.length;
      return `<section class="iq-group${folded ? " folded" : ""}${g.needs ? " has-needs" : ""}">
      <h2 class="iq-gt iq-proj"><button type="button" class="iq-ph" data-proj="${esc(g.wsId)}" aria-expanded="${!folded}" aria-controls="iq-r-${gi}" title="${esc(g.name)}${folded ? (away ? ` · show ${away} stopped` : " · unfold") : " · fold the stopped away"}">
        <svg class="iq-chev" viewBox="0 0 10 10" aria-hidden="true"><path d="M3.5 2.5 6 5 3.5 7.5" /></svg><span class="iq-pn">${esc(g.name)}</span>${g.needs ? `<span class="iq-need" aria-label="${g.needs} need${g.needs === 1 ? "s" : ""} you">${g.needs}</span>` : ""}<span class="iq-n">${g.tasks.length}</span></button><button type="button" class="iq-mem" data-mem="${esc(g.wsId)}" title="Project memory: what every agent in ${esc(g.name)} knows" aria-label="${esc(g.name)} memory"><svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="M3.5 2.5h7.5a1.5 1.5 0 0 1 1.5 1.5v9.5H5a1.5 1.5 0 0 1-1.5-1.5z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><path d="M3.5 12a1.5 1.5 0 0 1 1.5-1.5h7.5M6.5 5.5h3.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg></button></h2>
      <ul class="iq-rows" id="iq-r-${gi}"${rows.length ? "" : " hidden"}>${rows.map((t) => {
        const p = paneOf(t);
        const main = t.title ?? t.name;
        const said = rowLine(t);
        // The state in words, unless the line already is the state ("Needs you", "Stopped").
        const state = GROUP_LABEL[t.status.state];
        const line = said === state || t.status.limit || t.status.state === "merged" ? said : `${state} · ${said}`;
        // what it has used: money when its CLI prices it, else tokens; its context once nearly full
        const use = p ? agentUsage(p) : null;
        const cost = use?.usd ? `<span class="iq-cost" title="${p?.spec.badge === "claude" ? "What its conversation has used so far, at API prices" : "What its conversation has cost so far, as its CLI says"}">${money(use.usd)}</span>`
          : use?.tokens ? `<span class="iq-cost" title="Tokens its conversation has used so far">${tokens(use.tokens)} tokens</span>` : "";
        const full = use?.contextPct != null && use.contextPct >= 80 ? `<span class="iq-full" title="Its context is nearly full: it will soon sum up the conversation and lose detail">${Math.round(use.contextPct)}% context</span>` : "";
        return `<li><button class="iq-row st-${t.status.state}${t.status.limit ? " limited" : ""}" data-id="${esc(t.paneId)}" aria-current="${t.paneId === current}" aria-haspopup="menu" title="${esc(main)}${t.title ? ` · ${esc(t.name)}` : ""} · ${esc(g.name)} · right-click for more">
          <span class="iq-mk" style="--mk:${esc(p?.color ?? "#888")}" aria-hidden="true">${faceSvg(t.name, p?.color ?? "#888")}</span>
          <span class="iq-t">${esc(main)}</span>
          <span class="iq-tm">${pins.includes(t.paneId) && splitOn ? `<span class="iq-pin" title="In the Grid">◫</span> ` : ""}<span class="iq-ago" data-since="${t.since}">${ago(now - t.since)}</span></span>
          ${t.status.state === "stopped" ? `<span class="iq-resume" data-resume aria-hidden="true" title="Start ${esc(t.name)} again, in the conversation it was last in">Resume</span>` : ""}
          <span class="iq-more" data-more aria-hidden="true"><svg width="16" height="16" viewBox="0 0 16 16"><circle cx="3.5" cy="8" r="1.3" fill="currentColor"/><circle cx="8" cy="8" r="1.3" fill="currentColor"/><circle cx="12.5" cy="8" r="1.3" fill="currentColor"/></svg></span>
          <span class="iq-s">${t.title ? `<span class="iq-p">${esc(t.name)}</span><span class="iq-dot" aria-hidden="true">·</span>` : ""}${t.race ? `<span class="iq-race">race ${t.race.n}/${t.race.of}</span> ` : ""}<span class="iq-l">${esc(line)}</span>${lineCounts(t)}${cost}${full}</span>
        </button></li>`;
      }).join("")}</ul>
    </section>`;
    }).join("") : `<p class="iq-empty">No agents yet.</p>`) +
    clearMergedHtml(list) +
    `</div>` + planHtml(now);
  // Rebuilding every second would take the keyboard focus (and a click in
  // progress) away from the list; only a real change rebuilds it.
  const sig = html.replace(/<span class="iq-ago"[^>]*>[^<]*<\/span>/g, "");
  if (sig === queueSig) {
    for (const el of queueEl.querySelectorAll<HTMLElement>(".iq-ago")) el.textContent = ago(now - Number(el.dataset.since));
    return;
  }
  queueSig = sig;
  const had = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>(".inbox-queue [data-id]");
  const back = had?.dataset.id ? `.iq-row[data-id="${CSS.escape(had.dataset.id)}"]` : null;
  queueEl.innerHTML = html;
  if (back) queueEl.querySelector<HTMLElement>(back)?.focus({ preventScroll: true });
}
let queueSig = "";

/** Merged agents are done: one button takes them all off the list. */
function clearMergedHtml(list: Task[]): string {
  const n = list.filter((t) => t.status.state === "merged").length;
  return n ? `<button type="button" class="iq-clear" data-clear-merged>Clear ${n} merged</button>` : "";
}

/** Take merged agents off the list: their work is in, their worktree gone. */
function clearMerged(ids?: string[]): void {
  const done = allTasks().filter((t) => t.status.state === "merged" && (!ids || ids.includes(t.paneId)));
  for (const t of done) paneOf(t)?.el.querySelector<HTMLElement>("[data-kill]")?.click();
  if (done.length) topNote(`Cleared ${done.length} merged agent${done.length === 1 ? "" : "s"}`);
  render();
}

function renderHead(list: Task[]): void {
  if (!headEl) return;
  const h = headline(list);
  // A live region: only a real change is worth reading out.
  const html = `<b>${esc(h.lead)}</b>${esc(h.rest)}`;
  if (headEl.innerHTML !== html) headEl.innerHTML = html;
}

/** Headers of the agents on screen: the state pill everywhere; Changes and
 *  History on the stage; open-full-size and close on each Split card, plus
 *  the card's own answer box when that agent needs you. */
function renderHeaders(list: Task[], pane: Pane | undefined): void {
  for (const ws of workspaces.values()) for (const p of ws.panes.values()) {
    const pinned = p.el.classList.contains("split-pin");
    const t = p === pane || pinned ? list.find((x) => x.paneId === p.id) : undefined;
    let pill = p.el.querySelector<HTMLElement>(".ib-pill");
    let acts = p.el.querySelector<HTMLElement>(".ib-acts");
    p.el.classList.toggle("ib-needs", !!t && pinned && t.status.state === "needs");
    if (!t) { pill?.remove(); acts?.remove(); continue; }
    if (!pill) {
      pill = document.createElement("span");
      p.el.querySelector(".pane-bar .pb-sp")?.after(pill);
    }
    pill.className = `ib-pill st-${t.status.state}`;
    pill.textContent = GROUP_LABEL[t.status.state];
    const mode = pinned ? "split" : splitOn ? "grid" : "stage";
    const stopped = t.status.state === "stopped";
    const build = `${mode}|${t.name}|${t.race?.of ?? ""}|${stopped}`;
    if (!acts || acts.dataset.mode !== build) {
      acts?.remove();
      acts = document.createElement("span");
      acts.className = "ib-acts";
      acts.dataset.mode = build;
      acts.innerHTML = pinned
        ? `<button class="ib-icon" data-stage="full" title="Open full size" aria-label="Open ${esc(t.name)} full size">⤢</button><button class="ib-icon" data-stage="close" title="Take out of Grid" aria-label="Take ${esc(t.name)} out of the Grid">✕</button>`
        : stopped
          // stopped: nothing to type into, one button starts it again
          ? `<button class="ib-act ib-go" data-stage="resume" title="Start ${esc(t.name)} again, in the conversation it was last in">Resume</button>${handoffTargets(p).length || openTargets(p).length ? `<button class="ib-act" data-stage="handover" title="Another agent carries on ${esc(t.name)}'s task">Hand over</button>` : ""}<button class="ib-act" data-stage="review" title="What ${esc(t.name)} changed (Alt+R)">Changes</button><button class="ib-act" data-stage="history" title="What ${esc(t.name)} has done (Alt+H)">History</button>`
          : `<button class="ib-act ib-term" data-stage="cmds" title="Type / so ${esc(t.name)}'s CLI shows its own commands">Commands</button>${profileOf(p.spec.badge).modelCommand ? `<button class="ib-act ib-term" data-stage="model" title="Open the CLI's own model picker">Model</button>` : ""}<button class="ib-act" data-stage="review" title="What ${esc(t.name)} changed (Alt+R)">Changes</button><button class="ib-act" data-stage="history" title="What ${esc(t.name)} has done (Alt+H)">History</button>${t.race ? `<button class="ib-act" data-stage="compare" title="Everyone on this job, side by side">Compare ${t.race.of}</button>` : ""}`;
      pill.after(acts);
    }
    acts.querySelector('[data-stage="review"]')?.setAttribute("aria-pressed", String(reviewer?.paneId === p.id));
    acts.querySelector('[data-stage="history"]')?.setAttribute("aria-pressed", String(historian?.paneId === p.id));
    // Just the branch: the project is already in the queue and the headline.
    // The Grid's terminal says its project first, as its cards are grouped by
    // project, then the job when there is one.
    const where = p.el.querySelector<HTMLElement>("[data-where]");
    if (where) { where.textContent = pinned ? [t.project, t.title ?? t.branch].filter(Boolean).join(" · ") : t.branch ?? ""; where.title = where.textContent; }
    // On the stage the job is the big line, the agent's name moves up with the CLI.
    let ttl = p.el.querySelector<HTMLElement>(".ib-ttl");
    const showTtl = !pinned && !!t.title;
    p.el.classList.toggle("ib-has-title", showTtl);
    if (!showTtl) ttl?.remove();
    else {
      if (!ttl) {
        ttl = document.createElement("span");
        ttl.className = "ib-ttl";
        ttl.setAttribute("role", "heading");
        ttl.setAttribute("aria-level", "1");
        const nameEl = p.el.querySelector(".pane-bar .pb-name");
        if (nameEl) nameEl.after(ttl); else p.el.querySelector(".pane-bar")?.appendChild(ttl);
      }
      ttl.textContent = t.title!;
      ttl.title = t.title!;
    }
  }
}

const askKey = (a: NonNullable<Task["status"]["ask"]>) => `${a.prompt}|${a.detail ?? ""}|${a.options.map((o) => o.label).join("/")}`;

/** The buttons that answer a prompt: Allow / Always / Deny, or its numbered choices. */
function askButtons(a: NonNullable<Task["status"]["ask"]>): string[] {
  return isYesNo(a)
    ? [...a.options].sort((x, y) => rank(x) - rank(y)).map((o) =>
        `<button class="ia-opt ${o.deny ? "deny" : o.always ? "always" : "allow"}" data-n="${o.n}" title="${esc(o.label)}">${esc(shortLabel(o))}</button>`)
    : a.options.map((o) => `<button class="ia-opt" data-n="${o.n}">${o.n}. ${esc(o.label)}</button>`);
}

/** An agent's questions are answered in its own terminal; the stage only
 *  carries a chip to Review when an agent finished with changes. */
function renderAsk(list: Task[], pane: Pane | undefined): void {
  if (!askEl) return;
  // No chip in Split: its cards say which agents are done.
  if (splitSig) { askEl.hidden = true; askEl.innerHTML = ""; askEl.dataset.sig = ""; return; }
  const t = pane ? list.find((x) => x.paneId === pane.id) : undefined;
  // An agent that finished with changes gets a quiet chip that opens Review.
  if (t && t.status.state === "review" && reviewer?.paneId !== t.paneId) {
    const sig = `review|${t.paneId}|${t.changedFiles ?? 0}`;
    if (askEl.dataset.sig === sig && !askEl.hidden) return;
    askEl.dataset.sig = sig;
    askEl.hidden = false;
    askEl.className = "inbox-ask min review";
    askEl.innerHTML = `<button class="ia-chip" data-act="review"><i class="ia-dot" aria-hidden="true"></i>${esc(t.name)} is ready · ${esc(rowLine(t))} · Review</button>`;
    return;
  }
  askEl.hidden = true; askEl.innerHTML = ""; askEl.dataset.sig = "";
}

const rank = (o: AskOption) => (o.always ? 0 : o.deny ? 1 : 2);

/** A project with no agents: say so, and offer the one thing to do. */
function renderEmpty(): void {
  if (!emptyEl) return;
  const ws = activeWs;
  const none = !!ws && ws.panes.size === 0;
  emptyEl.hidden = !none;
  if (!none || !ws) return;
  const sig = ws.id + ws.name;
  if (emptyEl.dataset.sig === sig) return;
  emptyEl.dataset.sig = sig;
  emptyEl.innerHTML = `<b>No agents in ${esc(ws.name)} yet</b><span>Say what you want done and one starts on it in this folder.</span>
    <button type="button" class="im-btn primary" data-empty-new>New agent</button>`;
}

/** Tips for what just became useful; showTip keeps them one at a time. */
function teach(list: Task[]): void {
  if (list.length >= 1 && !tipSeen("menu") && !splitOn) { if (showTip("menu")) return; }
  if (list.length >= 2 && !tipSeen("split") && !splitOn) showTip("split");
}

function render(): void {
  if (!document.body.classList.contains("inbox-ui")) return;
  ensureStage();
  layoutSplit();
  const list = allTasks();
  teach(list);
  const pane = stagePane();
  renderQueue(list, pane?.id, Date.now());
  renderHead(list);
  renderHeaders(list, pane);
  renderAsk(list, pane);
  renderEmpty();
  renderStart();
  keepHistory(pane);
  // History follows the stage: switch agents and it shows the new one's.
  if (historian?.paneId && pane && historian.paneId !== pane.id) historian.open(pane);
  historian?.refresh();
}

/** When each agent's terminal went out of sight. */
const hiddenSince = new Map<string, number>();
/** Terminals out of sight a while keep less history (keepLines): with many
 *  agents, full histories are the biggest use of memory. */
function keepHistory(stage: Pane | undefined, now = Date.now()): void {
  const shown = new Set<string>(splitOn ? pins : []);
  if (stage) shown.add(stage.id);
  for (const ws of workspaces.values()) for (const p of ws.panes.values()) {
    if (shown.has(p.id)) hiddenSince.delete(p.id);
    else if (!hiddenSince.has(p.id)) hiddenSince.set(p.id, now);
    p.term.setScrollback?.(keepLines(shown.has(p.id), now - (hiddenSince.get(p.id) ?? now)));
  }
}

function move(delta: number): void {
  // In the order the list shows them: project by project.
  const list = projectGroups(allTasks(), listOrder()).flatMap(shownTasks);
  if (!list.length) return;
  const cur = stagePane()?.id;
  const i = list.findIndex((t) => t.paneId === cur);
  const next = list[Math.max(0, Math.min(list.length - 1, (i < 0 ? 0 : i) + delta))];
  if (next) openTask(next);
}

/** A dialog is up (Settings, New agent, Help, the palette, a confirm). */
function dialogOpen(): boolean {
  return !!document.querySelector(".inbox-modal-back, .backdrop.open");
}

function onKey(e: KeyboardEvent): void {
  if (!document.body.classList.contains("inbox-ui") || !e.altKey || e.ctrlKey || e.metaKey) return;
  if (dialogOpen()) return;
  if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); move(e.key === "ArrowDown" ? 1 : -1); return; }
  const k = e.key.toLowerCase();
  if (k === "s") { e.preventDefault(); setSplit(!splitOn); return; }
  if (k === "p") { e.preventDefault(); pinCurrent(); return; }
  if (k === "r") { e.preventDefault(); if (reviewer?.paneId) reviewer.close(); else openReview(); return; }
  if (k === "h") { e.preventDefault(); if (historian?.paneId) historian.close(); else openHistory(); return; }
  if (k === "o") { e.preventDefault(); openSessions(); return; }
}

function openReview(): void {
  const pane = stagePane();
  if (!pane || !reviewer) return;
  if (historian?.paneId) historian.close();
  reviewer.open(pane);
  render();
}

/** Earlier Claude conversations of the project on screen (its agents' worktrees
 *  too), to read back; every folder's when no project is open. */
function openSessions(): void {
  const ws = activeWs;
  sessions.open(ws ? ws.repoRoot ?? ws.dir : null, ws?.name);
}

function openHistory(): void {
  const pane = stagePane();
  if (!pane || !historian) return;
  if (reviewer?.paneId) reviewer.close();
  if (historian?.paneId) historian.close();
  historian.open(pane);
  render();
}

/** Agents racing on one job, in their slot order. */
export function raceOf(list: Task[], id: string): Task[] {
  return list.filter((t) => t.race?.id === id).sort((a, b) => a.race!.n - b.race!.n);
}

/** Compare a race: every agent on the job with how far it got and what it
 *  changed; Review opens that agent's diff, where you merge the one you keep. */
function openCompare(raceId: string): void {
  const racers = raceOf(allTasks(), raceId);
  if (!racers.length) return;
  document.querySelector(".cmp-back")?.remove();
  const back = document.createElement("div");
  back.className = "inbox-modal-back cmp-back";
  back.innerHTML = `<div class="inbox-modal cmp" role="dialog" aria-modal="true" aria-labelledby="cmpTitle">
    <header class="im-head"><div><h2 id="cmpTitle">${esc(racers[0].title ?? "Same job")}</h2><p class="im-sub">${racers.length} agents on the same job. Review each one, merge the best, discard the rest.</p></div>
      <button type="button" class="im-x" data-close aria-label="Close">✕</button></header>
    <div class="cmp-grid">${racers.map((t) => {
      const p = paneOf(t);
      return `<section class="cmp-col">
        <div class="cmp-h"><span class="iq-mk" style="--mk:${esc(p?.color ?? "#888")}" aria-hidden="true">${faceSvg(t.name, p?.color ?? "#888")}</span><b>${esc(t.name)}</b><span class="ib-pill st-${t.status.state}">${GROUP_LABEL[t.status.state]}</span></div>
        <p class="cmp-n">${t.changedFiles == null ? "Changes unknown" : `${t.changedFiles} file${t.changedFiles === 1 ? "" : "s"} changed`}${lineCounts(t)}</p>
        <p class="cmp-l">${esc(rowLine(t))}</p>
        <button class="im-btn${t.status.state === "review" ? " primary" : ""}" data-review="${esc(t.paneId)}">Review</button>
      </section>`;
    }).join("")}</div></div>`;
  document.body.appendChild(back);
  const was = document.activeElement as HTMLElement | null;
  const close = () => { back.remove(); if (was?.isConnected) was.focus(); };
  back.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t === back || t.closest("[data-close]")) { close(); return; }
    const id = t.closest<HTMLElement>("[data-review]")?.dataset.review;
    const task = id && allTasks().find((x) => x.paneId === id);
    if (!task) return;
    close();
    if (splitOn) setSplit(false);
    openTask(task);
    openReview();
  });
  back.addEventListener("keydown", (e) => { if (e.key === "Escape") { e.stopPropagation(); close(); } });
  back.querySelector<HTMLElement>("[data-review]")?.focus();
}

/** Everything Ctrl K can reach: every agent (in queue order), then actions. */
export function paletteItems(): PaletteItem[] {
  const agents: PaletteItem[] = allTasks().map((t) => {
    const p = paneOf(t);
    return {
      group: GROUP_LABEL[t.status.state],
      label: t.name,
      sub: `${t.title ? `${t.title} · ` : ""}${t.project} · ${rowLine(t)}`,
      mark: { color: p?.color ?? "#888", letter: (t.name.trim()[0] ?? "?").toUpperCase(), face: t.name },
      run: () => openTask(t),
    };
  });
  const act = (label: string, run: () => void, keys?: string, sub?: string): PaletteItem => ({ group: "Actions", label, run, keys, sub });
  const ws = activeWs;
  const projects: PaletteItem[] = [...workspaces.values()].filter((w) => w !== ws).map((w) => ({
    group: "Projects", label: w.name, sub: `${w.panes.size} agent${w.panes.size === 1 ? "" : "s"}${w.dir ? ` · ${w.dir}` : ""}`,
    run: () => activateWorkspace(w),
  }));
  const here: PaletteItem[] = ws ? [
    { group: "Projects", label: `Rename ${ws.name}`, run: () => void confirmModal({ title: "Rename project", message: "What should this project be called?", okLabel: "Rename", input: { value: ws.name } }).then((r) => { if (r.ok) { renameWorkspace(ws, r.value); render(); } }) },
    { group: "Projects", label: `Close ${ws.name}`, sub: "Stops its agents", run: () => void removeWorkspace(ws) },
  ] : [];
  return [
    ...agents,
    act("New agent", () => openNewAgent(), "Ctrl+Shift+T", "Give 2 or 3 the same job to race them"),
    act(splitOn ? "Back to Focus: one agent, full size" : "Grid: one agent and the others beside it", () => setSplit(!splitOn), "Alt+S", splitOn ? undefined : "The one you work with as a terminal, every other one on a card that answers in place"),
    act("Add the current agent to the Grid, or take it out", () => pinCurrent(), "Alt+P"),
    act("Changes of the current agent", () => openReview(), "Alt+R", "Merge them, or send it back with a note"),
    ...mergeItem(),
    act("History of the current agent", () => openHistory(), "Alt+H", "What it did and what you answered"),
    act("Sessions", () => openSessions(), "Alt+O", "Read back earlier Claude conversations"),
    act("Board", () => openTool("kanban"), "Ctrl+Shift+K", "Cards the agents move as they work"),
    ...(ws ? [act("Project memory", () => openProjectMemory(ws.id), undefined, `What every agent in ${ws.name} knows: read, edit, or ask an agent to fix it`)] : []),
    act("Files", () => document.getElementById("btnToggleCode")?.click(), "Ctrl+Shift+E", "Folder tree and editor"),
    act("Settings", () => openSettings(), "Ctrl+,"),
    act("Pomodoro timer", () => openTool("pomodoro"), "Ctrl+Shift+J"),
    act("Flow", () => openTool("flow"), "Ctrl+Shift+M", "Every hand-off between agents"),
    act("Fleet", () => openTool("fleet"), "Ctrl+Shift+L", "Every agent in every project"),
    act("Scheduled agents", () => openTool("schedule"), undefined, "Start a preset at a set time"),
    act("Replays", () => openTool("replays"), undefined, "Watch a recorded session back"),
    act("Usage", () => openTool("usage"), undefined, "Tokens and estimated cost"),
    act("What Maestro can do", () => openHelpPage(), "?", "Every shortcut and hidden feature"),
    ...projects,
    ...here,
  ];
}

/** A project's memory (memoryview.ts): what its agents share, to edit by hand
 *  or send to one of its running agents to fix. */
function openProjectMemory(wsId: string): void {
  const ws = workspaces.get(wsId);
  if (!ws) return;
  if (!ws.dir) { topNote(`<b>${esc(ws.name)}</b> has no folder, so it has no memory`); return; }
  closePalette();
  openMemory({
    dir: ws.dir,
    project: ws.name,
    agents: () => {
      const cur = stagePane()?.id;
      return allTasks()
        .filter((t) => t.wsId === wsId && paneOf(t)?.running && t.status.state !== "stopped" && t.status.state !== "merged")
        .sort((a, b) => Number(b.paneId === cur) - Number(a.paneId === cur))
        .map((t) => ({ id: t.paneId, name: t.name }));
    },
    send: (id, text) => sendMessage(id, text),
    projects: [...workspaces.values()].filter((w) => !!w.dir).map((w) => ({ id: w.id, name: w.name })),
    current: wsId,
    switchTo: (id) => openProjectMemory(id),
  });
}

/** Ctrl K: merge the agent on the stage, when it is done on a branch of its own. */
function mergeItem(): PaletteItem[] {
  const p = stagePane();
  const t = p && taskOf(p.id);
  if (!p || !t || t.status.state !== "review" || !p.spec.worktree) return [];
  return [{ group: "Actions", label: `Merge ${p.spec.name}'s branch`, keys: "M", sub: "From Changes: its worktree is removed after", run: () => { openReview(); void reviewer?.merge(); } }];
}

/** Ctrl K opens this palette instead of the classic switcher while the new
 *  interface is on. Window capture runs before the switcher's document one. */
function onCtrlK(e: KeyboardEvent): void {
  if (!document.body.classList.contains("inbox-ui")) return;
  // ? or Ctrl+/ shows everything Maestro can do.
  if (isHelpKey(e)) { e.preventDefault(); e.stopImmediatePropagation(); openHelpPage(); return; }
  // Ctrl+, opens Settings, as in most desktop apps.
  if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key === ",") { e.preventDefault(); e.stopImmediatePropagation(); openSettings(); return; }
  if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey || e.key.toLowerCase() !== "k") return;
  e.preventDefault();
  e.stopImmediatePropagation();
  if (paletteOpen()) closePalette(); else openPalette(paletteItems());
}

function onStageButton(e: MouseEvent): void {
  const target = e.target as HTMLElement;
  const paneEl = target.closest<HTMLElement>(".pane");
  const ws = activeWs;
  const pane = ws && paneEl ? screenPanes().find((p) => p.el === paneEl) : undefined;
  const b = target.closest<HTMLElement>("[data-stage]");
  if (!b) return;
  e.stopPropagation();
  switch (b.dataset.stage) {
    // The CLI's own menus, typed into its terminal as you would.
    case "cmds": if (pane) { void sendInput(pane.id, "/"); pane.term.focus(); } break;
    case "model": { const c = pane && profileOf(pane.spec.badge).modelCommand; if (pane && c) { void sendInput(pane.id, `${c}\r`); pane.term.focus(); } break; }
    case "review": if (reviewer?.paneId) reviewer.close(); else openReview(); break;
    case "history": if (historian?.paneId) historian.close(); else openHistory(); break;
    case "compare": { const r = pane && allTasks().find((x) => x.paneId === pane.id)?.race; if (r) openCompare(r.id); break; }
    case "resume": if (pane) void pane.restart?.(); break;
    case "handover": {
      const t = pane && allTasks().find((x) => x.paneId === pane.id);
      if (!pane || !t) break;
      const r = b.getBoundingClientRect();
      openMenu(r.left, r.bottom + 4, handoverItems(pane, t), `Hand over ${t.name}'s task`);
      break;
    }
    case "close": if (pane) closeFromSplit(pane.id); break;
    case "full": {
      const t = pane && allTasks().find((x) => x.paneId === pane.id);
      setSplit(false);
      if (t) openTask(t);
      break;
    }
  }
}

/** The ✕ on an agent's header removes it: ask first, as the menu's Remove does.
 *  The confirmed click is replayed (untrusted), which pane.ts then acts on. */
function onKillClick(e: MouseEvent): void {
  const b = (e.target as HTMLElement).closest<HTMLElement>("[data-kill]");
  if (!b || !e.isTrusted) return;
  const pane = screenPanes().find((p) => p.el.contains(b));
  if (!pane) return;
  e.preventDefault();
  e.stopImmediatePropagation();
  void confirmModal({
    title: `Remove ${pane.spec.name}?`,
    message: pane.running ? "It stops and its terminal closes. Its branch and the changes on it stay in git." : "Its terminal closes. Its branch and the changes on it stay in git.",
    okLabel: "Remove",
    danger: true,
  }).then((r) => { if (r.ok) b.click(); });
}

const lastState = new Map<string, TaskState>();

/** Settings → Inbox / Review: act on an agent's state changing. Bring an agent
 *  that starts waiting on you onto the stage when the one there is not busy,
 *  and open Changes when the agent on the stage finishes with changes. */
function onStateChanges(list: Task[]): void {
  const stage = stagePane();
  const stageTask = stage && list.find((t) => t.paneId === stage.id);
  const typing = (document.activeElement as HTMLElement | null)?.closest("input, textarea, .inbox-modal, .inbox-pal");
  for (const t of list) {
    const before = lastState.get(t.paneId);
    lastState.set(t.paneId, t.status.state);
    if (before === undefined || before === t.status.state) continue;
    if (t.status.state === "needs" && getPref("jumpToNeeds") && !splitOn && !typing && t.paneId !== stage?.id &&
        (!stageTask || !["needs", "working"].includes(stageTask.status.state))) {
      openTask(t);
      return;
    }
    if (t.status.state === "review" && before === "working" && !tipSeen("review")) showTip("review", `<b>${esc(t.name)}</b> finished. `);
    if (t.status.state === "review" && before === "working" && t.paneId === stage?.id &&
        getPref("reviewOnDone") && !reviewer?.paneId && !splitOn) openReview();
  }
}

/** Clicking into another terminal in Split makes it the current one. */
function onFocusIn(e: FocusEvent): void {
  if (!splitOn || !activeWs) return;
  const el = (e.target as HTMLElement | null)?.closest<HTMLElement>(".pane.split-pin");
  if (!el || el.classList.contains("focused")) return;
  const pane = screenPanes().find((p) => p.el === el);
  if (pane) setCurrent(pane);
}

function mount(): void {
  const app = document.getElementById("app");
  if (!app || queueEl) return;
  document.body.classList.add("inbox-ui");
  mountStart();
  queueEl = document.createElement("aside");
  queueEl.className = "inbox-queue";
  queueSig = ""; // a new, empty list: the first render fills it
  queueEl.setAttribute("aria-label", "Agents");
  app.appendChild(queueEl);
  askEl = document.createElement("section");
  askEl.className = "inbox-ask";
  askEl.hidden = true;
  askEl.setAttribute("aria-live", "polite");
  app.appendChild(askEl);
  emptyEl = document.createElement("section");
  emptyEl.className = "ib-empty";
  emptyEl.hidden = true;
  emptyEl.setAttribute("aria-label", "No agents yet");
  emptyEl.addEventListener("click", (e) => { if ((e.target as HTMLElement).closest("[data-empty-new]")) openNewAgent(); });
  app.appendChild(emptyEl);
  headEl = document.createElement("p");
  headEl.className = "inbox-head";
  headEl.setAttribute("aria-live", "polite");
  document.querySelector(".topbar .tb-center")?.prepend(headEl);
  // The floating dock at the bottom: views, the command bar, New agent, tools.
  dockEl = document.createElement("nav");
  dockEl.className = "inbox-dock";
  dockEl.setAttribute("aria-label", "Maestro");
  dockEl.innerHTML = `
    <div class="id-seg">
      <button data-dock="queue" aria-pressed="true" title="Focus: one agent at a time, full size (Alt+S switches)"><svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2.5" y="2.5" width="11" height="11" rx="2.5"/><path d="M5.5 6.5h5M5.5 9.5h3"/></svg><span>Focus</span></button>
      <button data-dock="split" aria-pressed="false" title="Grid: the agent you work with as a terminal, the others as cards beside it (Alt+S switches, Alt+P adds one)"><svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2" y="2" width="5" height="5" rx="1.4"/><rect x="9" y="2" width="5" height="5" rx="1.4"/><rect x="2" y="9" width="5" height="5" rx="1.4"/><rect x="9" y="9" width="5" height="5" rx="1.4"/></svg><span>Grid</span></button>
    </div>
    <button class="id-search" data-dock="search" title="Jump to an agent or run a command (Ctrl+K)">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
      <span>Jump to an agent or run a command</span><kbd>Ctrl+K</kbd></button>
    <button class="id-new" data-dock="new" title="New agent (Ctrl+Shift+T)">New agent</button>
    <button class="id-gear" data-dock="sessions" aria-label="Sessions" title="Sessions: read back earlier conversations (Alt+O)">
      <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l3 2"/></svg></button>
    <button class="id-gear id-help" data-dock="help" aria-label="What Maestro can do" title="What Maestro can do (?)">?</button>
    <button class="id-gear" data-dock="settings" aria-label="Settings" title="Settings (Ctrl+,)">
      <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg></button>`;
  app.appendChild(dockEl);
  splitBtn = dockEl.querySelector<HTMLButtonElement>('[data-dock="split"]');
  dockEl.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("[data-dock]");
    switch (b?.dataset.dock) {
      case "queue": setSplit(false); break;
      case "split": setSplit(!splitOn); break;
      case "search": openPalette(paletteItems()); break;
      case "new": openNewAgent(); break;
      case "settings": openSettings(); break;
      case "help": openHelpPage(); break;
      case "sessions": openSessions(); break;
    }
  });
  document.getElementById("workspaces")?.addEventListener("click", onStageButton);
  document.getElementById("workspaces")?.addEventListener("click", onKillClick, true);
  historian = createHistoryDrawer(app, render);
  reviewer = createReviewDrawer(app, render);

  // Right-click an agent in the queue for everything you can do with it.
  queueEl.addEventListener("contextmenu", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>(".iq-row");
    const t = row && allTasks().find((x) => x.paneId === row.dataset.id);
    if (!t) return;
    e.preventDefault();
    openMenu(e.clientX, e.clientY, agentMenu(t), `${t.name} actions`);
  });
  // Shift+F10 or the menu key opens the same menu on the focused row.
  queueEl.addEventListener("keydown", (e) => {
    if (!(e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey))) return;
    const row = (e.target as HTMLElement).closest<HTMLElement>(".iq-row");
    const t = row && allTasks().find((x) => x.paneId === row.dataset.id);
    if (!t || !row) return;
    e.preventDefault();
    const r = row.getBoundingClientRect();
    openMenu(r.left + 24, r.bottom - 4, agentMenu(t), `${t.name} actions`);
  });
  queueEl.addEventListener("click", (e) => {
    const mem = (e.target as HTMLElement).closest<HTMLElement>("[data-mem]");
    if (mem) { openProjectMemory(mem.dataset.mem ?? ""); return; }
    // A project's name folds its stopped agents away, or shows them again (remembered).
    const head = (e.target as HTMLElement).closest<HTMLElement>("[data-proj]");
    if (head) {
      const id = head.dataset.proj ?? "";
      if (foldedProjects.has(id)) foldedProjects.delete(id); else foldedProjects.add(id);
      saveFolded();
      render();
      queueEl?.querySelector<HTMLElement>(`[data-proj="${CSS.escape(id)}"]`)?.focus({ preventScroll: true });
      return;
    }
    if ((e.target as HTMLElement).closest("[data-clear-merged]")) { clearMerged(); return; }
    const row = (e.target as HTMLElement).closest<HTMLElement>(".iq-row");
    const t = row && allTasks().find((x) => x.paneId === row.dataset.id);
    if (!t || !row) return;
    // Resume on a stopped row starts it again, in place.
    if ((e.target as HTMLElement).closest("[data-resume]")) { void paneOf(t)?.restart?.(); return; }
    // The ⋯ on a row opens the same menu as a right-click.
    if ((e.target as HTMLElement).closest("[data-more]")) {
      const r = (e.target as HTMLElement).closest<HTMLElement>("[data-more]")!.getBoundingClientRect();
      openMenu(r.left, r.bottom + 4, agentMenu(t), `${t.name} actions`);
      return;
    }
    openTask(t);
  });
  askEl.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!b || !askEl) return;
    if (b.dataset.act === "review") openReview();
  });
  document.addEventListener("keydown", onKey, true);
  document.addEventListener("focusin", onFocusIn);
  window.addEventListener("keydown", onCtrlK, true);
  window.addEventListener("resize", render);
  // Remember where everyone stands now, so only later changes count.
  for (const t of allTasks()) lastState.set(t.paneId, t.status.state);
  offTasks = onTasksChange(() => { onStateChanges(allTasks()); render(); });
  onAgentCost(render);
  startPlanMeter(render);
  startLimitGuard();
  // Resume carries on the conversation each agent was last in.
  startSessionFollow();
  // Agents that finish say so; a click on that notification opens the agent.
  startFinishWatch({
    screen: () => stagePane()?.id ?? null,
    open: (id) => { const t = taskOf(id); if (t) openTask(t); },
  });
  // Hidden, the inbox redraws every 5 s (it also trims hidden terminals'
  // history); what changes an agent's state still redraws it at once.
  const gate = hiddenGate(5);
  timer = window.setInterval(() => { if (gate()) render(); }, 1000);
  window.setTimeout(() => showTip("welcome"), 2600);
  render();
}

function unmount(): void {
  document.body.classList.remove("inbox-ui");
  unmountStart();
  stopPlanMeter();
  stopLimitGuard();
  stopFinishWatch();
  stopSessionFollow();
  if (splitWs) clearSplit(splitWs);
  splitOn = false; pins = []; splitSig = ""; splitWs = null;
  if (reviewer?.paneId) reviewer.close();
  for (const ws of workspaces.values()) for (const p of ws.panes.values()) { p.el.querySelector(".ib-pill")?.remove(); p.el.querySelector(".ib-acts")?.remove(); p.el.querySelector(".ib-ttl")?.remove(); p.el.classList.remove("ib-needs", "ib-has-title"); }
  queueEl?.remove(); askEl?.remove(); headEl?.remove(); dockEl?.remove(); emptyEl?.remove(); emptyEl = null;
  queueEl = askEl = headEl = dockEl = null; splitBtn = null; reviewer = null; historian = null;
  if (timer !== null) { clearInterval(timer); timer = null; }
  offTasks?.(); offTasks = null;
  document.removeEventListener("keydown", onKey, true);
  document.removeEventListener("focusin", onFocusIn);
  document.getElementById("workspaces")?.removeEventListener("click", onStageButton);
  document.getElementById("workspaces")?.removeEventListener("click", onKillClick, true);
  window.removeEventListener("keydown", onCtrlK, true);
  closePalette();
  window.removeEventListener("resize", render);
  lastState.clear();
}

/** Mount or take down the interface. The app mounts it once at startup
 *  (initInbox); tests use this to start each case clean. */
export function setInbox(on: boolean): void {
  if (on) mount(); else unmount();
}

/** Mount the interface. Call once at startup. */
export function initInbox(): void {
  mount();
}
