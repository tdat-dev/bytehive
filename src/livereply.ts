// The reply a Claude Code agent is writing right now, read off its terminal.
//
// Claude Code writes an answer to its transcript only once the whole block is
// done, so the chat (built from the transcript) shows a reply in chunks, about
// a second late, while the terminal shows it word by word. The screen is the
// only live source: this finds the last "● " block above the spinner and gives
// its text back as markdown-ish paragraphs, or null when there is nothing
// being written (a tool call, a finished turn, another CLI's screen). Pure.
//
// What a working Claude Code v2 screen looks like (composer between the rules):
//
//   ● I added mul(a, b) to math.js and one line to README.md, and changed nothing
//     else.
//     - math.js:9: export function mul(a, b) { return a * b; }, written the same way
//       as add and sub.
//
//   ✢ Schlepping… (running PreToolUse hooks… 1/6 · 3s · ↓ 163 tokens ·
//   thinking with high effort)
//   ────────────────────────────────────────────────────────────────────────────────
//   ❯
//   ────────────────────────────────────────────────────────────────────────────────
//
// The terminal hard-wraps at its width; a line break is only kept where the
// next word would have fitted on the line above it (so it was the agent's own).

/** Where an assistant block starts: "● " (Windows) or "⏺ " (elsewhere). */
const HEADER = /^[●⏺]\s/;
/** Claude Code's spinner line, or the line it leaves when the turn is over. */
const STATUS = /^[·✢✳✶✻✽*✱]\s+\S/;
/** A tool call's header: `● Bash(…)`, `● Read(…)`, an MCP tool, a folded group. */
const TOOL_HEADER = /^[●⏺]\s+(?:[A-Z][\w]*\(|mcp__\w+\(|[\w-]+ - [\w-]+ \(MCP\)|.*\(ctrl\+\w to expand\))/;
/** A folded group still running: "● Reading 1 file, running 1 shell command…". */
const GROUP_HEADER = /^[●⏺]\s+[A-Z][a-z]+ing\b.*(?:…|\.\.\.)\s*$/;
/** A list item starts its own line. */
const LIST_ITEM = /^(?:[-*+•]|\d+[.)])\s/;
/** How far above the composer the spinner may sit (a long plan lists under it). */
const SPINNER_REACH = 30;

/** Columns a string takes: wide (CJK, full-width) characters take two. */
function cells(s: string): number {
  let n = 0;
  for (const ch of s) n += /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]/.test(ch) ? 2 : 1;
  return n;
}
const indentOf = (l: string) => l.length - l.trimStart().length;
/** The composer's rules: a full-width line of ─ at the left edge. */
const isRule = (l: string) => /^[─━]{20,}\s*$/.test(l);

/** The block being written, as text; `cut` when its top is above the screen. */
export interface LiveReply {
  text: string;
  cut: boolean;
}

/** The text of the last assistant block on a Claude Code screen while it is
 *  still working, or null. `cut` says the block starts above what is shown. */
export function liveReplyOf(screen: string): LiveReply | null {
  const lines = screen.split(/\r?\n/).map((l) => l.replace(/\s+$/, ""));
  const rules: number[] = [];
  lines.forEach((l, i) => { if (isRule(l)) rules.push(i); });
  // Only Claude Code draws its composer between two rules; anything else
  // (Codex, opencode, a shell) has no reply of this shape to read.
  if (rules.length < 2) return null;
  const top = rules[rules.length - 2];
  const width = Math.max(...rules.map((i) => cells(lines[i])));

  // The spinner, between the last block and the composer; under it may sit its
  // own wrapped tail, a tip or the plan, none of which is the reply.
  let spin = -1;
  for (let i = top - 1; i >= Math.max(0, top - SPINNER_REACH); i -= 1) {
    if (STATUS.test(lines[i])) { spin = i; break; }
    if (HEADER.test(lines[i])) return null; // no spinner: not working
  }
  if (spin < 0) return null;
  // "✻ Baked for 14s · done": the turn is over, the transcript has it all.
  if (!/…|\.\.\./.test(lines[spin])) return null;

  let end = spin - 1;
  while (end >= 0 && !lines[end].trim()) end -= 1;
  if (end < 0) return null;
  let start = end;
  for (; start >= 0; start -= 1) {
    const l = lines[start];
    if (HEADER.test(l)) break;
    // Something else at the left edge (your message, a thinking line, an
    // earlier turn's end) means the last thing above the spinner is no reply.
    if (l.trim() && indentOf(l) === 0) return null;
  }
  const cut = start < 0;
  const block = lines.slice(Math.max(0, start), end + 1);
  if (!cut && (TOOL_HEADER.test(block[0]) || (block.length === 1 && GROUP_HEADER.test(block[0])))) return null;
  // A tool's output hangs under "⎿": that block is a tool call, not words.
  if (block.some((l) => /^\s*⎿/.test(l))) return null;
  // A block cut at the top has to read like text: its lines sit two columns in
  // (a tool's output sits five in, under its "⎿").
  if (cut && Math.min(...block.filter((l) => l.trim()).map(indentOf)) !== 2) return null;

  const text = unwrap(cut ? block : ["  " + block[0].replace(HEADER, ""), ...block.slice(1)], width);
  return text ? { text, cut } : null;
}

/** Just the text: what `liveReplyOf` reads, or null. */
export function liveReply(screen: string): string | null {
  return liveReplyOf(screen)?.text ?? null;
}

/** Join the lines the terminal wrapped back into paragraphs. Every line sits two
 *  columns in. A line is a wrap of the one above it when its first word would
 *  not have fitted there; list items, table rows and code keep their lines. */
export function unwrap(lines: string[], width: number): string {
  const out: string[] = [];
  let prev = ""; // the physical line the last output line ended with
  let code = false;
  for (const raw of lines) {
    const body = raw.startsWith("  ") ? raw.slice(2) : raw.trimStart();
    const t = body.trim();
    if (/^(```|~~~)/.test(t)) {
      code = !code;
      out.push(t);
      prev = "";
      continue;
    }
    if (code) { out.push(body); prev = ""; continue; }
    if (!t) { out.push(""); prev = ""; continue; }
    const word = t.split(/\s/)[0];
    const wrapped = prev !== "" && !LIST_ITEM.test(t) && !t.startsWith("|") && !prev.trimStart().startsWith("|")
      && cells(prev) + 1 + cells(word) > width;
    if (wrapped) out[out.length - 1] += ` ${t}`;
    else out.push(body);
    prev = raw;
  }
  return out.join("\n").replace(/\n{3,}/g, "\n\n").trim();
}

const words = (s: string): string[] => s.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];

/** The conversation already has this reply: nearly all of its words come, in
 *  order, in one of `texts`. Loose on purpose: the screen shows markdown drawn
 *  (no `**`, no backticks, a link as its words), the transcript the markdown. */
export function landedIn(live: string, texts: string[]): boolean {
  const want = words(live);
  if (!want.length) return true;
  return texts.some((t) => {
    const have = words(t);
    let at = 0;
    let hit = 0;
    for (const w of want) {
      const k = have.indexOf(w, at);
      if (k < 0) continue;
      hit += 1;
      at = k + 1;
    }
    return hit >= Math.ceil(want.length * 0.9);
  });
}
