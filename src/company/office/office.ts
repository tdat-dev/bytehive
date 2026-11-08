// src/company/office/office.ts
/* The office in the app: when it shows, what it replaces, and what happens without WebGL. */
import { dockIsOpen } from "../../dock";
import { topNote } from "../../hint";
import { changesOpenFor, closeChanges, focusStage, openChanges, refitStage, stageAgent } from "../../inbox";
import { companyElsewhere, companyUsedNow, onCompanyChange } from "../engine";
import type { OfficeModel, OfficePerson } from "./model";
import { BUILDING, deskOf, homeView, type DeskTab, type Level } from "./nav";
import { createJobBox, createRail, fellHtml, why } from "./rail";
import { createShell, type ShellHost } from "./shell";
import type { OfficeSource } from "./source";

/** Turn the 3D scene off for everyone. Task 10's gate is what can set this false. */
export const OFFICE_3D: boolean = true;

const USED_KEY = "maestro.office.used";
/** How long a lost context gets before the Inbox takes over. */
const RESTORE_MS = 2000;
const FELL_BACK = "The office can't draw on this graphics card. Everything still works here.";
/** The rail lives in the Inbox, and with no project open Start shows instead. */
const FELL_BACK_NO_PROJECT = "The office can't draw on this graphics card. Open a project to see the Company.";
const NOT_HERE = "The Company runs in the main window.";

export interface OfficeDeps {
  showApp(): void;
  showHome(): void;
  /** Open the Board on a project's folder. False when that project is not open. */
  openBoard?(dir: string): boolean;
  /** Ask for a folder and open it: Start is out of reach once the office is home. */
  openProject?(): void;
  /** The bar's doors to the tools a key can't reach from a focused terminal. */
  openPalette?(): void;
  openBoardTool?(): void;
  newAgent?(): void;
}

let deps: OfficeDeps | null = null;
let shell: ReturnType<typeof createShell> | null = null;
let source: OfficeSource | null = null;
let side: { el: HTMLElement; dispose(): void } | null = null;
let visible = false;
let fallback = false;
/** The note goes up once a run, however often the scene fails. */
let noted = false;
/** Boot opened what is on screen, not the owner: another app holding the Company takes it down. */
let byBoot = false;
let booted = false;
let seenUsed = false;
/** Goes up with each open and close: an open still loading is dropped when it is not the last. */
let turn = 0;
/** The pane and tab a Desk has put on the stage, so asking again moves nothing. */
let seated = "";
let lostTimer: number | null = null;

function safeGet(key: string): string | null {
  try { return localStorage.getItem(key); } catch { return null; }
}
function safeSet(key: string, value: string): void {
  try { localStorage.setItem(key, value); } catch { /* the engine still knows, for this run */ }
}

const used = () => companyUsedNow() || safeGet(USED_KEY) === "1";

const sourceNow = async (): Promise<OfficeSource> =>
  (import.meta.env.DEV && import.meta.env.VITE_OFFICE_DEMO ? (await import("./fixtures")).demoSource() : (await import("./live")).liveSource());

const everyone = (m: OfficeModel | null | undefined): OfficePerson[] => (m ? [...(m.founder ? [m.founder] : []), ...m.floors.flatMap((f) => f.people)] : []);
const modelNow = () => source?.snapshot(Date.now()).model;

/** What sits under the layer is out of reach. Not the dock's panel: it stands over the office. */
function cover(on: boolean): void {
  document.body.classList.toggle("office-cover", on);
  document.getElementById("home")?.toggleAttribute("inert", on);
  for (const c of document.getElementById("app")?.children ?? []) c.toggleAttribute("inert", on && !c.classList.contains("dock-panel"));
}

/** Whether the keyboard is the office's, or nobody's: leaving, it would go nowhere. */
function keyboardHere(): boolean {
  const at = document.activeElement;
  return !at || at === document.body || !!at.closest("#office, .of-tabs");
}

/** Take the layer down. The app comes back first: focus cannot land on what is still covered. */
function uncover(): void {
  const held = keyboardHere();
  turn += 1;
  visible = false;
  seated = "";
  shell?.hide();
  cover(false);
  deps?.showHome();
  if (held) focusStage();
}

function clearLost(): void {
  if (lostTimer !== null) window.clearTimeout(lostTimer);
  lostTimer = null;
}

/** Safe to ask again: the same pane and tab move nothing, so the keyboard stays where it is.
 *  A terminal takes the keyboard only when it can be seen, and only if the owner came for it. */
function showDesk(personId: string, tab: DeskTab, focus: boolean): boolean {
  const paneId = everyone(modelNow()).find((p) => p.id === personId)?.paneId;
  if (!paneId) {
    seated = "";
    return false;
  }
  const spot = `${paneId}:${tab}`;
  if (spot === seated) return true;
  const arriving = !seated.startsWith(`${paneId}:`);
  seated = spot;
  deps?.showApp();
  const take = focus && tab === "terminal";
  // A new agent has no task yet, so staging fails: reveal it. If nothing could, say so and keep
  // nothing, or the Desk would hold another agent's stage and never ask again.
  if ((arriving || tab === "terminal") && !stageAgent(paneId, take) && !source?.showPerson(personId, take)) {
    seated = "";
    return false;
  }
  if (tab === "changes") openChanges(paneId);
  return true;
}

function openBoard(floorId: string): void {
  const f = modelNow()?.floors.find((x) => x.id === floorId);
  if (f && !deps?.openBoard?.(f.dir)) topNote(`${f.name} is closed. Open it to see its board.`);
}

function failed(reason: string): void {
  console.error(`[bytehive] the office could not start: ${reason}`);
  clearLost();
  // Nobody is looking: the next open makes a new scene.
  if (!visible) return;
  uncover();
  void toFallback();
}

const host: ShellHost = {
  showDesk,
  levelChanged(l) {
    cover(l.kind !== "desk");
    if (l.kind !== "desk") seated = "";
    // On every change: it stands over the rail, and only its own Esc or a tab would close it.
    if (l.kind !== "desk" || l.tab !== "changes") closeChanges();
  },
  refit: () => refitStage(),
  leave: () => close(),
  failed,
  loadScene: () => import("./scene"),
  changesOpenFor: () => changesOpenFor(),
  openBoard,
  contextLost() {
    lostTimer ??= window.setTimeout(() => {
      lostTimer = null;
      shell?.sceneFailed("the 3D context was lost");
    }, RESTORE_MS);
  },
  contextRestored: clearLost,
};

/** A row in the fallback: there is no Desk, so it goes straight to the agent. */
function goFromSide(l: Level): void {
  if (l.kind !== "desk") return;
  const p = everyone(modelNow()).find((x) => x.id === l.personId);
  if (!p?.paneId) {
    topNote(`${p?.name ?? "That person"} is not running.`);
    return;
  }
  deps?.showApp();
  if (l.tab === "changes") openChanges(p.paneId);
  else stageAgent(p.paneId);
}

function leaveFallback(): void {
  fallback = false;
  side?.dispose();
  side = null;
  document.body.classList.remove("office-fallback");
}

/** The Inbox, with the rail and the job box as a fourth column. Only open comes here, so only
 *  someone who used the Company or pressed the key: without 3D, this is where the job box is. */
async function toFallback(): Promise<void> {
  if (fallback) return;
  fallback = true;
  const el = document.createElement("aside");
  const made: Array<{ dispose(): void }> = [];
  try {
    source ??= await sourceNow();
    const app = document.getElementById("app");
    if (!app || !fallback) return;
    el.className = "of-side";
    el.setAttribute("aria-label", "The Company");
    el.innerHTML = `${fellHtml(OFFICE_3D)}<div data-job-box></div><div data-rail></div>`;
    el.querySelector("[data-retry]")?.addEventListener("click", () => void open());
    app.append(el);
    const box = createJobBox(source);
    const rail = createRail(source, goFromSide);
    made.push(box, rail);
    box.mount(el.querySelector<HTMLElement>("[data-job-box]")!);
    rail.mount(el.querySelector<HTMLElement>("[data-rail]")!);
    side = { el, dispose() { box.dispose(); rail.dispose(); el.remove(); } };
    document.body.classList.add("office-fallback");
    if (!noted) topNote(app.hidden ? FELL_BACK_NO_PROJECT : FELL_BACK);
    noted = true;
  } catch (e) {
    // Nothing worse than the Inbox as it always was.
    for (const part of made) part.dispose();
    el.remove();
    fallback = false;
    console.error(`[bytehive] the office could not start: ${why(e)}`);
  }
}

/** The layer goes up at once, so the Inbox never shows while the scene loads. The shell fetches
 *  three.js through loadScene, which is the first time it is fetched, and reports back to failed. */
async function open(to?: Level): Promise<void> {
  if (!OFFICE_3D) {
    await toFallback();
    return;
  }
  const mine = ++turn;
  visible = true;
  try {
    source ??= await sourceNow();
    if (mine !== turn) return;
    if (fallback) leaveFallback();
    if (!shell) {
      shell = createShell(source, host);
      document.body.append(shell.el);
    }
    shell.show(to);
  } catch (e) {
    failed(why(e));
  }
}

function close(): void {
  byBoot = false;
  uncover();
}

function onEngine(): void {
  if (!seenUsed && companyUsedNow()) {
    seenUsed = true;
    safeSet(USED_KEY, "1");
    // Behind the splash there is nothing to yank. After it, this waits for the next house icon or start.
    if (booted && !visible && !fallback && document.body.classList.contains("boot")) officeBoot();
  }
  if (!byBoot || !companyElsewhere()) return;
  // Boot could not know: the lock had not been read yet. The Inbox is where this app belongs.
  if (visible) close();
  else if (fallback) leaveFallback();
  byBoot = false;
}

function onKey(e: KeyboardEvent): void {
  if (e.key !== "Escape" || !visible || !shell) return;
  const at = document.activeElement as HTMLElement | null;
  const went = shell.escape({
    terminal: !!at?.closest(".xterm"),
    dialog: !!document.querySelector(".backdrop.open, .inbox-modal-back, .inbox-pal, .inbox-review, .cm-menu, .sm-pop"),
    dock: dockIsOpen(),
    typing: !!at?.closest("input, textarea, select, .cm-editor"),
  });
  if (went) {
    e.preventDefault();
    e.stopPropagation();
  }
}

export function initOffice(d: OfficeDeps): void {
  deps = d;
  host.openProject = d.openProject;
  host.openPalette = d.openPalette;
  host.openBoardTool = d.openBoardTool;
  host.newAgent = d.newAgent;
  onCompanyChange(onEngine);
  document.addEventListener("keydown", onKey, true);
}

export function officeBoot(): void {
  booted = true;
  if (!used() || homeView({ used: true, webgl: true, elsewhere: !!companyElsewhere() }) !== "office") return;
  byBoot = true;
  void open();
}

export function officeToggle(): void {
  if (!deps) {
    topNote(NOT_HERE);
    return;
  }
  if (visible) {
    close();
    return;
  }
  // Someone who never used the Company got the column by asking: the key puts it away again.
  if (fallback && !used()) {
    leaveFallback();
    return;
  }
  if (fallback && !OFFICE_3D) return;
  byBoot = false;
  void open();
}

export function officeGoHome(): boolean {
  if (!deps) return false;
  if (homeView({ used: used(), webgl: OFFICE_3D && !fallback, elsewhere: !!companyElsewhere() }) !== "office") return false;
  if (visible) shell?.go(BUILDING);
  else void open(BUILDING);
  return true;
}

export const officeVisible = (): boolean => visible;

export function officeReveal(paneId: string): void {
  if (!visible || !shell) return;
  const p = everyone(modelNow()).find((x) => x.paneId === paneId);
  if (p) shell.go(deskOf(p));
  else close();
}
