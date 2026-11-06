/** The zero-token pass: messages that are clearly small talk are blocked, and
 *  a cost question with a model and two token counts is answered without the
 *  API. Anything else returns null and goes to the classifier. Pure. */
import { normalize } from "./normalize";
import type { Classified } from "./types";

const SMALL = new Set([
  "ok", "oke", "okay", "okie", "okla", "yes", "yep", "yeah", "sure", "no", "nope",
  "thanks", "thank", "you", "thx", "ty", "hi", "hello", "hey", "yo", "bye", "good",
  "morning", "evening", "night", "great", "nice", "cool", "done",
  "cảm", "cám", "ơn", "on", "nhé", "nha", "nhe", "ạ", "vâng", "dạ", "da", "ừ",
  "uh", "chào", "chao", "tạm", "biệt", "biet", "sáng", "sang", "tối", "toi",
  "chiều", "chieu", "lát", "gat", "gặp", "gap", "lại", "lai", "rồi", "roi",
]);

/** Only a greeting, thanks or acknowledgement, in four words or fewer. */
export function smallTalk(normalized: string): boolean {
  const words = normalized
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .split(/\s+/)
    .filter(Boolean);
  if (words.length === 0) return true;
  return words.length <= 4 && words.every((w) => SMALL.has(w));
}

const COST = /(giá|chi phí|báo giá|bao nhiêu|cost|price|estimate)/i;
const MODEL = /(claude-[a-z0-9.-]+)/i;
const IN_TOKENS = /(\d{1,12})\s*(?:tokens?\s*)?(?:vào|vao|input|in)\b/i;
const OUT_TOKENS = /(\d{1,12})\s*(?:tokens?\s*)?(?:ra|output|out)\b/i;

const tokenCount = (m: RegExpMatchArray | null): number | null => {
  if (!m) return null;
  const n = Number(m[1]);
  return Number.isFinite(n) ? n : null;
};

/** A deterministic answer when the request is only "what does this cost". */
export function rules(text: string): Classified | null {
  const n = normalize(text);
  if (!n) return { is_actionable: false, data: null };
  if (smallTalk(n)) return { is_actionable: false, data: null };
  const model = n.match(MODEL)?.[1]?.replace(/[.,;:]+$/, "");
  const inputTokens = tokenCount(n.match(IN_TOKENS));
  const outputTokens = tokenCount(n.match(OUT_TOKENS));
  if (model && inputTokens !== null && outputTokens !== null && COST.test(n)) {
    return {
      is_actionable: true,
      data: { intent: "estimate_cost", model, inputTokens, outputTokens },
    };
  }
  return null;
}
