// src/company/panel.ts
/* Temporary home of the company until the 3D office replaces it: give the
 * Founder a job, act on what needs you, see who works where. */
import "./panel.css";
import { approve, companyAlerts, companyElsewhere, companyError, companyView, dismissAlert, finishJob, giveJob, onCompanyChange, openPerson, raiseCap, reject, stopJob } from "./engine";
import { needsYou, type NeedRow, type PlanDetail } from "./needs";
import type { Job, JobStatus, Person } from "./types";

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
/** A colour from a role file may be anything: only a plain hex goes in a style. */
const shirt = (c: string) => (/^#[0-9a-f]{3,8}$/i.test(c) ? c : "var(--idle)");

const ASK_HINT = "Add marketing to gcare";
const WHY_HINT = "Why send it back? Leave empty to cancel the job.";
const RAISE_BY = 1_000_000;
/** The Jobs still going, as the panel names them. A Stop works on each. */
const GOING: Partial<Record<JobStatus, string>> = { planning: "Planning", "awaiting-plan": "Waiting for your approval", running: "Running", paused: "Paused" };

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

const jobHtml = (j: Job, busy: Busy) => {
  const tokens = `${j.tokensUsed.toLocaleString("en-US")} of ${j.tokenCap.toLocaleString("en-US")} tokens`;
  return `<li class="co-row"><b>${esc(j.title)}</b><span>${esc(j.id)}. ${GOING[j.status]}, ${tokens}.</span><div>${actButton(busy, "stop", "Stop", j.title, j.id)}</div></li>`;
};

function rowHtml(r: NeedRow, busy: Busy): string {
  const btn = (act: string, label: string, extra = "") => actButton(busy, act, label, r.title, r.jobId, extra);
  const open = r.personId ? btn("open", "Open", ` data-person="${esc(r.personId)}"`) : "";
  const buttons =
    r.kind === "plan" ? btn("approve", "Approve") + btn("reject", "Send back")
    : r.kind === "paused" ? btn("raise", "Add 1M tokens")
    : r.kind === "merge" ? open + btn("finish", "Mark done")
    : open + (r.standing ? btn("dismiss", "Dismiss", ` data-key="${esc(r.key)}"`) : "");
  return `<li class="co-row co-${r.kind}"><b>${esc(r.title)}</b>${r.detail ? `<span>${esc(r.detail)}</span>` : ""}${r.plan ? planHtml(r.plan) : ""}${buttons ? `<div>${buttons}</div>` : ""}</li>`;
}

const personHtml = (p: Person) =>
  `<li><button type="button" class="co-person" data-act="open" data-person="${esc(p.id)}"><i style="background:${shirt(p.look.shirt)}"></i>${esc(p.name)}<span>${esc(p.role)}</span></button></li>`;

/** A failure as words: the engine rejects with Errors, strings or backend objects. */
function why(e: unknown): string {
  if (e instanceof Error && e.message) return e.message;
  if (typeof e === "string" && e) return e;
  const failed = (e as { Failed?: unknown } | null)?.Failed;
  return typeof failed === "string" ? failed : "Something went wrong.";
}

export function createCompanyPanel() {
  let body: HTMLElement | null = null;
  /** Actions in flight, by "act:job". A second click on one does nothing. */
  const pending = new Set<string>();
  let failure = "";

  const busy = (key: string) => pending.has(key);

  const draw = () => {
    if (!body) return;
    const s = companyView();
    const list = body.querySelector<HTMLElement>("[data-needs]");
    const jobs = body.querySelector<HTMLElement>("[data-jobs]");
    const org = body.querySelector<HTMLElement>("[data-org]");
    const msg = body.querySelector<HTMLElement>("[data-msg]");
    const box = body.querySelector<HTMLTextAreaElement>("textarea");
    const send = body.querySelector<HTMLButtonElement>("[data-send]");
    if (!list || !jobs || !org || !msg || !box || !send) return;

    const loadError = companyError();
    const note = failure || (!s && loadError ? `The company could not start: ${loadError}` : "");
    msg.textContent = note;
    msg.hidden = !note;
    const giving = busy("give:");
    send.textContent = giving ? "Working..." : "Send";
    send.setAttribute("aria-disabled", String(!s || giving));

    if (!s) {
      const text = companyElsewhere() ?? (loadError ? "Not available." : "Starting...");
      list.innerHTML = `<li class="co-none">${text}</li>`;
      jobs.innerHTML = "";
      org.innerHTML = `<p class="co-none">${text}</p>`;
      box.placeholder = ASK_HINT;
      return;
    }

    const needs = needsYou(s.jobs, companyAlerts(), { floors: s.company.floors, roles: s.roles });
    box.placeholder = needs.some((r) => r.kind === "plan") ? WHY_HINT : ASK_HINT;

    const founder = s.company.people.filter((p) => p.rank === "founder");
    const floors = s.company.floors.map((f) => {
      const people = s.company.people.filter((p) => p.floor === f.id).sort((a, b) => a.rank.localeCompare(b.rank));
      return `<h4>${esc(f.name)}</h4><ul>${people.map(personHtml).join("") || `<li class="co-none">Nobody here yet</li>`}</ul>`;
    }).join("");

    // A redraw replaces the buttons: keep keyboard focus on the same one.
    const at = document.activeElement as HTMLElement | null;
    const focused = at && body.contains(at) && at.dataset.act ? { act: at.dataset.act, job: at.dataset.job, person: at.dataset.person } : null;
    list.innerHTML = needs.map((r) => rowHtml(r, busy)).join("") || `<li class="co-none">Nothing needs you.</li>`;
    const live = s.jobs.filter((j) => GOING[j.status]);
    jobs.innerHTML = live.map((j) => jobHtml(j, busy)).join("") || `<li class="co-none">No job is running.</li>`;
    org.innerHTML = `<h4>Top floor</h4><ul>${founder.map(personHtml).join("") || `<li class="co-none">No Founder yet</li>`}</ul>${floors}`;
    if (focused) {
      const same = [...body.querySelectorAll<HTMLElement>("[data-act]")].find(
        (b) => b.dataset.act === focused.act && b.dataset.job === focused.job && b.dataset.person === focused.person,
      );
      same?.focus();
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
    mount(el: HTMLElement) {
      body = el;
      el.innerHTML =
        `<form class="co-give"><label for="coAsk">Give the Founder a job</label>` +
        `<textarea id="coAsk" rows="3" placeholder="${ASK_HINT}"></textarea>` +
        `<button type="submit" class="co-send" data-send>Send</button></form>` +
        `<p class="co-error" role="alert" data-msg hidden></p>` +
        `<h3>Needs you</h3><ul class="co-needs" data-needs></ul>` +
        `<h3>Jobs</h3><ul class="co-jobs" data-jobs></ul>` +
        `<h3>People</h3><div class="co-org" data-org></div>`;
      const box = el.querySelector<HTMLTextAreaElement>("textarea")!;
      el.querySelector("form")?.addEventListener("submit", (e) => {
        e.preventDefault();
        const ask = box.value;
        if (!ask.trim() || !companyView()) return;
        void act("give:", () => giveJob(ask)).then((ok) => { if (ok && box.value === ask) box.value = ""; });
      });
      el.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLElement>("[data-act]");
        if (!b) return;
        const job = b.dataset.job ?? "";
        const kind = b.dataset.act;
        if (kind === "open") openPerson(b.dataset.person ?? "");
        else if (kind === "dismiss") dismissAlert(b.dataset.key ?? "");
        else if (kind === "approve") void act(`approve:${job}`, () => approve(job));
        else if (kind === "reject") {
          const note = box.value;
          void act(`reject:${job}`, () => reject(job, note)).then((ok) => { if (ok && box.value === note) box.value = ""; });
        } else if (kind === "raise") void act(`raise:${job}`, () => raiseCap(job, RAISE_BY));
        else if (kind === "finish") void act(`finish:${job}`, () => finishJob(job));
        else if (kind === "stop") void act(`stop:${job}`, () => stopJob(job));
      });
      onCompanyChange(draw);
      draw();
    },
    show: draw,
  };
}
