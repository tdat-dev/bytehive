// The plans' limits at the foot of the agent list: for Claude and for Codex,
// the 5-hour window and the week (and a model's own week when it has one),
// how much of each is used and when it resets.
//   Claude: asked from Anthropic (claude_plan_usage; the numbers of Claude
//   Code's /usage, nothing spent).
//   Codex: read from its newest rollout (codex_plan_usage; what its last
//   request was told), nothing asked of anyone.
// Asked a few seconds after ByteHive opens (never in the way of the CLIs
// starting), then every few minutes while the window shows.

import { claudePlanUsage, codexPlanUsage, type PlanUsage } from "./ipc";

const EVERY_MS = 2 * 60_000;
const FIRST_MS = 5_000;
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

interface Source { usage: PlanUsage | null; problem: string | null; askedAt: number }
const sources: Record<"claude" | "codex", Source> = {
  claude: { usage: null, problem: null, askedAt: 0 },
  codex: { usage: null, problem: null, askedAt: 0 },
};
let timer: number | null = null;
let first: number | null = null;
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

/** When a limit resets, however its CLI says it. */
const resetOf = (l: PlanUsage["limits"][number]): string | null => l.resets_at ?? (l.resets_ms ? new Date(l.resets_ms).toISOString() : null);

/** Every window of these numbers has reset since they were true: the CLI
 *  hasn't been used since, and 0% everywhere is only noise. */
export function allReset(u: PlanUsage, now = Date.now()): boolean {
  return u.limits.length > 0 && u.limits.every((l) => { const r = resetOf(l); return !!r && Date.parse(r) <= now; });
}

/** The rows to show: the 5 hours, the week, then any model's own week that is in use. */
export function planRows(u: PlanUsage, now = Date.now()): PlanRow[] {
  const rows: PlanRow[] = [];
  const level = (p: number, severity: string): PlanRow["level"] =>
    p >= 90 || /critical|exceeded|rejected/i.test(severity) ? "full" : p >= 70 || /warn/i.test(severity) ? "high" : "ok";
  const order = (k: string) => (k === "session" ? 0 : k === "weekly_all" ? 1 : 2);
  for (const l of [...u.limits].sort((a, b) => order(a.kind) - order(b.kind))) {
    const days = /^window_(\d+)$/.exec(l.kind);
    const label = l.kind === "session" ? "5 hours" : l.kind === "weekly_all" ? "This week"
      : l.model ? `${l.model}, this week`
      : days ? (Number(days[1]) % 1440 === 0 ? `${Number(days[1]) / 1440} days` : `${Math.round(Number(days[1]) / 60)} hours`) : "";
    // a model's own week only once it is used; empty it is noise
    if (!label || (l.kind === "weekly_scoped" && l.percent <= 0)) continue;
    const reset = resetOf(l);
    // numbers from before a window reset: it starts from nothing again
    const past = !!reset && Date.parse(reset) <= now;
    const r = past ? { short: "", full: "" } : resetWords(reset, now);
    const percent = past ? 0 : Math.round(l.percent);
    rows.push({ label, percent, resets: r.short, resetsFull: r.full, level: level(percent, past ? "" : l.severity) });
  }
  return rows;
}

const title = (u: PlanUsage) => `${u.provider ?? "Claude"} ${esc(u.plan ? u.plan[0].toUpperCase() + u.plan.slice(1) : "plan")} limits`;
const clock = (ms: number) => new Date(ms).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });

function sectionHtml(src: Source, now: number): string {
  const u = src.usage;
  if (!u) {
    return src.problem === "expired"
      ? `<div class="pl-sec"><p class="pl-note">Claude's sign-in renews the next time an agent runs; its limits show then.</p></div>`
      : "";
  }
  if (allReset(u, now)) return "";
  const rows = planRows(u, now);
  if (!rows.length) return "";
  // how fresh: a Claude answer that failed since, or Codex numbers from its last run
  const asOf = src.problem ? src.askedAt : u.as_of_ms ?? 0;
  const note = asOf && (src.problem || now - asOf > 10 * 60_000)
    ? ` <span class="pl-stale" title="${esc(src.problem === "expired" ? "Claude's sign-in renews the next time an agent runs" : src.problem ?? `What ${u.provider ?? "it"} was told on its last request`)}">as of ${clock(asOf)}</span>` : "";
  return `<div class="pl-sec"><h3 class="pl-h">${title(u)}${note}</h3>${rows.map((r) =>
    `<div class="pl-row ${r.level}" title="${esc(`${r.label}: ${r.percent}% used${r.resetsFull ? `, resets ${r.resetsFull}` : ""}`)}">
      <span class="pl-l">${esc(r.label)}</span><span class="pl-p">${r.percent}%</span>
      <span class="pl-bar" role="meter" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${r.percent}" aria-label="${esc(`${u.provider ?? "Claude"} ${r.label}`)} used"><i style="width:${Math.min(100, r.percent)}%"></i></span>
      ${r.resets ? `<span class="pl-r">resets ${esc(r.resets)}</span>` : ""}
    </div>`).join("")}</div>`;
}

/** The plans as last read, leaving out ones whose every window has reset since. */
export function planSnapshot(now = Date.now()): PlanUsage[] {
  return [sources.claude.usage, sources.codex.usage].filter((u): u is PlanUsage => !!u && !allReset(u, now));
}

/** When the limit an agent of this plan ran into lifts: the soonest reset of a
 *  window that is full (99% or more). Null when none is, or the plan isn't known. */
export function planResetFor(provider: string, now = Date.now()): number | null {
  const u = planSnapshot(now).find((x) => (x.provider ?? "Claude") === provider);
  const resets = (u?.limits ?? [])
    .filter((l) => l.percent >= 99)
    .map((l) => Date.parse(resetOf(l) ?? ""))
    .filter((t) => Number.isFinite(t) && t > now);
  return resets.length ? Math.min(...resets) : null;
}

/** A limit's name in words ("5-hour limit"). */
export function limitName(kind: string, model: string | null): string {
  return kind === "session" ? "5-hour limit" : kind === "weekly_all" ? "weekly limit" : model ? `${model} weekly limit` : "limit";
}

/** When a limit resets, however its CLI says it. */
export function limitResetsAt(l: PlanUsage["limits"][number]): number | null {
  const t = Date.parse(resetOf(l) ?? "");
  return Number.isFinite(t) ? t : null;
}

/** The fullest window of this plan (the one an agent would run out of first). */
function fullest(provider: string, now: number): { percent: number; name: string; at: number | null } | null {
  const u = planSnapshot(now).find((x) => (x.provider ?? "Claude") === provider);
  let best: { percent: number; name: string; at: number | null } | null = null;
  for (const l of u?.limits ?? []) {
    const at = limitResetsAt(l);
    if (at !== null && at <= now) continue;
    if (!best || l.percent > best.percent) best = { percent: l.percent, name: limitName(l.kind, l.model), at };
  }
  return best;
}

/** For a New agent form: how much room the chosen CLI's plan has left, when
 *  little (80% or more used), and another CLI that has room. "" otherwise. */
export function roomNote(badge: string, others: Array<{ badge: string; label: string }>, now = Date.now()): string {
  const provider = badge === "claude" ? "Claude" : badge === "codex" ? "Codex" : "";
  const f = provider ? fullest(provider, now) : null;
  if (!f || f.percent < 80) return "";
  const at = f.at ? new Date(f.at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : "";
  const head = f.percent >= 99
    ? `${provider} is out of its ${f.name}${at ? ` until ${at}` : ""}.`
    : `${provider} has used ${Math.round(f.percent)}% of its ${f.name}${at ? ` · resets ${at}` : ""}.`;
  const roomy = others.find((o) => {
    if (o.badge === badge) return false;
    const p = o.badge === "claude" ? "Claude" : o.badge === "codex" ? "Codex" : "";
    const g = p ? fullest(p, now) : null;
    return !g || g.percent < 80;
  });
  return roomy ? `${head} ${roomy.label} has room.` : head;
}

/** The meter's HTML for the foot of the list; "" when no plan has anything to say. */
export function planHtml(now = Date.now()): string {
  const body = sectionHtml(sources.claude, now) + sectionHtml(sources.codex, now);
  if (!body) return "";
  const chev = `<svg class="iq-chev" viewBox="0 0 10 10" aria-hidden="true"><path d="M3.5 2.5 6 5 3.5 7.5" /></svg>`;
  return `<details class="iq-plan"${planOpen ? " open" : ""}><summary class="pl-sum">${chev}<b>Limits</b>${summaryHtml(now)}</summary><div class="pl-body">${body}</div></details>`;
}

/** Each plan's fullest window, so the folded line still warns before a limit. */
function summaryHtml(now: number): string {
  return planSnapshot(now).map((u) => {
    const top = planRows(u, now).sort((a, b) => b.percent - a.percent)[0];
    return top ? `<span class="pl-s ${top.level}" title="${esc(`${u.provider ?? "Claude"}, ${top.label}`)}">${esc(u.provider ?? "Claude")} ${top.percent}%</span>` : "";
  }).join("");
}

const PLAN_OPEN = "maestro.plan.open";
let planOpen = (() => { try { return localStorage.getItem(PLAN_OPEN) === "1"; } catch { return false; } })();

/** Remember whether the limits are unfolded. */
export function setPlanOpen(open: boolean): void {
  planOpen = open;
  try { localStorage.setItem(PLAN_OPEN, open ? "1" : "0"); } catch { /* storage blocked */ }
}

async function askClaude(): Promise<void> {
  const s = sources.claude;
  try {
    s.usage = await Promise.resolve().then(() => claudePlanUsage());
    s.problem = null;
    s.askedAt = Date.now();
  } catch (e) {
    const why = String((e as { message?: string })?.message ?? e);
    s.problem = /not signed in/.test(why) ? "not signed in" : /expired/.test(why) ? "expired" : why;
    if (s.problem === "not signed in") s.usage = null;
  }
}

async function askCodex(): Promise<void> {
  const s = sources.codex;
  try {
    s.usage = await Promise.resolve().then(() => codexPlanUsage());
    s.askedAt = Date.now();
  } catch { /* keep what it had */ }
}

async function ask(): Promise<void> {
  const before = JSON.stringify([sources.claude.usage, sources.claude.problem, sources.codex.usage]);
  await Promise.all([askClaude(), askCodex()]);
  if (JSON.stringify([sources.claude.usage, sources.claude.problem, sources.codex.usage]) !== before) changed();
}

/** Start asking: a few seconds from now (after the CLIs have started), then
 *  every few minutes while ByteHive shows, and on coming back to it. */
export function startPlanMeter(onChange: () => void, firstMs = FIRST_MS): void {
  changed = onChange;
  if (timer !== null) return;
  first = window.setTimeout(() => { first = null; void ask(); }, firstMs);
  timer = window.setInterval(() => { if (!document.hidden) void ask(); }, EVERY_MS);
  window.addEventListener("focus", onFocus);
}

function onFocus(): void {
  if (first === null && Date.now() - sources.claude.askedAt > 60_000) void ask();
}

export function stopPlanMeter(): void {
  if (timer !== null) window.clearInterval(timer);
  if (first !== null) window.clearTimeout(first);
  timer = first = null;
  window.removeEventListener("focus", onFocus);
}
