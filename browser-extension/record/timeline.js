/* The timeline of a recording: what the agent did and what went wrong, each
 * with its second in the video. Kept short on purpose: an agent reads it with
 * its own tokens, so the same error ten times is one line with ×10. Pure, so
 * it is tested with node (timeline.test.mjs). */

export const MAX_EVENTS = 400;

/** "0:07.4", "12:03.0" */
export function clock(ms) {
  const s = Math.max(0, ms) / 1000;
  const m = Math.floor(s / 60);
  return `${m}:${(s - m * 60).toFixed(1).padStart(4, "0")}`;
}

const cut = (s, n) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
const one = (s) => String(s ?? "").replace(/\s+/g, " ").trim();

/** One agent action as a timeline line, or "" for calls that change nothing
 *  on screen (reading the page, the console...). */
export function describe(tool, a = {}) {
  switch (tool) {
    case "navigate": return /^(back|forward|reload)$/.test(a.url) ? a.url : `open ${cut(one(a.url), 120)}`;
    case "tab_new": return `new tab${a.url ? ` ${cut(one(a.url), 120)}` : ""}`;
    case "form_input": return `fill ${a.ref} = "${cut(one(a.value), 60)}"`;
    case "upload": return `attach ${(a.paths ?? []).length} file(s) to ${a.ref}`;
    case "dialog": return a.accept ? "accept dialog" : "dismiss dialog";
    case "computer": {
      const at = a.ref ? a.ref : Array.isArray(a.coordinate) ? `(${a.coordinate.map(Math.round).join(",")})` : "";
      switch (a.action) {
        case "type": return `type "${cut(one(a.text), 60)}"`;
        case "key": return `key ${one(a.text)}`;
        case "scroll": return `scroll ${a.scroll_direction ?? "down"} ${at}`.trim();
        case "hover": return `hover ${at}`.trim();
        case "screenshot": case "wait": return "";
        default: return /click$/.test(a.action ?? "") ? `${a.action.replace("_", " ")} ${at}`.trim() : "";
      }
    }
    default: return "";
  }
}

/** Add an event, folding a repeat of the same error into its first line. */
export function addEvent(list, at, kind, text) {
  text = one(text);
  if (!text) return null;
  if (kind !== "action") {
    const same = list.find((e) => e.kind === kind && e.text === text);
    if (same) { same.n += 1; return same; }
  }
  if (list.length >= MAX_EVENTS) return null;
  const e = { at, kind, text, n: 1 };
  list.push(e);
  return e;
}

/** A console message or a failed request worth a line, else "". */
export function consoleLine(level, text) {
  return level === "error" || level === "exception" || level === "assert" ? cut(one(text), 200) : "";
}
export function networkLine(method, url, status, error) {
  if (error) return /ERR_ABORTED/.test(error) ? "" : `${method} ${cut(url, 140)} failed: ${error}`;
  return status >= 400 ? `${method} ${cut(url, 140)} ${status}` : "";
}

const TAG = { action: "", error: "ERROR ", net: "NET " };

export function toMarkdown(head, events) {
  const lines = events.map((e) => `${clock(e.at)} ${TAG[e.kind] ?? ""}${e.text}${e.n > 1 ? ` (×${e.n})` : ""}${e.frames ? ` [${e.frames}]` : ""}`);
  return `# ${head}\n\n${lines.join("\n") || "(nothing happened)"}\n`;
}

export function counts(events) {
  let actions = 0, errors = 0;
  for (const e of events) e.kind === "action" ? actions++ : (errors += e.n);
  return { actions, errors };
}
