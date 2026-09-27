// What each Claude agent's conversation has cost so far: its own session's
// usage (claude_session_usage) at API list prices (pricing.ts). A transcript
// can run to megabytes, so it is read again only after the agent has done
// something new, and not more often than every REFRESH_MS.

import { claudeSessionUsage } from "./ipc";
import { totalCost } from "./pricing";
import type { Pane } from "./panetypes";

const REFRESH_MS = 30_000;

interface Known { key: string; at: number; usd: number | null; busy: boolean }
const known = new Map<string, Known>();
let changed: () => void = () => {};

/** Called when a figure changes, to draw it again. */
export function onAgentCost(fn: () => void): void {
  changed = fn;
}

/** This agent's cost so far as last read (null before the first read, or for
 *  a CLI that isn't Claude); asks again in the background when it is stale. */
export function agentCost(pane: Pane, now = Date.now()): number | null {
  const dir = pane.spec.ranIn;
  const session = pane.spec.sessionId;
  if (pane.spec.badge !== "claude" || !dir || !session) return null;
  const key = `${dir}|${session}`;
  let k = known.get(pane.id);
  if (!k || k.key !== key) {
    k = { key, at: 0, usd: null, busy: false };
    known.set(pane.id, k);
  }
  const moved = (pane.lastOutputAt ?? 0) > k.at;
  if (!k.busy && (k.at === 0 || (moved && now - k.at > REFRESH_MS))) {
    const entry = k;
    entry.busy = true;
    entry.at = now;
    void Promise.resolve()
      .then(() => claudeSessionUsage(dir, session))
      .then((rows) => {
        const usd = totalCost(rows);
        if (usd !== entry.usd) { entry.usd = usd; changed(); }
      })
      .catch(() => {})
      .finally(() => { entry.busy = false; });
  }
  return k.usd;
}

/** Forget a removed agent. */
export function forgetCost(paneId: string): void {
  known.delete(paneId);
}
