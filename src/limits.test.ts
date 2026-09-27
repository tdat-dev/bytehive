import { describe, expect, it } from "vitest";
import { durationMs, limitOnScreen, parseReset } from "./limits";

// Saturday 27 Sep 2026, 14:00 local time
const NOW = new Date(2026, 8, 27, 14, 0, 0).getTime();
const at = (y: number, mo: number, d: number, h: number, mi = 0) => new Date(y, mo, d, h, mi, 0).getTime();

describe("when a usage limit lifts", () => {
  it("reads a wait", () => {
    expect(durationMs("2 hours 13 minutes")).toBe(2 * 3600e3 + 13 * 60e3);
    expect(parseReset("in 2 hours 13 minutes.", NOW)).toBe(NOW + 2 * 3600e3 + 13 * 60e3);
    expect(parseReset("in 3h 12m", NOW)).toBe(NOW + 3 * 3600e3 + 12 * 60e3);
  });

  it("reads a time as the next one", () => {
    expect(parseReset("7pm (Asia/Saigon)", NOW)).toBe(at(2026, 8, 27, 19));
    expect(parseReset("at 7:30 pm", NOW)).toBe(at(2026, 8, 27, 19, 30));
    // 1am has passed today: tomorrow's
    expect(parseReset("1am", NOW)).toBe(at(2026, 8, 28, 1));
    expect(parseReset("12:10am", NOW)).toBe(at(2026, 8, 28, 0, 10));
  });

  it("reads a day and a time", () => {
    expect(parseReset("Thu 1am", NOW)).toBe(at(2026, 9, 1, 1));
    expect(parseReset("Oct 2, 1am", NOW)).toBe(at(2026, 9, 2, 1));
    expect(parseReset("October 2 at 5:00 PM", NOW)).toBe(at(2026, 9, 2, 17));
    expect(parseReset("soon", NOW)).toBeNull();
    expect(parseReset("42", NOW)).toBeNull();
  });
});

describe("an agent that hit its usage limit", () => {
  it("is seen in Claude Code's words, with when it lifts", () => {
    const screen = "● Done with the header.\n\n  ⎿  You've hit your limit · resets 7pm (Asia/Saigon)\n     /upgrade to increase your usage limit.\n\n╭────────╮\n│ >      │\n╰────────╯\n  ? for shortcuts";
    const hit = limitOnScreen(screen, NOW)!;
    expect(hit.what).toContain("You've hit your limit");
    expect(hit.resetsAt).toBe(at(2026, 8, 27, 19));
    expect(limitOnScreen("5-hour limit reached ∙ resets 7pm\n> ", NOW)!.resetsAt).toBe(at(2026, 8, 27, 19));
    expect(limitOnScreen("You've hit your weekly limit · resets Oct 2, 1am", NOW)!.resetsAt).toBe(at(2026, 9, 2, 1));
  });

  it("is seen in Codex's words", () => {
    const hit = limitOnScreen("■ You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again in 2 hours 13 minutes.\n\n› ", NOW)!;
    expect(hit.resetsAt).toBe(NOW + 2 * 3600e3 + 13 * 60e3);
  });

  it("is seen in Gemini's words, even without a time", () => {
    expect(limitOnScreen("✕ You have exhausted your daily quota on this model.\n> ", NOW)).toMatchObject({ resetsAt: null });
  });

  it("isn't taken from far up the screen, or from talk about limits", () => {
    const old = ["You've hit your limit · resets 7pm", ...Array.from({ length: 20 }, (_, i) => `line ${i}`)].join("\n");
    expect(limitOnScreen(old, NOW)).toBeNull();
    expect(limitOnScreen("The API returns 429 when the rate limit is exceeded; we retry with backoff.\n> ", NOW)).toBeNull();
  });
});
