// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ answer: null as unknown, fail: "", codex: null as unknown }));
vi.mock("./ipc", () => ({
  claudePlanUsage: async () => { if (io.fail) throw new Error(io.fail); return io.answer; },
  codexPlanUsage: async () => io.codex,
}));

import { allReset, planHtml, planRows, resetWords, roomNote, startPlanMeter, stopPlanMeter } from "./planmeter";

const NOW = Date.parse("2026-09-27T08:57:00Z");
const limit = (kind: string, percent: number, resets_at: string | null, model: string | null = null, severity = "normal") => ({ kind, percent, resets_at, severity, model });

describe("the plan's limits", () => {
  it("says when a limit resets: soon as a countdown, later as a day and time", () => {
    expect(resetWords("2026-09-27T12:09:59Z", NOW).short).toBe("in 3h 13m");
    expect(resetWords("2026-09-27T09:30:00Z", NOW).short).toBe("in 33m");
    expect(resetWords("2026-10-02T17:59:59Z", NOW).short).not.toMatch(/^in /);
    expect(resetWords(null, NOW).short).toBe("");
  });

  it("shows the 5 hours, then the week, then a model's week only once it is used", () => {
    const rows = planRows({ plan: "max", limits: [
      limit("weekly_all", 64.4, "2026-10-02T17:59:59Z"),
      limit("weekly_scoped", 0, null, "Fable"),
      limit("session", 92, "2026-09-27T12:09:59Z"),
      limit("weekly_scoped", 12, null, "Opus"),
    ] }, NOW);
    expect(rows.map((r) => [r.label, r.percent, r.level])).toEqual([
      ["5 hours", 92, "full"],
      ["This week", 64, "ok"],
      ["Opus, this week", 12, "ok"],
    ]);
    expect(planRows({ plan: null, limits: [limit("weekly_all", 10, null, null, "warning")] }, NOW)[0].level).toBe("high");
  });

  it("asks Anthropic, draws the limits, and draws nothing without a Claude sign-in", async () => {
    io.answer = { plan: "max", limits: [limit("session", 2, "2026-09-27T12:09:59Z"), limit("weekly_all", 4, "2026-10-02T17:59:59Z")] };
    io.fail = "";
    let drawn = 0;
    startPlanMeter(() => { drawn++; }, 0);
    await new Promise((r) => setTimeout(r, 5));
    await new Promise((r) => setTimeout(r, 0));
    expect(drawn).toBe(1);
    const el = document.createElement("div");
    el.innerHTML = planHtml(NOW);
    expect(el.querySelector(".pl-h")!.textContent).toBe("Claude Max limits");
    // folded, the line still says how full each plan is
    expect(el.querySelector("details.iq-plan")!.hasAttribute("open")).toBe(false);
    expect(el.querySelector(".pl-sum")!.textContent).toMatch(/^Limits.*Claude \d+%/);
    expect([...el.querySelectorAll(".pl-row")].map((r) => `${r.querySelector(".pl-l")!.textContent} ${r.querySelector(".pl-p")!.textContent}`)).toEqual(["5 hours 2%", "This week 4%"]);
    stopPlanMeter();
    // an API key user: nothing to show
    io.fail = "not signed in";
    startPlanMeter(() => {}, 0);
    await new Promise((r) => setTimeout(r, 5));
    await new Promise((r) => setTimeout(r, 0));
    expect(planHtml(NOW)).toBe("");
    stopPlanMeter();
  });

  it("shows Codex's limits from its last run beside Claude's, and drops them once they have all reset", async () => {
    io.fail = "not signed in";
    const soon = NOW + 3600_000;
    io.codex = { provider: "Codex", plan: "plus", as_of_ms: NOW - 60_000, limits: [
      { kind: "session", percent: 40, resets_at: null, resets_ms: soon, severity: "normal", model: null },
      { kind: "weekly_all", percent: 80, resets_at: null, resets_ms: NOW - 1000, severity: "normal", model: null },
    ] };
    startPlanMeter(() => {}, 0);
    await new Promise((r) => setTimeout(r, 5));
    await new Promise((r) => setTimeout(r, 0));
    const el = document.createElement("div");
    el.innerHTML = planHtml(NOW);
    expect(el.querySelector(".pl-h")!.textContent).toBe("Codex Plus limits");
    // the week has reset since: it starts from nothing
    expect([...el.querySelectorAll(".pl-row")].map((r) => r.querySelector(".pl-p")!.textContent)).toEqual(["40%", "0%"]);
    expect(allReset(io.codex as never, soon + 1)).toBe(true);
    expect(planHtml(soon + 1)).toBe("");
    stopPlanMeter();
    io.codex = null;
  });

  it("tells a new agent how little room its CLI's plan has, and which one has room", async () => {
    const soon = Date.now() + 2 * 3600_000;
    io.fail = "";
    io.answer = { provider: "Claude", plan: "max", limits: [{ kind: "session", percent: 93, resets_at: new Date(soon).toISOString(), severity: "normal", model: null }] };
    io.codex = { provider: "Codex", plan: "plus", as_of_ms: Date.now(), limits: [{ kind: "session", percent: 20, resets_at: null, resets_ms: soon, severity: "normal", model: null }] };
    startPlanMeter(() => {}, 0);
    await new Promise((r) => setTimeout(r, 5));
    await new Promise((r) => setTimeout(r, 0));
    const clis = [{ badge: "claude", label: "Claude Code" }, { badge: "codex", label: "Codex" }];
    expect(roomNote("claude", clis)).toMatch(/^Claude has used 93% of its 5-hour limit · resets .+\. Codex has room\.$/);
    expect(roomNote("codex", clis)).toBe("");
    stopPlanMeter();
    io.codex = null;
  });
});
