// src/company/office/rail.ts
/* Needs you: what the owner has to act on, as cards, at every level of the office and in
 * the Inbox when there is no 3D. Knows nothing about the scene: a portrait comes in as a
 * picture from whoever has one. */
import "./office.css";
import { confirmModal } from "../../confirmmodal";
import { glyph, icon } from "../../icons";
import type { NeedRow, PlanDetail } from "../needs";
import type { OfficeJob, OfficeModel, OfficePerson } from "./model";
import { BUILDING, rowTarget, type Level } from "./nav";
import type { OfficeSource } from "./source";

const RAISE_BY = 1_000_000;
const ASK_HINT = "Add marketing to gcare";
const WHY_HINT = "Fewer hires, and leave QA for later";
const NOTE_NEEDED = "Send back needs a note. To drop the job instead, use Cancel job in its menu.";
/** Examples for an empty box: what a first job can sound like. */
const EXAMPLES = ["Add a pricing page", "Fix the login", "Write a launch post"];
const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** A failure as words: the engine rejects with Errors, strings or backend objects. */
export function why(e: unknown): string {
  if (e instanceof Error && e.message) return e.message;
  if (typeof e === "string" && e) return e;
  const failed = (e as { Failed?: unknown } | null)?.Failed;
  return typeof failed === "string" ? failed : "Something went wrong.";
}

const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;
const withArticle = (hire: string) => `${/^[aeiou]/i.test(hire) ? "an" : "a"} ${hire}`;

function listed(items: string[]): string {
  if (items.length < 2) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

/** 5,000,000 as 5M, and 1,200,000 as 1.2M. */
export function tokensShort(n: number): string {
  if (n >= 1_000_000) return `${+(n / 1_000_000).toFixed(1)}M`;
  return n >= 1000 ? `${+(n / 1000).toFixed(1)}K` : String(n);
}

/** The whole plan as one sentence: it reads as a proposal, and the list stays folded under it. */
export function planSentence(p: PlanDetail): string {
  const cards = p.floors.reduce((n, f) => n + f.cards.length, 0);
  const wants = p.hires.length ? `The Founder wants to hire ${listed(p.hires.map(withArticle))}` : "The Founder wants to do this with the people you have";
  return `${wants}, ${plural(cards, "card")}, up to ${tokensShort(p.tokenCap)} tokens.`;
}

function planHtml(p: PlanDetail, summary: string): string {
  const item = (term: string, text: string, cls = "") => `<dt>${esc(term)}</dt><dd${cls}>${esc(text)}</dd>`;
  return `<dl class="co-plan">${[
    ...(summary ? [item("Plan", summary)] : []),
    item("Hires", p.hires.join(", ") || "Nobody new"),
    ...p.floors.map((f) => item(`Cards on ${f.floor}`, f.cards.join(", "))),
    item("Token cap", p.tokenCap.toLocaleString("en-US")),
    ...p.newRoles.map((r) => item(`New role: ${r.title} (${r.id}, ${r.department})`, r.prompt, ' class="co-prompt"')),
  ].join("")}</dl>`;
}

type Busy = (key: string) => boolean;

/** A busy button is aria-disabled, not disabled: a disabled button cannot keep keyboard focus. */
function actButton(busy: Busy, act: string, label: string, about: string, jobId: string | null, kind: string, extra = ""): string {
  const working = busy(`${act}:${jobId ?? ""}`);
  const job = jobId ? ` data-job="${esc(jobId)}"` : "";
  return `<button type="button" class="btn ${kind}" data-act="${act}"${job}${extra} aria-label="${esc(`${label}: ${about}`)}"${working ? ' aria-disabled="true" aria-busy="true"' : ""}>${working ? "Working..." : label}</button>`;
}

/** The one word that fits the line. The full label is there for a screen reader. */
const JOB_WORD: Partial<Record<OfficeJob["status"], string>> = { "awaiting-plan": "Waiting" };
const JOB_GLYPH: Partial<Record<OfficeJob["status"], string>> = { running: "working", paused: "stopped", planning: "idle", "awaiting-plan": "needs" };
/** A job nobody has started on is cancelled. One that has people on it is stopped. */
const stopWord = (j: OfficeJob) => (j.status === "planning" || j.status === "awaiting-plan" ? "Cancel job" : "Stop job");

const jobHtml = (j: OfficeJob, busy: Busy, menuOpen: boolean) => {
  const done = j.tokenCap > 0 ? Math.min(100, Math.round((j.tokensUsed / j.tokenCap) * 100)) : 0;
  const tokens = `${j.tokensUsed.toLocaleString("en-US")} of ${j.tokenCap.toLocaleString("en-US")} tokens`;
  return `<li class="co-job" style="--done:${done}%">${glyph(JOB_GLYPH[j.status] ?? "idle")}<b title="${esc(j.title)}">${esc(j.title)}</b>` +
    `<span class="co-state" title="${esc(tokens)}">${esc(JOB_WORD[j.status] ?? j.label)}, ${done}%</span><span class="of-sr">${esc(j.id)}. ${esc(j.label)}, ${tokens}.</span>` +
    `<button type="button" class="btn icon sm" data-menu="${esc(j.id)}" aria-haspopup="menu" aria-expanded="${menuOpen}" aria-label="${esc(`More for ${j.title}`)}">${icon("ellipsis")}</button>` +
    `<div class="co-menu" role="menu"${menuOpen ? "" : " hidden"}>${actButton(busy, "stop", stopWord(j), j.title, j.id, "danger", ' role="menuitem"')}</div></li>`;
};

const everyone = (m: OfficeModel | null): OfficePerson[] => (m ? [...(m.founder ? [m.founder] : []), ...m.floors.flatMap((f) => f.people)] : []);

/** The colour behind an initial. It lands in an attribute, so it is only ever a plain hex. */
const safeColour = (c: string) => (/^#[0-9a-f]{3,8}$/i.test(c) ? c : "#252a37");

/** A face: the rendered portrait when there is one, and until then, or with no 3D, an initial. */
function faceHtml(p: OfficePerson | undefined, face: Face): string {
  const url = p ? face(p) : null;
  if (url) return `<img class="co-face" src="${esc(url)}" alt="">`;
  return p ? `<span class="co-face" style="--c:${safeColour(p.shirt)}" aria-hidden="true">${esc([...p.name][0]?.toUpperCase() ?? "?")}</span>` : "";
}

const roleOf = (p: OfficePerson, m: OfficeModel) => {
  const floor = m.floors.find((f) => f.id === p.floorId)?.name;
  return floor ? `${p.title}, ${floor}` : p.title;
};

interface RowLook { goes: boolean; primary: boolean; asking: boolean; short: boolean; open: boolean; face: Face; model: OfficeModel }

function rowHtml(r: NeedRow, busy: Busy, o: RowLook): string {
  const main = o.primary ? "primary" : "secondary";
  const btn = (act: string, label: string, kind: string, extra = "") => actButton(busy, act, label, r.title, r.jobId, kind, extra);
  const open = (label: string) => (o.goes ? `<button type="button" class="btn ${main}" data-open="${esc(r.key)}" aria-label="${esc(`${label}: ${r.title}`)}">${label}</button>` : "");
  const buttons =
    r.kind === "plan" ? btn("approve", "Approve", main) + btn("reject", "Send back", "secondary")
    : r.kind === "paused" ? btn("raise", "Add 1M tokens", main)
    : r.kind === "merge" ? open("Review") + btn("finish", "Mark done", o.goes ? "secondary" : main)
    : r.standing ? btn("dismiss", "Dismiss", "secondary", ` data-key="${esc(r.key)}"`) : open("Open");
  const person = everyone(o.model).find((p) => p.id === r.personId);
  const who = r.kind === "plan" ? o.model.founder ?? undefined : person;
  const named = who && (r.kind === "plan" || person);
  const head = `${faceHtml(who, o.face)}<span class="co-who">${named ? `<b>${esc(who.name)}</b><span class="co-role">${esc(r.kind === "plan" ? r.title : roleOf(who, o.model))}</span>` : `<b>${esc(r.title)}</b>`}</span>`;
  const top = o.goes ? `<button type="button" class="co-go" data-row="${esc(r.key)}">${head}${icon("chevron-right")}</button>` : `<div class="co-head">${head}</div>`;
  // With a name on top, the title is what they need. Without one, the title is already up there.
  const what = r.kind === "plan" && r.plan ? planSentence(r.plan) : named ? r.title : r.detail;
  const more = named && r.kind !== "plan" && r.detail ? r.detail : "";
  const fold = r.plan ? `<details class="co-more"${o.open || r.plan.newRoles.length ? " open" : ""}><summary data-more="${esc(r.key)}">Details</summary>${planHtml(r.plan, r.detail)}</details>` : "";
  const note = r.kind === "plan" && r.jobId
    ? `<div class="co-field"${o.asking ? "" : " hidden"}><label>Why send it back?<textarea data-note="${esc(r.jobId)}" rows="2" placeholder="${esc(WHY_HINT)}"></textarea></label>${o.short ? `<p class="co-hint" role="status">${esc(NOTE_NEEDED)}</p>` : ""}</div>`
    : "";
  const words = `${what ? `<p class="co-what">${esc(what)}</p>` : ""}${more ? `<p class="co-more-line">${esc(more)}</p>` : ""}`;
  const acts = buttons ? `<div class="co-acts">${buttons}</div>` : "";
  // A plan has more to read. Everything else is a line, with its answer beside it.
  const rest = r.kind === "plan" ? `${words}${fold}${note}${acts}` : `<div class="co-line"><div class="co-words">${words}</div>${acts}</div>`;
  return `<li class="co-card co-${r.kind}">${top}${rest}</li>`;
}

/** What identifies a control across a redraw. */
const identity = (el: HTMLElement) => [el.dataset.act, el.dataset.job, el.dataset.key, el.dataset.row, el.dataset.note, el.dataset.open, el.dataset.menu, el.dataset.more].map((x) => x ?? "").join("|");

const sameLevel = (a: Level, b: Level) => JSON.stringify(a) === JSON.stringify(b);

/** A person's portrait, or null until there is one. */
type Face = (p: OfficePerson) => string | null;

export interface RailOptions {
  face?: Face;
  /** Asks before a job ends. A test answers for the owner. */
  confirm?(job: OfficeJob): Promise<boolean>;
}

/** Ending a job is its own decision: it says what stops and what stays. */
async function confirmEnd(j: OfficeJob): Promise<boolean> {
  const r = await confirmModal({
    title: `${stopWord(j)}?`,
    message: `${j.title} ends here, and its people stop working on it. Its cards and branches stay.`,
    okLabel: stopWord(j),
    danger: true,
  });
  return r.ok;
}

/** `where` says where the owner stands: a row title that would not move them is not a button. */
export function createRail(source: OfficeSource, go: (l: Level) => void, where: () => Level = () => BUILDING, o: RailOptions = {}): { mount(el: HTMLElement): void; draw(): void; dispose(): void } {
  const face: Face = o.face ?? (() => null);
  const confirmJob = o.confirm ?? confirmEnd;
  let body: HTMLElement | null = null;
  let off: (() => void) | null = null;
  let model: OfficeModel | null = null;
  /** Actions in flight, by "act:job". A second click on one does nothing. */
  const pending = new Set<string>();
  /** Notes being typed, by job: a redraw replaces the rows. */
  const notes = new Map<string, string>();
  /** Jobs whose note box is open, and the ones told they need a note. */
  const asking = new Set<string>();
  const short = new Set<string>();
  /** Plans with their details unfolded, by row. */
  const unfolded = new Set<string>();
  let menu: string | null = null;
  let failure = "";
  /** What each list holds now. A redraw that changes nothing leaves it alone: replacing a
   *  row between a mouse press and its release loses the click. */
  const drawn = new Map<HTMLElement, string>();
  const put = (el: HTMLElement, html: string) => {
    if (drawn.get(el) === html) return;
    drawn.set(el, html);
    el.innerHTML = html;
  };

  const busy = (key: string) => pending.has(key);

  const calmLine = (m: OfficeModel) => {
    if (m.counts.working) return `${m.counts.working === 1 ? "1 person is" : `${m.counts.working} people are`} working.`;
    return m.counts.people > 1 ? "Everyone is resting." : "Give the Founder a job to get started.";
  };

  const draw = () => {
    if (!body) return;
    const list = body.querySelector<HTMLElement>("[data-needs]");
    const jobs = body.querySelector<HTMLElement>("[data-jobs]");
    const msg = body.querySelector<HTMLElement>("[data-msg]");
    const count = body.querySelector<HTMLElement>("[data-count]");
    if (!list || !jobs || !msg || !count) return;

    const snap = source.snapshot(Date.now());
    model = snap.model;
    const note = failure || (!snap.model && snap.error ? `The company could not start: ${snap.error}` : "");
    msg.textContent = note;
    msg.hidden = !note;

    for (const n of body.querySelectorAll<HTMLTextAreaElement>("[data-note]")) notes.set(n.dataset.note ?? "", n.value);
    const at = document.activeElement as HTMLElement | null;
    const focused = at && body.contains(at) && (at.dataset.act || at.dataset.row || at.dataset.note || at.dataset.open || at.dataset.menu || at.dataset.more)
      ? { id: identity(at), start: (at as HTMLTextAreaElement).selectionStart, end: (at as HTMLTextAreaElement).selectionEnd }
      : null;

    if (!model) {
      const text = snap.note ?? (snap.error ? "Not available." : "Starting...");
      put(list, `<li class="co-none">${esc(text)}</li>`);
      put(jobs, "");
      count.textContent = "";
      return;
    }

    count.textContent = model.needs.length ? String(model.needs.length) : "";
    const here = where();
    const m = model;
    // One honey button: the first thing that asks for something, and nothing below it.
    const lead = m.needs.findIndex((r) => r.kind !== "alert" || !r.standing);
    put(list, m.needs.map((r, i) => rowHtml(r, busy, {
      goes: !sameLevel(rowTarget(r, m), here),
      primary: i === lead,
      asking: !!r.jobId && (asking.has(r.jobId) || !!notes.get(r.jobId)),
      short: !!r.jobId && short.has(r.jobId),
      open: unfolded.has(r.key),
      face,
      model: m,
    })).join("") || `<li class="co-none"><b>Nothing needs you.</b><span>${esc(calmLine(m))}</span></li>`);
    put(jobs, m.jobs.map((j) => jobHtml(j, busy, menu === j.id)).join("") || `<li class="co-none">No job is running.</li>`);

    const live = new Set<string>();
    for (const n of body.querySelectorAll<HTMLTextAreaElement>("[data-note]")) {
      live.add(n.dataset.note ?? "");
      n.value = notes.get(n.dataset.note ?? "") ?? "";
    }
    for (const k of [...notes.keys()]) if (!live.has(k)) notes.delete(k);

    if (focused) {
      const same = [...body.querySelectorAll<HTMLElement>("[data-act],[data-row],[data-note],[data-open],[data-menu],[data-more]")].find((b) => identity(b) === focused.id);
      same?.focus();
      if (same instanceof HTMLTextAreaElement && focused.start !== null) same.setSelectionRange(focused.start, focused.end ?? focused.start);
    }
  };

  /** Run one action once at a time; show why it failed instead of throwing. */
  const act = async (key: string, run: () => Promise<void>): Promise<boolean> => {
    if (pending.has(key)) return false;
    pending.add(key);
    failure = "";
    draw();
    try {
      await run();
      return true;
    } catch (e) {
      failure = why(e);
      return false;
    } finally {
      pending.delete(key);
      draw();
    }
  };

  const closeMenu = (back = false) => {
    if (menu === null) return;
    const was = menu;
    menu = null;
    draw();
    if (back) body?.querySelector<HTMLElement>(`[data-menu="${CSS.escape(was)}"]`)?.focus();
  };
  const outside = (e: Event) => { if (menu !== null && !(e.target as HTMLElement).closest?.(".co-job")) closeMenu(); };

  return {
    mount(el) {
      body = el;
      el.classList.add("co-rail");
      el.innerHTML =
        `<p class="co-error" role="alert" data-msg hidden></p>` +
        `<h3 id="coNeedsH">Needs you <span class="co-n" data-count></span></h3>` +
        `<ul class="co-needs" aria-labelledby="coNeedsH" data-needs></ul>` +
        `<h3 id="coJobsH">Jobs</h3><ul class="co-jobs" aria-labelledby="coJobsH" data-jobs></ul>`;
      el.addEventListener("input", (e) => {
        if (short.delete((e.target as HTMLElement).dataset.note ?? "")) draw();
      });
      el.addEventListener("toggle", (e) => {
        const key = (e.target as HTMLElement).querySelector<HTMLElement>("[data-more]")?.dataset.more;
        if (!key) return;
        if ((e.target as HTMLDetailsElement).open) unfolded.add(key);
        else unfolded.delete(key);
      }, true);
      el.addEventListener("keydown", (e) => { if (e.key === "Escape" && menu !== null) { e.stopPropagation(); closeMenu(true); } });
      document.addEventListener("pointerdown", outside);
      const noteOf = (job: string) => [...el.querySelectorAll<HTMLTextAreaElement>("[data-note]")].find((n) => n.dataset.note === job);
      el.addEventListener("click", (e) => {
        const target = e.target as HTMLElement;
        const opener = target.closest<HTMLElement>("[data-menu]");
        if (opener) {
          menu = menu === opener.dataset.menu ? null : opener.dataset.menu ?? null;
          draw();
          if (menu !== null) el.querySelector<HTMLElement>(".co-menu:not([hidden]) [data-act]")?.focus();
          return;
        }
        const b = target.closest<HTMLElement>("[data-act]");
        if (b) {
          const job = b.dataset.job ?? "";
          const kind = b.dataset.act;
          if (kind === "dismiss") source.dismiss(b.dataset.key ?? "");
          else if (kind === "approve") void act(`approve:${job}`, () => source.approve(job));
          else if (kind === "reject") {
            const words = noteOf(job)?.value ?? "";
            if (!words.trim()) {
              // The first press opens the box. An empty note never reaches the job.
              if (asking.has(job)) short.add(job);
              asking.add(job);
              draw();
              noteOf(job)?.focus();
              return;
            }
            short.delete(job);
            void act(`reject:${job}`, () => source.reject(job, words)).then((ok) => {
              // A redraw replaced the box, so read it again, and keep words typed since.
              const box = noteOf(job);
              if (!ok || (box?.value ?? "") !== words) return;
              if (box) box.value = "";
              notes.delete(job);
              asking.delete(job);
              draw();
            });
          } else if (kind === "raise") void act(`raise:${job}`, () => source.raiseCap(job, RAISE_BY));
          else if (kind === "finish") void act(`finish:${job}`, () => source.finishJob(job));
          else if (kind === "stop") {
            const j = model?.jobs.find((x) => x.id === job);
            closeMenu();
            if (!j) return;
            void confirmJob(j).then((yes) => { if (yes) void act(`stop:${job}`, () => source.stopJob(job)); });
          }
          return;
        }
        const row = target.closest<HTMLElement>("[data-row],[data-open]");
        const key = row?.dataset.row ?? row?.dataset.open;
        const need = key && model?.needs.find((r) => r.key === key);
        if (need && model) go(rowTarget(need, model));
      });
      off = source.onChange(draw);
      draw();
    },
    draw,
    dispose() {
      off?.();
      off = null;
      body = null;
      document.removeEventListener("pointerdown", outside);
    },
  };
}

/** Enter sends and Shift+Enter breaks the line. While an IME is composing, Enter belongs to it:
 *  Vietnamese typing would send half a word. */
export const sendsOnEnter = (e: KeyboardEvent): boolean => e.key === "Enter" && !e.shiftKey && !e.isComposing && e.keyCode !== 229;

/** The box that gives the Founder a job. */
export function createJobBox(source: OfficeSource): { mount(el: HTMLElement): void; draw(): void; dispose(): void } {
  let body: HTMLElement | null = null;
  let off: (() => void) | null = null;
  let working = false;
  let failure = "";

  const draw = () => {
    const send = body?.querySelector<HTMLElement>("[data-send]");
    const msg = body?.querySelector<HTMLElement>("[data-msg]");
    const box = body?.querySelector<HTMLTextAreaElement>("textarea");
    const chips = body?.querySelector<HTMLElement>("[data-chips]");
    if (!send || !msg || !box || !chips) return;
    msg.textContent = failure;
    msg.hidden = !failure;
    send.textContent = working ? "Working..." : "Send";
    const m = source.snapshot(Date.now()).model;
    send.setAttribute("aria-disabled", String(!m || working));
    // Written and ready is your move, unless something above it is waiting for you first.
    send.classList.toggle("primary", !!box.value.trim() && !!m && !m.needs.length);
    send.classList.toggle("secondary", !send.classList.contains("primary"));
    chips.hidden = !!box.value;
  };

  return {
    mount(el) {
      body = el;
      el.classList.add("co-box");
      el.innerHTML =
        `<form class="co-give"><label for="coAsk">Give the Founder a job</label>` +
        `<textarea id="coAsk" rows="2" placeholder="${ASK_HINT}"></textarea>` +
        `<div class="co-chips" data-chips>${EXAMPLES.map((x) => `<button type="button" class="btn secondary sm" data-chip>${esc(x)}</button>`).join("")}</div>` +
        `<div class="co-foot"><span class="co-hint">Enter sends. Shift+Enter starts a new line.</span><button type="submit" class="btn secondary" data-send>Send</button></div></form>` +
        `<p class="co-error" role="alert" data-msg hidden></p>`;
      const box = el.querySelector<HTMLTextAreaElement>("textarea")!;
      const send = () => {
        const ask = box.value;
        if (!ask.trim() || working || !source.snapshot(Date.now()).model) return;
        working = true;
        failure = "";
        draw();
        source.giveJob(ask).then(
          () => { if (box.value === ask) box.value = ""; },
          (err) => { failure = why(err); },
        ).finally(() => { working = false; draw(); });
      };
      el.querySelector("form")!.addEventListener("submit", (e) => {
        e.preventDefault();
        send();
      });
      box.addEventListener("keydown", (e) => {
        if (!sendsOnEnter(e)) return;
        e.preventDefault();
        send();
      });
      box.addEventListener("input", draw);
      el.addEventListener("click", (e) => {
        const chip = (e.target as HTMLElement).closest<HTMLElement>("[data-chip]");
        if (!chip) return;
        box.value = chip.textContent ?? "";
        box.focus();
        draw();
      });
      off = source.onChange(draw);
      draw();
    },
    draw,
    dispose() {
      off?.();
      off = null;
      body = null;
    },
  };
}
