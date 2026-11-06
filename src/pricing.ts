// What Claude usage costs, estimated from the token counts in Claude Code's
// transcripts at Anthropic's API list prices (USD per million tokens). A
// subscription plan doesn't bill per token, so this is what the same work
// would cost on the API, not a bill. Pure.

import type { ModelUsage } from "./ipc";

export interface Price { in: number; out: number; cacheWrite: number; cacheRead: number }

// First match wins, so newer and more specific ids come before the family.
// Cache writes are the 5-minute rate (1.25× input); reads as each model lists.
const PRICES: Array<{ match: RegExp; p: Price }> = [
  { match: /(fable|mythos)-5-1/, p: { in: 10, out: 50, cacheWrite: 12.5, cacheRead: 0.25 } },
  { match: /fable|mythos/, p: { in: 10, out: 50, cacheWrite: 12.5, cacheRead: 1 } },
  { match: /opus-5-5/, p: { in: 4, out: 20, cacheWrite: 5, cacheRead: 0.2 } },
  { match: /opus-(5|4-[5-9])/, p: { in: 5, out: 25, cacheWrite: 6.25, cacheRead: 0.5 } },
  { match: /opus/, p: { in: 15, out: 75, cacheWrite: 18.75, cacheRead: 1.5 } },
  { match: /sonnet-5/, p: { in: 2, out: 10, cacheWrite: 2.5, cacheRead: 0.2 } },
  { match: /sonnet/, p: { in: 3, out: 15, cacheWrite: 3.75, cacheRead: 0.3 } },
  { match: /haiku/, p: { in: 1, out: 5, cacheWrite: 1.25, cacheRead: 0.1 } },
];

/** The list price of a Claude model id ("claude-opus-5-5", "claude-haiku-4-5-20251001"), or null. */
export function priceFor(model: string): Price | null {
  const m = model.toLowerCase();
  return PRICES.find((x) => x.match.test(m))?.p ?? null;
}

/** Estimated USD for one model's usage, or null when the model isn't priced. */
export function usageCost(u: ModelUsage): number | null {
  const p = priceFor(u.model);
  if (!p) return null;
  return (u.input_tokens * p.in + u.output_tokens * p.out + u.cache_creation * p.cacheWrite + u.cache_read * p.cacheRead) / 1e6;
}

/** Estimated USD across models; null when none of them is priced. */
export function totalCost(rows: ModelUsage[]): number | null {
  let sum: number | null = null;
  for (const r of rows) {
    const c = usageCost(r);
    if (c !== null) sum = (sum ?? 0) + c;
  }
  return sum;
}

/** "$0.004", "$1.23", "$12". */
export function money(usd: number): string {
  if (usd > 0 && usd < 0.01) return `$${usd.toFixed(3)}`;
  if (usd < 10) return `$${usd.toFixed(2)}`;
  return `$${Math.round(usd)}`;
}

/** Changes when the table does, so cached answers stop matching a new price. */
export function pricesFingerprint(): string {
  return PRICES.map((x) => `${x.match.source}:${x.p.in},${x.p.out},${x.p.cacheWrite},${x.p.cacheRead}`).join("|");
}
