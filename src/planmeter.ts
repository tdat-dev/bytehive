// The Claude plan's limits at the foot of the agent list: the 5-hour window
// and the week (and a model's own week when it has one), how much of each is
// used and when it resets. Asked from Anthropic every few minutes through
// claude_plan_usage (the same numbers as Claude Code's /usage; nothing spent).

import { claudePlanUsage, type PlanUsage } from "./ipc";

const EVERY_MS = 2 * 60_000;
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

let usage: PlanUsage | null = null;
let problem: string | null = null;
let askedAt = 0;
let timer: number | null = null;
let changed: () => void = () => {};

export interface PlanRow { label: string; percent: number; resets: string; resetsFull: string; level: "ok" | "high" | "full" }

/** "in 3h 12m", "in 40m"; the day and time when it is further off. */
export function resetWords(iso: string | null, now = Date.now()): { short: string; full: string } {
  if (!iso) return { short: "", full: "" };
  const at = Date.parse(iso);
  if (!Number.isFinite(at)) return { short: "", full: "" };
  const d = new Date(at);
  const full = d.toLocaleString([], { weekday: "short", hour: "2-digit", minute: "2-digit" });
  const mins = Math.max(0, Math.round((at - now) / 60_000));
  if (mins >= 24 * 60) return { short: full, full };
  const h = Math.floor(mins / 60);
  return { short: `in ${h ? `${h}h ${mins % 60}m` : `${mins}m`}`, full };
}

/** The rows to show: the 5 hours, the week, then any model's own week that is in use. */
export function planRows(u: PlanUsage, now = Date.now()): PlanRow[] {
  const rows: PlanRow[] = [];
  const level = (p: number, severity: string): PlanRow["level"] =>
    p >= 90 || /critical|exceeded|rejected/i.test(severity) ? "full" : p >= 70 || /warn/i.test(severity) ? "high" : "ok";
  const order = (k: string) => (k === "session" ? 0 : k === "weekly_all" ? 1 : 2);
  for (const l of [...u.limits].sort((a, b) => order(a.kind) - order(b.kind))) {
    const label = l.kind === "session" ? "5 hours" : l.kind === "weekly_all" ? "This week" : l.model ? `${l.model}, this week` : "";
    // a model's own week only once it is used; empty it is noise
    if (!label || (l.kind === "weekly_scoped" && l.percent <= 0)) continue;
    const r = resetWords(l.resets_at, now);
    rows.push({ label, percent: Math.round(l.percent), resets: r.short, resetsFull: r.full, level: level(l.percent, l.severity) });
  }
  return rows;
}

/** The meter's HTML for the foot of the list; "" when there is nothing to say
 *  (no Claude sign-in, an API key, or not asked yet). */
export function planHtml(now = Date.now()): string {
  if (!usage) {
    if (problem === "expired") return `<section class="iq-plan" aria-label="Claude plan limits"><p class="pl-note">Claude's sign-in renews the next time an agent runs; the limits show then.</p></section>`;
    return "";
  }
  const rows = planRows(usage, now);
  if (!rows.length) return "";
  const stale = problem ? ` <span class="pl-stale" title="${esc(problem === "expired" ? "Claude's sign-in renews the next time an agent runs" : problem)}">as of ${new Date(askedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>` : "";
  return `<section class="iq-plan" aria-label="Claude plan limits"><h3 class="pl-h">Claude ${esc(usage.plan ? usage.plan[0].toUpperCase() + usage.plan.slice(1) : "plan")} limits${stale}</h3>${rows.map((r) =>
    `<div class="pl-row ${r.level}" title="${esc(`${r.label}: ${r.percent}% used${r.resetsFull ? `, resets ${r.resetsFull}` : ""}`)}">
      <span class="pl-l">${esc(r.label)}</span><span class="pl-p">${r.percent}%</span>
      <span class="pl-bar" role="meter" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${r.percent}" aria-label="${esc(r.label)} used"><i style="width:${Math.min(100, r.percent)}%"></i></span>
      ${r.resets ? `<span class="pl-r">resets ${esc(r.resets)}</span>` : ""}
    </div>`).join("")}</section>`;
}

async function ask(): Promise<void> {
  try {
    usage = await Promise.resolve().then(() => claudePlanUsage());
    problem = null;
    askedAt = Date.now();
  } catch (e) {
    const why = String((e as { message?: string })?.message ?? e);
    problem = /not signed in/.test(why) ? "not signed in" : /expired/.test(why) ? "expired" : why;
    if (problem === "not signed in") usage = null;
  }
  changed();
}

/** Start asking (now, then every few minutes, and on coming back to the app). */
export function startPlanMeter(onChange: () => void): void {
  changed = onChange;
  if (timer !== null) return;
  void ask();
  timer = window.setInterval(() => { if (!document.hidden) void ask(); }, EVERY_MS);
  window.addEventListener("focus", onFocus);
}

function onFocus(): void {
  if (Date.now() - askedAt > 60_000) void ask();
}

export function stopPlanMeter(): void {
  if (timer !== null) window.clearInterval(timer);
  timer = null;
  window.removeEventListener("focus", onFocus);
}
