// An agent that has hit its plan's usage limit, read from the bottom of its
// screen, and when the limit lifts. Claude Code says "You've hit your limit ·
// resets 7pm (Asia/Saigon)" (or "5-hour limit reached ∙ resets 7pm"), Codex
// "You've hit your usage limit … try again in 2 hours 13 minutes", Gemini
// "exhausted your daily quota". Only the last lines count, so an agent talking
// about limits in its answer isn't taken for one that hit one. Pure.

export interface LimitHit {
  /** The line that said so, trimmed. */
  what: string;
  /** When it lifts (ms), when the line says; else null. */
  resetsAt: number | null;
}

/** Lines at the bottom of the screen looked at. */
const TAIL_LINES = 14;

const HIT = [
  /you'?ve hit your (?:[\w-]+ )?(?:usage |spend |rate )?limit/i,
  /\b(?:usage|session|weekly|5-hour|five-hour|daily|monthly) limit (?:reached|exceeded)/i,
  /\bclaude (?:ai )?usage limit reached/i,
  /\b(?:quota (?:exceeded|exhausted)|exhausted your (?:daily |free )?quota)/i,
];

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

/** "2 hours 13 minutes", "3h 12m", "45 min", "1 day" → ms; null when none. */
export function durationMs(text: string): number | null {
  let ms = 0;
  let any = false;
  for (const m of text.matchAll(/(\d+(?:\.\d+)?)\s*(days?|d\b|hours?|hrs?|h\b|minutes?|mins?|m\b|seconds?|secs?|s\b)/gi)) {
    const n = Number(m[1]);
    const u = m[2].toLowerCase();
    ms += n * (u.startsWith("d") ? 86_400_000 : u.startsWith("h") ? 3_600_000 : u.startsWith("m") ? 60_000 : 1000);
    any = true;
  }
  return any ? ms : null;
}

/** A clock time ("7pm", "7:30 pm", "19:05") on the day `base` is in. */
function atClock(text: string, base: Date): Date | null {
  const m = /\b(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b/i.exec(text);
  if (!m) return null;
  let h = Number(m[1]);
  const min = Number(m[2] ?? 0);
  const ap = m[3]?.toLowerCase();
  if (!ap && !m[2]) return null; // a bare number isn't a time
  if (ap === "pm" && h < 12) h += 12;
  if (ap === "am" && h === 12) h = 0;
  if (h > 23 || min > 59) return null;
  const d = new Date(base);
  d.setHours(h, min, 0, 0);
  return d;
}

/** When a limit lifts, from what the CLI says after "resets" or "try again":
 *  a wait ("in 2 hours 13 minutes"), a time ("7pm": the next 7pm), a day and
 *  time ("Thu 1am", "Oct 2, 1am"). Null when it can't be told. */
export function parseReset(text: string, now: number): number | null {
  const t = text.replace(/\([^)]*\)/g, " ").replace(/\s+/g, " ").trim();
  // a wait
  const wait = /\b(?:in|after)\s+(.+)$/i.exec(t);
  if (wait) {
    const ms = durationMs(wait[1]);
    if (ms) return now + ms;
  }
  const iso = /\b(\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(?::\d{2})?(?:Z|[+-]\d{2}:?\d{2})?)/.exec(t);
  if (iso && Number.isFinite(Date.parse(iso[1]))) return Date.parse(iso[1]);
  const base = new Date(now);
  // "Oct 2, 1am", "October 2 at 1am", "2 Oct 1am"
  const md = new RegExp(`\\b(?:(${MONTHS.join("|")})[a-z]*\\.?\\s+(\\d{1,2})|(\\d{1,2})\\s+(${MONTHS.join("|")})[a-z]*)\\b`, "i").exec(t);
  if (md) {
    const mon = MONTHS.indexOf((md[1] ?? md[4]).toLowerCase().slice(0, 3));
    const day = Number(md[2] ?? md[3]);
    const d = new Date(base.getFullYear(), mon, day);
    const at = atClock(t.slice(md.index + md[0].length), d) ?? d;
    // December's reset seen in January
    if (at.getTime() < now - 86_400_000) at.setFullYear(at.getFullYear() + 1);
    return at.getTime();
  }
  // "Thu 1am", "Thursday at 1am"
  const wd = new RegExp(`\\b(${DAYS.join("|")})[a-z]*\\b`, "i").exec(t);
  if (wd) {
    const want = DAYS.indexOf(wd[1].toLowerCase().slice(0, 3));
    const d = new Date(base);
    d.setDate(d.getDate() + ((want - d.getDay() + 7) % 7));
    const at = atClock(t.slice(wd.index + wd[0].length), d);
    if (at) {
      if (at.getTime() <= now) at.setDate(at.getDate() + 7);
      return at.getTime();
    }
  }
  // "7pm": the next one
  const at = atClock(t, base);
  if (at) {
    if (at.getTime() <= now) at.setDate(at.getDate() + 1);
    return at.getTime();
  }
  return null;
}

/** The limit an agent's screen says it hit, or null. */
export function limitOnScreen(screen: string, now: number): LimitHit | null {
  const lines = screen.split(/\r?\n/).map((l) => l.replace(/[│┃|╭╮╰╯─━■⎿●∙•·]+/g, " ").replace(/\s+/g, " ").trim()).filter(Boolean).slice(-TAIL_LINES);
  for (let i = lines.length - 1; i >= 0; i--) {
    if (!HIT.some((re) => re.test(lines[i]))) continue;
    // the reset is on the same line or the next
    const around = `${lines[i]} ${lines[i + 1] ?? ""}`;
    const r = /\b(?:resets?|reset at|try again|available again)\b\s*(.*)$/i.exec(around);
    return { what: lines[i].slice(0, 160), resetsAt: r ? parseReset(r[1], now) : null };
  }
  return null;
}
