/** The zero-token pass: courtesy is blocked, a short answer goes straight to
 *  the agent, and a question that is only about cost is answered without the
 *  API. Anything else returns null and goes to the classifier. Pure. */
import { normalize } from "./normalize";
import type { Classified } from "./types";

const COURTESY = new Set([
  "thanks", "thank", "you", "thx", "ty", "hi", "hello", "hey", "yo", "bye", "good",
  "morning", "evening", "night", "great", "nice", "cool",
  "cảm", "cám", "ơn", "on", "nhé", "nha", "nhe", "ạ",
  "chào", "chao", "tạm", "biệt", "biet", "sáng", "sang", "tối", "toi",
  "chiều", "chieu", "lát", "gat", "gặp", "gap", "lại", "lai",
]);

/** Can answer a question put in plain text, which never sets the agent's ask
 *  status, or report finished work. Never blocked, whatever is around them. */
const ANSWERS = new Set([
  "ok", "oke", "okay", "okie", "okla", "yes", "yep", "yeah", "sure", "no", "nope", "done",
  "vâng", "dạ", "da", "ừ", "uh", "rồi", "roi",
]);

const wordsOf = (normalized: string): string[] =>
  normalized
    .normalize("NFC")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .split(/\s+/)
    .filter(Boolean);

/** Greeting, thanks and answer words only, in four words or fewer. */
function short(words: string[]): boolean {
  return words.length <= 4 && words.every((w) => COURTESY.has(w) || ANSWERS.has(w));
}

const COST = /(giá|chi phí|báo giá|bao nhiêu|cost|price|estimate)/i;
const MODEL = /(claude-[a-z0-9.-]+)/i;
const IN_TOKENS = /(\d{1,12})\s*(?:tokens?\s*)?(?:vào|vao|input|in)\b/i;
const OUT_TOKENS = /(\d{1,12})\s*(?:tokens?\s*)?(?:ra|output|out)\b/i;
/** Work to do, not a question: a task that mentions a price is still a task. */
const CODING =
  /(?<![\p{L}\p{N}])(sửa|viết|thêm|xoá|xóa|đổi|kiểm tra|hàm|lỗi|fix|write|add|remove|change|update|implement|refactor|debug|test|function|bug|code|commit|file)(?![\p{L}\p{N}])|\.(ts|tsx|js|rs|py|json|md|css|html)\b/iu;
const MAX_COST_QUESTION = 200;

const tokenCount = (m: RegExpMatchArray | null): number | null => {
  if (!m) return null;
  const n = Number(m[1]);
  return Number.isFinite(n) ? n : null;
};

/** A deterministic verdict, or null when the classifier has to look. */
export function rules(text: string): Classified | null {
  const n = normalize(text);
  if (!n) return { is_actionable: false, data: null };
  const words = wordsOf(n);
  if (short(words)) return { is_actionable: words.some((w) => ANSWERS.has(w)), data: null };
  if (n.length > MAX_COST_QUESTION || CODING.test(n.normalize("NFC"))) return null;
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
