// What each agent has used so far, whatever its CLI: Claude's session priced
// at API list prices (claude_session_usage + pricing.ts); Codex, opencode,
// Gemini, Qwen and grok from where each keeps it (agent_usage); and for any
// CLI, what its own status line says (Aider's "Cost: $0.02 session", Codex's
// and Gemini's "42% context left"). Read again only after the agent has done
// something new, and not more often than every REFRESH_MS.

import { agentUsage as readAgentUsage, claudeSessionUsage } from "./ipc";
import { totalCost } from "./pricing";
import type { Pane } from "./panetypes";

const REFRESH_MS = 30_000;
/** CLIs whose own files ByteHive can read (agent_usage). */
const READABLE = new Set(["codex", "opencode", "gemini", "qwen", "grok"]);

export interface Usage {
  /** USD so far: at API prices for Claude, the CLI's own figure otherwise. */
  usd: number | null;
  tokens: number | null;
  /** How full its context is, 0–100. */
  contextPct: number | null;
}

interface Known { key: string; at: number; usage: Usage | null; busy: boolean }
const known = new Map<string, Known>();
let changed: () => void = () => {};

/** Called when a figure changes, to draw it again. */
export function onAgentCost(fn: () => void): void {
  changed = fn;
}

/** What a CLI's status line says it has spent and how full its context is. */
export function usageOnScreen(screen: string): Partial<Usage> {
  const out: Partial<Usage> = {};
  // Aider: "Tokens: 2.1k sent, 250 received. Cost: $0.01 message, $0.08 session."
  const session = /Cost:\s*\$([\d,]*\.?\d+)\s*message,\s*\$([\d,]*\.?\d+)\s*session/i.exec(screen);
  if (session) out.usd = Number(session[2].replace(/,/g, ""));
  else {
    const spent = [...screen.matchAll(/\b(?:session cost|total cost|spent|cost)\b[^$\n]{0,16}\$([\d,]*\.?\d+)/gi)].pop();
    if (spent) out.usd = Number(spent[1].replace(/,/g, ""));
  }
  // Codex, Gemini, Qwen: "(42% context left)"
  const left = [...screen.matchAll(/(\d{1,3})%\s*context left/gi)].pop();
  if (left) out.contextPct = Math.max(0, 100 - Number(left[1]));
  // Claude: "Context left until auto-compact: 12%"
  const auto = [...screen.matchAll(/context left until auto-compact:\s*(\d{1,3})%/gi)].pop();
  if (auto && out.contextPct === undefined) out.contextPct = Math.max(0, 100 - Number(auto[1]));
  return out;
}

async function read(pane: Pane, dir: string): Promise<Usage> {
  const u: Usage = { usd: null, tokens: null, contextPct: null };
  const badge = pane.spec.badge;
  if (badge === "claude" && pane.spec.sessionId) {
    const rows = await claudeSessionUsage(dir, pane.spec.sessionId);
    u.usd = totalCost(rows);
    const t = rows.reduce((n, r) => n + r.input_tokens + r.output_tokens + r.cache_creation + r.cache_read, 0);
    u.tokens = t || null;
  } else if (READABLE.has(badge)) {
    const r = await readAgentUsage(badge, dir, pane.spawnedAt ? pane.spawnedAt - 5000 : null);
    u.usd = r.usd ?? null;
    u.tokens = r.tokens ?? null;
    u.contextPct = r.context_pct ?? null;
  }
  // What the screen says fills what the files didn't.
  let screen = "";
  try { screen = pane.term.snapshot(20); } catch { /* no terminal to read */ }
  const s = usageOnScreen(screen);
  if (u.usd === null && s.usd !== undefined) u.usd = s.usd;
  if (u.contextPct === null && s.contextPct !== undefined) u.contextPct = s.contextPct;
  return u;
}

/** This agent's usage as last read (null before the first read); asks again
 *  in the background when it is stale. Shells have none. */
export function agentUsage(pane: Pane, now = Date.now()): Usage | null {
  const dir = pane.spec.ranIn ?? pane.spec.cwd;
  if (!dir || pane.spec.badge === "shell" || pane.spec.badge === "cmd") return null;
  const key = `${pane.spec.badge}|${dir}|${pane.spec.sessionId ?? pane.spawnedAt ?? ""}`;
  let k = known.get(pane.id);
  if (!k || k.key !== key) {
    k = { key, at: 0, usage: null, busy: false };
    known.set(pane.id, k);
  }
  const moved = (pane.lastOutputAt ?? 0) > k.at;
  if (!k.busy && (k.at === 0 || (moved && now - k.at > REFRESH_MS))) {
    const entry = k;
    entry.busy = true;
    entry.at = now;
    void Promise.resolve()
      .then(() => read(pane, dir))
      .then((u) => {
        const was = JSON.stringify(entry.usage);
        entry.usage = u;
        if (JSON.stringify(u) !== was) changed();
      })
      .catch(() => {})
      .finally(() => { entry.busy = false; });
  }
  return k.usage;
}

/** Just the money, for places that only show that. */
export function agentCost(pane: Pane, now = Date.now()): number | null {
  return agentUsage(pane, now)?.usd ?? null;
}

/** Forget a removed agent. */
export function forgetCost(paneId: string): void {
  known.delete(paneId);
}
