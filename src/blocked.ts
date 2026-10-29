// An agent stuck on something only you can clear: a sign-in screen, a
// CAPTCHA, a missing or invalid API key. Only the last lines count, so an
// agent talking about logging in isn't taken for one stuck on it. Pure.

export type BlockedKind = "auth" | "captcha" | "key";

export interface BlockedHit {
  kind: BlockedKind;
  /** The line that said so, trimmed. */
  what: string;
}

/** Lines at the bottom of the screen looked at. */
const TAIL_LINES = 14;

const CAPTCHA = [/captcha/i, /verify (that )?you('| a)re (a )?human/i, /human verification/i];
const KEY = [
  /(invalid|missing|expired).*api key/i,
  /api key.*(invalid|missing|expired|not set|not configured|not found|required)/i,
  /set (your|the|an) \S*api[_ ]?key/i,
];
const AUTH = [
  /please (log|sign) in\b/i,
  /\b(sign|log) in to continue\b/i,
  /you (need to|must) (log|sign) in\b/i,
  /\bnot (logged|signed) in\b/i,
  /authentication (failed|expired|required)/i,
  /session expired/i,
  /\blog ?in required\b/i,
];

/** What blocks the agent's screen that only you can clear, or null. The
 *  bottom-most notice wins; a line matching several reads as the most
 *  specific (a CAPTCHA or key problem before a generic sign-in). */
export function blockedOnScreen(screen: string): BlockedHit | null {
  const lines = screen.split(/\r?\n/).map((l) => l.replace(/[│┃|╭╮╰╯─━■⎿●∙•·]+/g, " ").replace(/\s+/g, " ").trim()).filter(Boolean).slice(-TAIL_LINES);
  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i];
    const kind = CAPTCHA.some((re) => re.test(line)) ? "captcha" : KEY.some((re) => re.test(line)) ? "key" : AUTH.some((re) => re.test(line)) ? "auth" : null;
    if (kind) return { kind, what: line.slice(0, 160) };
  }
  return null;
}
