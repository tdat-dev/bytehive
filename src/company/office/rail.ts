// src/company/office/rail.ts
/* Needs you: what the owner has to act on, in words, at every level of the office
 * and in the Inbox when there is no 3D. Knows nothing about the scene. */
import "./office.css";
import type { NeedRow, PlanDetail } from "../needs";
import type { OfficeJob, OfficeModel } from "./model";
import { BUILDING, rowTarget, type Level } from "./nav";
import type { OfficeSource } from "./source";

const RAISE_BY = 1_000_000;
const ASK_HINT = "Add marketing to gcare";
const WHY_HINT = "Why send it back? Leave empty to cancel the job.";
const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

/** A failure as words: the engine rejects with Errors, strings or backend objects. */
export function why(e: unknown): string {
  if (e instanceof Error && e.message) return e.message;
  if (typeof e === "string" && e) return e;
  const failed = (e as { Failed?: unknown } | null)?.Failed;
  return typeof failed === "string" ? failed : "Something went wrong.";
}

function planHtml(p: PlanDetail): string {
  const item = (term: string, text: string, cls = "") => `<dt>${esc(term)}</dt><dd${cls}>${esc(text)}</dd>`;
  return `<dl class="co-plan">${[
    item("Hires", p.hires.join(", ") || "Nobody new"),
    ...p.floors.map((f) => item(`Cards on ${f.floor}`, f.cards.join(", "))),
    item("Token cap", p.tokenCap.toLocaleString("en-US")),
    ...p.newRoles.map((r) => item(`New role: ${r.title} (${r.id}, ${r.department})`, r.prompt, ' class="co-prompt"')),
  ].join("")}</dl>`;
}

type Busy = (key: string) => boolean;

/** A busy button is aria-disabled, not disabled: a disabled button cannot keep keyboard focus. */
function actButton(busy: Busy, act: string, label: string, about: string, jobId: string | null, extra = ""): string {
  const working = busy(`${act}:${jobId ?? ""}`);
  const job = jobId ? ` data-job="${esc(jobId)}"` : "";
  return `<button type="button" class="kb-tool" data-act="${act}"${job}${extra} aria-label="${esc(`${label}: ${about}`)}"${working ? ' aria-disabled="true" aria-busy="true"' : ""}>${working ? "Working..." : label}</button>`;
}

const jobHtml = (j: OfficeJob, busy: Busy) => {
  const tokens = `${j.tokensUsed.toLocaleString("en-US")} of ${j.tokenCap.toLocaleString("en-US")} tokens`;
  return `<li class="co-row"><b>${esc(j.title)}</b><span>${esc(j.id)}. ${esc(j.label)}, ${tokens}.</span><div>${actButton(busy, "stop", "Stop", j.title, j.id)}</div></li>`;
};

function rowHtml(r: NeedRow, busy: Busy, goes: boolean): string {
  const btn = (act: string, label: string, extra = "") => actButton(busy, act, label, r.title, r.jobId, extra);
  const buttons =
    r.kind === "plan" ? btn("approve", "Approve") + btn("reject", "Send back")
    : r.kind === "paused" ? btn("raise", "Add 1M tokens")
    : r.kind === "merge" ? btn("finish", "Mark done")
    : r.standing ? btn("dismiss", "Dismiss", ` data-key="${esc(r.key)}"`) : "";
  const note = r.kind === "plan" && r.jobId
    ? `<textarea class="co-note" data-note="${esc(r.jobId)}" rows="2" placeholder="${WHY_HINT}" aria-label="Why send it back"></textarea>`
    : "";
  const title = goes ? `<button type="button" class="co-go" data-row="${esc(r.key)}"><b>${esc(r.title)}</b></button>` : `<b>${esc(r.title)}</b>`;
  return `<li class="co-row co-${r.kind}">${title}` +
    `${r.detail ? `<span>${esc(r.detail)}</span>` : ""}${r.plan ? planHtml(r.plan) : ""}${buttons ? `<div>${buttons}</div>` : ""}${note}</li>`;
}

/** What identifies a control across a redraw. */
const identity = (el: HTMLElement) => [el.dataset.act, el.dataset.job, el.dataset.key, el.dataset.row, el.dataset.note].map((x) => x ?? "").join("|");

const sameLevel = (a: Level, b: Level) => JSON.stringify(a) === JSON.stringify(b);

/** `where` says where the owner stands: a row title that would not move them is not a button. */
export function createRail(source: OfficeSource, go: (l: Level) => void, where: () => Level = () => BUILDING): { mount(el: HTMLElement): void; draw(): void; dispose(): void } {
  let body: HTMLElement | null = null;
  let off: (() => void) | null = null;
  let model: OfficeModel | null = null;
  /** Actions in flight, by "act:job". A second click on one does nothing. */
  const pending = new Set<string>();
  /** Notes being typed, by job: a redraw replaces the rows. */
  const notes = new Map<string, string>();
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
    const focused = at && body.contains(at) && (at.dataset.act || at.dataset.row || at.dataset.note)
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
    put(list, m.needs.map((r) => rowHtml(r, busy, !sameLevel(rowTarget(r, m), here))).join("") || `<li class="co-none">Nothing needs you.</li>`);
    put(jobs, m.jobs.map((j) => jobHtml(j, busy)).join("") || `<li class="co-none">No job is running.</li>`);

    const live = new Set<string>();
    for (const n of body.querySelectorAll<HTMLTextAreaElement>("[data-note]")) {
      live.add(n.dataset.note ?? "");
      n.value = notes.get(n.dataset.note ?? "") ?? "";
    }
    for (const k of [...notes.keys()]) if (!live.has(k)) notes.delete(k);

    if (focused) {
      const same = [...body.querySelectorAll<HTMLElement>("[data-act],[data-row],[data-note]")].find((b) => identity(b) === focused.id);
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

  return {
    mount(el) {
      body = el;
      el.classList.add("co-rail");
      el.innerHTML =
        `<p class="co-error" role="alert" data-msg hidden></p>` +
        `<h3 id="coNeedsH">Needs you <span class="co-n" data-count></span></h3>` +
        `<ul class="co-needs" aria-labelledby="coNeedsH" data-needs></ul>` +
        `<h3 id="coJobsH">Jobs</h3><ul class="co-jobs" aria-labelledby="coJobsH" data-jobs></ul>`;
      const noteOf = (job: string) => [...el.querySelectorAll<HTMLTextAreaElement>("[data-note]")].find((n) => n.dataset.note === job);
      el.addEventListener("click", (e) => {
        const target = e.target as HTMLElement;
        const b = target.closest<HTMLElement>("[data-act]");
        if (b) {
          const job = b.dataset.job ?? "";
          const kind = b.dataset.act;
          if (kind === "dismiss") source.dismiss(b.dataset.key ?? "");
          else if (kind === "approve") void act(`approve:${job}`, () => source.approve(job));
          else if (kind === "reject") {
            const words = noteOf(job)?.value ?? "";
            void act(`reject:${job}`, () => source.reject(job, words)).then((ok) => {
              // A redraw replaced the box, so read it again, and keep words typed since.
              const box = noteOf(job);
              if (!ok || (box?.value ?? "") !== words) return;
              if (box) box.value = "";
              notes.delete(job);
            });
          } else if (kind === "raise") void act(`raise:${job}`, () => source.raiseCap(job, RAISE_BY));
          else if (kind === "finish") void act(`finish:${job}`, () => source.finishJob(job));
          else if (kind === "stop") void act(`stop:${job}`, () => source.stopJob(job));
          return;
        }
        const row = target.closest<HTMLElement>("[data-row]");
        const need = row && model?.needs.find((r) => r.key === row.dataset.row);
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
    },
  };
}

/** The box that gives the Founder a job. */
export function createJobBox(source: OfficeSource): { mount(el: HTMLElement): void; draw(): void; dispose(): void } {
  let body: HTMLElement | null = null;
  let off: (() => void) | null = null;
  let working = false;
  let failure = "";

  const draw = () => {
    const send = body?.querySelector<HTMLElement>("[data-send]");
    const msg = body?.querySelector<HTMLElement>("[data-msg]");
    if (!send || !msg) return;
    msg.textContent = failure;
    msg.hidden = !failure;
    send.textContent = working ? "Working..." : "Send";
    send.setAttribute("aria-disabled", String(!source.snapshot(Date.now()).model || working));
  };

  return {
    mount(el) {
      body = el;
      el.classList.add("co-box");
      el.innerHTML =
        `<form class="co-give"><label for="coAsk">Give the Founder a job</label>` +
        `<textarea id="coAsk" rows="3" placeholder="${ASK_HINT}"></textarea>` +
        `<button type="submit" class="co-send" data-send>Send</button></form>` +
        `<p class="co-error" role="alert" data-msg hidden></p>`;
      const box = el.querySelector<HTMLTextAreaElement>("textarea")!;
      el.querySelector("form")!.addEventListener("submit", (e) => {
        e.preventDefault();
        const ask = box.value;
        if (!ask.trim() || working || !source.snapshot(Date.now()).model) return;
        working = true;
        failure = "";
        draw();
        source.giveJob(ask).then(
          () => { if (box.value === ask) box.value = ""; },
          (err) => { failure = why(err); },
        ).finally(() => { working = false; draw(); });
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
