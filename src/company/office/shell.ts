/* The office around the scene: where you are, what you can press, and what needs you.
 * Three levels on one axis. Nothing moves between hosts: at a Desk the layer narrows to a
 * column and the app's real stage is simply no longer covered. */
import { CHIP_BG, OFFICE, PANEL_BG, TAG_INK } from "./palette";
import type { OfficeFloor, OfficeModel, OfficePerson } from "./model";
import { BUILDING, clampLevel, crumbs, deskOf, escapeGoesUp, navUp, type Crumb, type DeskLevel, type DeskTab, type Level } from "./nav";
import { createJobBox, createRail, why } from "./rail";
import type { DeskReport, ReportCard } from "./report";
import type { Anchor, OfficeScene, SceneOptions } from "./scene";
import type { OfficeSource } from "./source";

export interface ShellHost {
  /** Put a person's real pane on the stage and open a tab there. `focus` says the owner came
   *  here: false when the Desk changed by itself, and the keyboard must stay where it is. */
  showDesk(personId: string, tab: DeskTab, focus: boolean): boolean;
  /** Fit the terminal to the stage again. Takes no focus: this runs on any resize at a Desk. */
  refit?(): void;
  /** Called when the level changes: the app shows or hides what sits under the layer. */
  levelChanged(l: Level): void;
  /** Leave the office for the Inbox: the owner asked to. */
  leave(): void;
  /** The scene cannot run: no context, a failed import, or a context that stayed lost. */
  failed(why: string): void;
  loadScene(): Promise<typeof import("./scene")>;
  /** Which pane's Changes drawer is open: the Changes tab follows it. */
  changesOpenFor?(): string | null;
  /** Ask for a folder and open it as a project. Without it the bar does not offer that. */
  openProject?(): void;
  /** Open the Board tool on a floor's project. Without it the wall board's counts are plain text. */
  openBoard?(floorId: string): void;
  /** The context went or came back, for whoever decides how long to wait. */
  contextLost?(): void;
  contextRestored?(): void;
}

const TICK_MS = 1000;
const RESIZE_MS = 120;
const HOVER_MS = 80;
/** Space between a tag and what it marks, and between a tag and the stage's edge. */
const TAG_GAP = 8;
const HEAD_GAP = 3;
const TAG_EDGE = 6;
const TAG_TOUCH = 2;
/** A bubble is a little wider than it is tall. */
const BUBBLE_WIDE = 1.2;
const TABS: Array<{ id: DeskTab; label: string }> = [{ id: "terminal", label: "Terminal" }, { id: "changes", label: "Changes" }, { id: "report", label: "Report" }];
const MOOD: Partial<Record<OfficePerson["state"], string>> = { needs: "needs you", review: "to review", idle: "idle", stopped: "stopped" };

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const sameLevel = (a: Level, b: Level) => JSON.stringify(a) === JSON.stringify(b);
const people = (n: number) => (n === 1 ? "1 person" : `${n} people`);
const needYou = (n: number) => (n ? ` <em>${n === 1 ? "1 needs you" : `${n} need you`}</em>` : "");
const contactOf = (f: OfficeFloor): OfficePerson | null => f.people.find((p) => p.id === f.lead) ?? f.people[0] ?? null;
const personIn = (m: OfficeModel | null, id: string): OfficePerson | undefined =>
  (m?.founder?.id === id ? m.founder : m?.floors.flatMap((f) => f.people).find((p) => p.id === id)) ?? undefined;

export interface Box { x: number; y: number; w: number; h: number }
const hits = (a: Box, b: Box) => a.x < b.x + b.w + TAG_TOUCH && b.x < a.x + a.w + TAG_TOUCH && a.y < b.y + b.h + TAG_TOUCH && b.y < a.y + a.h + TAG_TOUCH;

/** Tags apart, in place. Off a bubble sideways: going up would land on its owner's tag. Then,
 *  nearest the viewer first, each keeps its place and the one behind steps up past it. */
export function settleTags(tags: Box[], bubbles: Box[], edge = 0): void {
  for (const r of tags) {
    for (const o of bubbles) {
      if (hits(r, o)) r.x = Math.max(edge, r.x + r.w / 2 < o.x + o.w / 2 ? o.x - r.w - TAG_TOUCH : o.x + o.w + TAG_TOUCH);
    }
  }
  const done: Box[] = [];
  for (const r of [...tags].sort((a, b) => b.y - a.y)) {
    for (let i = 0; i < done.length; i += 1) {
      if (!hits(r, done[i])) continue;
      r.y = done[i].y - r.h - TAG_TOUCH;
      i = -1;
    }
    done.push(r);
  }
}

/** What a tag says, and what pressing it does. */
interface Tag { key: string; id: string; kind: Anchor["kind"]; html: string; label: string; needs: boolean; to?: Level; floor?: string }

function floorTag(f: OfficeFloor): Tag {
  const closed = f.open ? "" : " <span>closed</span>";
  return { key: `floor:${f.id}`, id: f.id, kind: "floor", html: `<b>${esc(f.name)}</b> <span>${people(f.people.length)}</span>${needYou(f.needs)}${closed}`, label: f.name, needs: f.needs > 0, to: { kind: "floor", floorId: f.id } };
}

function personTag(p: OfficePerson): Tag {
  const who = p.rank === "lead" ? "Lead" : p.title;
  const mood = MOOD[p.state];
  const detail = mood ? `${who}, ${mood}` : who;
  return { key: `person:${p.id}`, id: p.id, kind: "person", html: `${esc(p.name)} <span>${esc(detail)}</span>`, label: `${p.name}, ${detail}`, needs: p.state === "needs", to: deskOf(p) };
}

function wantedTags(l: Level, m: OfficeModel, canOpenBoard: boolean): Tag[] {
  if (l.kind === "building") {
    // A plan or a paused Job is nobody's on a floor: without this the tags would not add up to the rail.
    const here = new Set(m.floors.flatMap((f) => f.people.map((p) => p.id)));
    const loose = m.needs.filter((r) => !r.personId || !here.has(r.personId)).length;
    const top: Tag[] = m.founder ? [{ key: "top:top", id: "top", kind: "top", html: `Founder${needYou(loose)}`, label: "", needs: loose > 0, to: deskOf(m.founder) }] : [];
    return [...top, ...m.floors.map(floorTag)];
  }
  if (l.kind === "desk") return [];
  const f = m.floors.find((x) => x.id === l.floorId);
  if (!f) return [];
  const tags = f.people.map(personTag);
  if (f.board) {
    const b = f.board;
    tags.push({ key: `board:${f.id}`, id: f.id, kind: "board", html: `To do ${b.todo}, Doing ${b.doing}, Done ${b.done}`, label: canOpenBoard ? `Open the board. To do ${b.todo}, Doing ${b.doing}, Done ${b.done}` : "", needs: false, ...(canOpenBoard ? { floor: f.id } : {}) });
  }
  return tags;
}

const cardHtml = (c: ReportCard) => {
  const where = [c.job, c.jobTitle, c.list, c.stopped ? "Stopped" : ""].filter(Boolean).map((x) => esc(x!)).join(", ");
  const ticks = c.checklist.map((i) => `<li data-done="${!!i.done}"><span aria-hidden="true">${i.done ? "&#10003;" : "&#9675;"}</span>${esc(i.text)}<span class="of-sr">${i.done ? ", done" : ", not done"}</span></li>`).join("");
  return `<section class="of-card"><h3>${esc(c.title)}</h3><p class="of-meta">${where}</p>` +
    `${c.summary ? `<p>${esc(c.summary)}</p>` : ""}${ticks ? `<ul class="of-ticks">${ticks}</ul>` : ""}` +
    `${c.files.length ? `<ul class="of-files">${c.files.map((f) => `<li>${esc(f)}</li>`).join("")}</ul>` : ""}</section>`;
};

function reportHtml(r: DeskReport | null, fallbackName: string): string {
  const name = r?.name ?? fallbackName;
  const branch = r?.branch ? `<p class="of-meta">Branch <code>${esc(r.branch)}</code></p>` : "";
  const body = r?.cards.length ? r.cards.map(cardHtml).join("") : `<p class="of-empty">Nothing to report yet.</p>`;
  return `<h2>${esc(name)}</h2>${branch}${body}`;
}

const LAYOUT = `
  <header class="of-bar" data-tauri-drag-region>
    <nav class="of-crumbs" aria-label="Where you are" data-crumbs></nav>
    <div class="of-acts">
      <button type="button" class="of-inbox" data-open-project hidden>Open project</button>
      <button type="button" class="of-inbox" data-leave>Inbox</button>
    </div>
  </header>
  <div class="of-stage" data-stage>
    <canvas class="of-canvas" tabindex="-1" aria-hidden="true"></canvas>
    <div class="of-tags" data-tags></div>
    <p class="of-note" data-note hidden></p>
  </div>
  <div class="of-job" data-job-box></div>
  <form class="of-msg" data-msg-box hidden>
    <label for="ofMsg"></label><textarea id="ofMsg" rows="2"></textarea>
    <div class="of-msg-foot"><p class="of-sent" role="status" data-sent></p><button type="submit" class="co-send" data-send>Send</button></div>
    <p class="co-error" role="alert" data-msg-error hidden></p>
  </form>
  <aside class="of-rail" aria-label="Needs you" data-rail></aside>
  <section class="of-cover of-report" id="ofReport" role="tabpanel" aria-labelledby="ofTabReport" tabindex="0" data-report hidden></section>
  <section class="of-cover of-wake-cover" data-wake-card hidden>
    <div class="of-wake"><p data-wake-line></p><button type="button" class="co-send" data-wake></button><p class="co-error" role="alert" data-wake-error hidden></p></div>
  </section>`;

export function createShell(source: OfficeSource, host: ShellHost) {
  const el = document.createElement("div");
  el.id = "office";
  el.className = "of";
  el.hidden = true;
  el.innerHTML = LAYOUT;
  // The scene's own colours, so the chrome around it cannot drift from it.
  for (const [name, value] of Object.entries({ sky: OFFICE.sky, honey: OFFICE.honey, chip: CHIP_BG, panel: PANEL_BG, ink: TAG_INK })) el.style.setProperty(`--of-${name}`, value);

  const find = <T extends HTMLElement>(sel: string) => el.querySelector<T>(sel)!;
  const stage = find("[data-stage]");
  const crumbsEl = find("[data-crumbs]");
  const openEl = find("[data-open-project]");
  const tagsEl = find("[data-tags]");
  const noteEl = find("[data-stage] [data-note]");
  const jobEl = find("[data-job-box]");
  const msgEl = find<HTMLFormElement>("[data-msg-box]");
  const msgBox = find<HTMLTextAreaElement>("#ofMsg");
  const msgLabel = find("[data-msg-box] label");
  const msgSend = find("[data-msg-box] [data-send]");
  const msgSent = find("[data-sent]");
  const msgError = find("[data-msg-error]");
  const reportEl = find("[data-report]");
  const wakeCard = find("[data-wake-card]");
  const wakeLine = find("[data-wake-line]");
  const wakeBtn = find("[data-wake]");
  const wakeError = find("[data-wake-error]");
  let canvas = find<HTMLCanvasElement>("canvas");

  let level: Level = BUILDING;
  let model: OfficeModel | null = null;
  let scene: OfficeScene | null = null;
  let offMoved: (() => void) | null = null;
  let shown = false;
  let loading = false;
  let disposed = false;
  /** The camera is on its way: tags wait, and the pointer does not ask what is under it. */
  let moving = false;
  let timer: number | null = null;
  let fitTimer: ReturnType<typeof setTimeout> | null = null;
  let lastHover = -Infinity;
  let trail: Crumb[] = [];
  let trailHtml = "";

  /** Who this message box is for. A draft stays with its floor, or it could reach the wrong Lead. */
  let msgFloor: string | null = null;
  const drafts = new Map<string, string>();
  let sending = false;

  let noPane = false;
  let waking = false;
  let wakeFailure = "";
  let reportAsk = 0;

  const placed = new Map<string, { el: HTMLElement; tag: Tag }>();

  const rail = createRail(source, (l) => go(l), () => level);
  const jobBox = createJobBox(source);
  rail.mount(find("[data-rail]"));
  jobBox.mount(jobEl);

  const tablist = document.createElement("div");
  tablist.className = "of-tabs";
  tablist.setAttribute("role", "tablist");
  tablist.setAttribute("aria-label", "Desk");
  tablist.innerHTML = TABS.map((t) => `<button type="button" role="tab" class="of-tab" data-tab="${t.id}" id="ofTab${t.label}">${t.label}</button>`).join("");
  const tabButtons = [...tablist.querySelectorAll<HTMLElement>("[role=tab]")];

  const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const sceneOptions: SceneOptions = {
    reducedMotion: motion.matches,
    dpr: window.devicePixelRatio,
    onLost: () => host.contextLost?.(),
    onRestored: () => host.contextRestored?.(),
  };

  /* ---- what is on screen ---- */

  function drawCrumbs(m: OfficeModel): void {
    trail = crumbs(level, m);
    const html = `<ol>${trail.map((c, i) => `<li>${c.current
      ? `<span data-crumb="${i}" aria-current="page" tabindex="-1" title="${esc(c.label)}">${esc(c.label)}</span>`
      : `<button type="button" data-crumb="${i}" title="${esc(c.label)}">${esc(c.label)}</button>`}</li>`).join("")}</ol>`;
    if (html === trailHtml) return;
    trailHtml = html;
    crumbsEl.innerHTML = html;
  }

  function showNote(text: string | null): void {
    noteEl.textContent = text ?? "";
    noteEl.hidden = !text;
  }

  function drawMessageBox(f: OfficeFloor | undefined): void {
    const to = f ? contactOf(f) : null;
    msgEl.hidden = !to;
    const id = to ? f!.id : null;
    if (id !== msgFloor) {
      if (msgFloor) drafts.set(msgFloor, msgBox.value);
      msgBox.value = (id && drafts.get(id)) ?? "";
      msgFloor = id;
      msgSent.textContent = "";
      msgError.hidden = true;
    }
    if (to) msgLabel.textContent = `Message ${to.name}`;
    msgSend.setAttribute("aria-disabled", String(sending));
    msgSend.textContent = sending ? "Working..." : "Send";
  }

  function drawBoxes(m: OfficeModel): void {
    const at = level;
    const f = at.kind === "floor" ? m.floors.find((x) => x.id === at.floorId) : undefined;
    jobEl.hidden = level.kind !== "building";
    drawMessageBox(f);
    showNote(f && !f.people.length ? "Nobody here yet." : null);
  }

  /** A tag's button, kept between redraws: replacing it would lose the keyboard. */
  function tagEl(t: Tag, canPress: boolean): HTMLElement {
    const had = placed.get(t.key);
    if (had) return had.el;
    const b = document.createElement(canPress ? "button" : "span");
    if (b instanceof HTMLButtonElement) b.type = "button";
    b.className = `of-tag of-tag-${t.kind}`;
    b.dataset.key = t.key;
    if (t.kind === "board") b.dataset.board = t.id;
    else b.dataset.tag = t.id;
    return b;
  }

  function place(): void {
    tagsEl.classList.toggle("is-moving", moving);
    if (!scene || moving) return;
    const m = model;
    const tags = m ? wantedTags(level, m, !!host.openBoard) : [];
    const spots = new Map(scene.anchors().map((a) => [`${a.kind}:${a.id}`, a]));
    const live = tags.filter((t) => spots.has(t.key)).sort((a, b) => spots.get(a.key)!.y - spots.get(b.key)!.y);

    const keep = new Set(live.map((t) => t.key));
    for (const [key, had] of placed) {
      if (keep.has(key)) continue;
      had.el.remove();
      placed.delete(key);
    }
    const order = live.map((t) => tagEl(t, !!t.to || !!t.floor));
    if (order.some((b, i) => tagsEl.children[i] !== b) || tagsEl.children.length !== order.length) {
      const held = document.activeElement;
      tagsEl.replaceChildren(...order);
      if (held instanceof HTMLElement && held !== document.activeElement && tagsEl.contains(held)) held.focus({ preventScroll: true });
    }

    const w = stage.clientWidth;
    const h = stage.clientHeight;
    const boxes = live.map((t, i) => {
      const b = order[i];
      const a = spots.get(t.key)!;
      if (placed.get(t.key)?.tag.html !== t.html) b.innerHTML = t.html;
      placed.set(t.key, { el: b, tag: t });
      b.classList.toggle("is-needs", t.needs);
      if (t.label) b.title = t.label;
      if (t.label && t.kind !== "floor") b.setAttribute("aria-label", t.label);
      b.hidden = !a.visible;
      const bw = b.offsetWidth;
      const bh = b.offsetHeight;
      const x = t.kind === "person" ? a.x - bw / 2 : a.x + TAG_GAP;
      const y = t.kind === "person" ? a.y - bh - HEAD_GAP : a.y - bh / 2;
      return { x: Math.max(TAG_EDGE, Math.min(x, w - bw - TAG_EDGE)), y: Math.max(TAG_EDGE, Math.min(y, h - bh - TAG_EDGE)), w: bw, h: bh };
    });
    const bubbles = live.flatMap((t): Box[] => {
      const a = spots.get(t.key)!;
      return a.clear && a.visible ? [{ x: a.x - (a.clear * BUBBLE_WIDE) / 2, y: a.y, w: a.clear * BUBBLE_WIDE, h: a.clear }] : [];
    });
    settleTags(boxes.filter((_, i) => !order[i].hidden), bubbles, TAG_EDGE);
    boxes.forEach((r, i) => { order[i].style.transform = `translate(${Math.round(r.x)}px, ${Math.round(r.y)}px)`; });
  }

  function drawTabs(): void {
    if (level.kind !== "desk" || !shown) {
      tablist.remove();
      return;
    }
    if (!tablist.isConnected) document.querySelector(".tb-center")?.append(tablist);
    const tab = level.tab;
    for (const b of tabButtons) {
      const on = b.dataset.tab === tab;
      b.setAttribute("aria-selected", String(on));
      b.tabIndex = on ? 0 : -1;
    }
  }

  function drawWake(): void {
    const at = level.kind === "desk" ? level : null;
    const p = at && personIn(model, at.personId);
    wakeCard.hidden = !noPane || !p || at?.tab === "report";
    if (wakeCard.hidden || !at || !p) return;
    const f = model?.floors.find((x) => x.id === at.floorId);
    wakeLine.textContent = f && !f.open ? `${f.name} is closed. Starting ${p.name} opens it again.` : `${p.name} is not running.`;
    wakeBtn.textContent = waking ? "Starting..." : `Start ${p.name}`;
    wakeBtn.setAttribute("aria-disabled", String(waking));
    wakeError.textContent = wakeFailure;
    wakeError.hidden = !wakeFailure;
  }

  async function openReport(id: string): Promise<void> {
    const ask = ++reportAsk;
    const name = personIn(model, id)?.name ?? id;
    reportEl.hidden = false;
    reportEl.innerHTML = `<h2>${esc(name)}</h2><p class="of-empty">Reading the board...</p>`;
    let html: string;
    try {
      html = reportHtml(await source.report(id), name);
    } catch (e) {
      html = `<h2>${esc(name)}</h2><p class="co-error" role="alert">${esc(why(e))}</p>`;
    }
    if (ask === reportAsk) reportEl.innerHTML = html;
  }

  function closeReport(): void {
    reportAsk += 1;
    reportEl.hidden = true;
    reportEl.textContent = "";
  }

  /** The app under the layer, and what stands in for it. */
  function sit(l: DeskLevel, focus: boolean): void {
    noPane = !host.showDesk(l.personId, l.tab, focus);
    if (l.tab === "report") void openReport(l.personId);
    else closeReport();
  }

  /** Where the keyboard lands. A pane on show takes it for itself, through focusPane. Off the
   *  Terminal tab the pane is covered, so the tab takes it, or a terminal would keep it unseen. */
  function focusHere(): void {
    if (level.kind !== "desk") crumbsEl.querySelector<HTMLElement>('[aria-current="page"]')?.focus();
    else if (noPane) wakeBtn.focus();
    else if (level.tab !== "terminal") tabButtons.find((b) => b.getAttribute("aria-selected") === "true")?.focus();
  }

  function arrive(l: Level, how: { instant: boolean; focus: boolean; quiet?: boolean }): void {
    level = l;
    moving = !!scene && !how.instant;
    el.dataset.level = l.kind;
    scene?.setLevel(l, how.instant);
    document.body.classList.toggle("office-desk", l.kind === "desk");
    host.levelChanged(l);
    if (l.kind === "desk") sit(l, !how.quiet);
    else {
      noPane = false;
      closeReport();
    }
    wakeFailure = "";
    paint();
    rail.draw();
    if (how.focus) focusHere();
  }

  function paint(): void {
    // Not at a Desk: the new project would take the stage from the person sitting there.
    openEl.hidden = !host.openProject || level.kind === "desk";
    if (model) {
      drawCrumbs(model);
      drawBoxes(model);
    }
    place();
    drawTabs();
    drawWake();
  }

  /** The rail draws itself when the source changes: this timer must not, or a row can
   *  go between a mouse press and its release. */
  function draw(): void {
    if (!shown) return;
    const snap = source.snapshot(Date.now());
    model = snap.model;
    if (!model) {
      showNote(snap.note ?? snap.error ?? "Starting...");
      jobEl.hidden = true;
      msgEl.hidden = true;
      return;
    }
    scene?.setModel(model);
    const clamped = clampLevel(level, model);
    if (clamped !== level) {
      // Left focused under the layer, a terminal would take keys the owner cannot see it take.
      const at = document.activeElement as HTMLElement | null;
      const blind = clamped.kind !== "desk" && !!at?.closest(".xterm");
      if (blind) at?.blur();
      arrive(clamped, { instant: false, focus: blind, quiet: true });
    }
    if (level.kind === "desk") {
      const at = level;
      if (at.tab === "changes" && !noPane && host.changesOpenFor && !host.changesOpenFor()) arrive({ ...at, tab: "terminal" }, { instant: false, focus: false, quiet: true });
      else if (noPane && personIn(model, at.personId)?.paneId) noPane = !host.showDesk(at.personId, at.tab, false);
    }
    paint();
  }

  function fit(): void {
    if (fitTimer !== null) clearTimeout(fitTimer);
    fitTimer = null;
    scene?.resize(stage.clientWidth, stage.clientHeight);
    place();
  }

  /* ---- the scene ---- */

  /** A canvas that held a context cannot get another: a later try needs a new one. */
  function dropScene(): void {
    offMoved?.();
    offMoved = null;
    scene?.dispose();
    scene = null;
    moving = false;
    const fresh = canvas.cloneNode(false) as HTMLCanvasElement;
    canvas.replaceWith(fresh);
    canvas = fresh;
    for (const { el: b } of placed.values()) b.remove();
    placed.clear();
  }

  function sceneFailed(reason: string): void {
    dropScene();
    host.failed(reason);
  }

  async function load(): Promise<void> {
    if (scene || loading) return;
    loading = true;
    try {
      const mod = await host.loadScene();
      if (disposed || !shown) return;
      scene = mod.createScene(canvas, sceneOptions);
      offMoved = scene.onMoved(() => {
        moving = false;
        place();
      });
      scene.resize(stage.clientWidth, stage.clientHeight);
      draw();
      scene.setLevel(level, true);
      moving = false;
      place();
    } catch (e) {
      sceneFailed(why(e));
    } finally {
      loading = false;
    }
  }

  const followVisibility = () => scene?.setVisible(shown && !document.hidden);
  const followMotion = (e: { matches: boolean }) => {
    sceneOptions.reducedMotion = e.matches;
    followVisibility();
  };

  /* ---- what the owner does ---- */

  function go(l: Level): void {
    if (!shown) {
      level = l;
      return;
    }
    if (!sameLevel(l, level)) arrive(l, { instant: false, focus: true });
  }

  function chooseTab(tab: DeskTab, byKey: boolean): void {
    if (level.kind !== "desk" || level.tab === tab) return;
    arrive({ ...level, tab }, { instant: false, focus: false });
    // The terminal takes focus in the next frame. A click on Terminal wants that; nothing else does.
    if (!byKey && tab === "terminal") return;
    const b = tabButtons.find((x) => x.dataset.tab === tab);
    if (byKey) b?.focus();
    requestAnimationFrame(() => b?.focus());
  }

  function wake(): void {
    if (level.kind !== "desk" || waking) return;
    const at = level;
    waking = true;
    wakeFailure = "";
    drawWake();
    source.wakePerson(at.personId).then(
      () => {
        if (shown && !disposed && level.kind === "desk" && level.personId === at.personId) noPane = !host.showDesk(at.personId, level.tab, true);
      },
      (e) => { wakeFailure = why(e); },
    ).finally(() => {
      waking = false;
      drawWake();
    });
  }

  function sendMessage(): void {
    const text = msgBox.value;
    const floorId = msgFloor;
    const to = msgLabel.textContent?.replace(/^Message /, "") ?? "";
    if (!floorId || !text.trim() || sending) return;
    sending = true;
    msgSent.textContent = "";
    msgError.hidden = true;
    if (model) drawBoxes(model);
    source.messageFloor(floorId, text).then(
      () => {
        drafts.delete(floorId);
        if (msgFloor !== floorId) return;
        if (msgBox.value === text) msgBox.value = "";
        msgSent.textContent = `Sent to ${to}.`;
      },
      (e) => {
        if (msgFloor !== floorId) return;
        msgError.textContent = why(e);
        msgError.hidden = false;
      },
    ).finally(() => {
      sending = false;
      if (model) drawBoxes(model);
    });
  }

  crumbsEl.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button[data-crumb]");
    const c = b && trail[Number(b.dataset.crumb)];
    if (c) go(c.level);
  });
  find("[data-leave]").addEventListener("click", () => host.leave());
  openEl.addEventListener("click", () => host.openProject?.());
  tagsEl.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>("button.of-tag");
    const t = b && placed.get(b.dataset.key ?? "")?.tag;
    if (t?.to) go(t.to);
    else if (t?.floor) host.openBoard?.(t.floor);
  });
  stage.addEventListener("click", (e) => {
    if (e.target !== canvas || !scene) return;
    const r = canvas.getBoundingClientRect();
    const hit = scene.pick(e.clientX - r.left, e.clientY - r.top);
    const person = hit?.personId ? personIn(model, hit.personId) : undefined;
    if (person) go(deskOf(person));
    else if (hit?.floorId) go({ kind: "floor", floorId: hit.floorId });
  });
  stage.addEventListener("pointermove", (e) => {
    const now = performance.now();
    if (e.target !== canvas || !scene || moving || now - lastHover < HOVER_MS) return;
    lastHover = now;
    const r = canvas.getBoundingClientRect();
    canvas.style.cursor = scene.pick(e.clientX - r.left, e.clientY - r.top) ? "pointer" : "";
  });
  msgEl.addEventListener("submit", (e) => {
    e.preventDefault();
    sendMessage();
  });
  msgBox.addEventListener("input", () => { msgSent.textContent = ""; });
  el.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && e.target instanceof HTMLTextAreaElement) e.target.blur();
  });
  el.addEventListener("transitionend", (e) => {
    // The stage has its full width only now, so the terminal fits it again.
    if (e.target === el && e.propertyName === "width" && level.kind === "desk") host.refit?.();
  });
  wakeBtn.addEventListener("click", wake);
  tablist.addEventListener("click", (e) => {
    const tab = (e.target as HTMLElement).closest<HTMLElement>("[data-tab]")?.dataset.tab as DeskTab | undefined;
    if (tab) chooseTab(tab, false);
  });
  tablist.addEventListener("keydown", (e) => {
    const here = level;
    if (here.kind !== "desk") return;
    const at = TABS.findIndex((t) => t.id === here.tab);
    const next = e.key === "ArrowRight" ? at + 1 : e.key === "ArrowLeft" ? at - 1 : e.key === "Home" ? 0 : e.key === "End" ? TABS.length - 1 : null;
    if (next === null) return;
    e.preventDefault();
    chooseTab(TABS[(next + TABS.length) % TABS.length].id, true);
  });

  const resized = new ResizeObserver(() => {
    if (fitTimer !== null) clearTimeout(fitTimer);
    fitTimer = setTimeout(fit, RESIZE_MS);
  });
  resized.observe(stage);
  motion.addEventListener("change", followMotion);
  document.addEventListener("visibilitychange", followVisibility);
  const offChange = source.onChange(draw);

  return {
    el,
    show(l?: Level): void {
      if (disposed) return;
      shown = true;
      el.hidden = false;
      source.watching(true);
      timer ??= window.setInterval(draw, TICK_MS);
      model = source.snapshot(Date.now()).model;
      const to = l ?? level;
      // Starting up, nothing has the keyboard yet, so there is nothing to bring along.
      arrive(model ? clampLevel(to, model) : to, { instant: true, focus: document.activeElement !== document.body });
      draw();
      followVisibility();
      if (scene) fit();
      else void load();
    },
    hide(): void {
      shown = false;
      el.hidden = true;
      if (timer !== null) window.clearInterval(timer);
      timer = null;
      scene?.setVisible(false);
      source.watching(false);
      document.body.classList.remove("office-desk");
      drawTabs();
      closeReport();
    },
    level: (): Level => level,
    go,
    escape(ctx: { terminal: boolean; dialog: boolean; dock: boolean; typing: boolean }): boolean {
      if (!escapeGoesUp({ level, ...ctx })) return false;
      go(navUp(level));
      return true;
    },
    sceneFailed,
    dispose(): void {
      if (disposed) return;
      disposed = true;
      shown = false;
      if (timer !== null) window.clearInterval(timer);
      if (fitTimer !== null) clearTimeout(fitTimer);
      timer = null;
      fitTimer = null;
      resized.disconnect();
      motion.removeEventListener("change", followMotion);
      document.removeEventListener("visibilitychange", followVisibility);
      offChange();
      rail.dispose();
      jobBox.dispose();
      dropScene();
      source.watching(false);
      document.body.classList.remove("office-desk");
      tablist.remove();
      el.remove();
    },
  };
}
