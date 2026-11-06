/** Text shaping before a hash or a rule sees it. Pure. */

const UNITS: Record<string, number> = {
  k: 1000,
  "nghìn": 1000,
  nghin: 1000,
  "triệu": 1_000_000,
  trieu: 1_000_000,
};

/** One space between words, no ends. */
export function collapse(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

/** "120k", "250 nghìn" -> 120000, 250000. Other numbers are left alone. */
export function expandNumbers(text: string): string {
  return text.replace(
    /(\d+(?:[.,]\d+)?)\s*(k|nghìn|nghin|triệu|trieu)(?=\s|$|[.,;:!?])/gi,
    (_all, n: string, unit: string) =>
      String(Math.round(Number(n.replace(",", ".")) * (UNITS[unit.toLowerCase()] ?? 1))),
  );
}

/** A bare family word becomes a full model id, so the price table finds it. */
const ALIASES: Array<[RegExp, string]> = [
  [/(?<![\w-])opus\b/gi, "claude-opus-5-5"],
  [/(?<![\w-])sonnet\b/gi, "claude-sonnet-5"],
  [/(?<![\w-])haiku\b/gi, "claude-haiku-4-5"],
];

export function expandModels(text: string): string {
  let out = text;
  for (const [re, id] of ALIASES) out = out.replace(re, id);
  return out;
}

/** What the cache hashes and the rules read. */
export function normalize(text: string): string {
  return expandModels(expandNumbers(collapse(text)));
}
