import { describe, expect, it } from "vitest";
import { deriveTaskState, statusLine, STATE_RANK, WORKING_MS, type TaskFacts, type TaskStatus } from "./taskstate";

const NOW = 1_000_000;
const ASKING = `
 Do you want to proceed?
 ❯ 1. Yes
   2. No, and tell Claude what to do differently (esc)
`;
const facts = (over: Partial<TaskFacts>): TaskFacts => ({
  running: true, lastOutputAt: NOW - 60_000, screen: "", changedFiles: 0, ...over,
});

describe("deriveTaskState", () => {
  it("stopped when the process is gone, whatever is on screen", () => {
    expect(deriveTaskState(facts({ running: false, screen: ASKING }), NOW).state).toBe("stopped");
  });

  it("merged once its branch went into the base and it has not started again", () => {
    expect(deriveTaskState(facts({ running: false, merged: true, changedFiles: null }), NOW).state).toBe("merged");
    // started again: it is working on something new
    expect(deriveTaskState(facts({ merged: true, lastOutputAt: NOW }), NOW).state).toBe("working");
    expect(STATE_RANK.stopped).toBeLessThan(STATE_RANK.merged);
    expect(statusLine("Ivy", { state: "merged", ask: null })).toBe("Ivy's work is merged");
  });

  it("needs you when a prompt is on screen, even right after output", () => {
    const s = deriveTaskState(facts({ screen: ASKING, lastOutputAt: NOW - 100 }), NOW);
    expect(s.state).toBe("needs");
    expect(s.ask?.options).toHaveLength(2);
  });

  it("working while output is recent", () => {
    expect(deriveTaskState(facts({ lastOutputAt: NOW - (WORKING_MS - 1) }), NOW).state).toBe("working");
  });

  it("ready to review when it went quiet with changes on its branch", () => {
    expect(deriveTaskState(facts({ changedFiles: 3 }), NOW).state).toBe("review");
  });

  it("idle when quiet with no changes, or when changes are unknown", () => {
    expect(deriveTaskState(facts({ changedFiles: 0 }), NOW).state).toBe("idle");
    expect(deriveTaskState(facts({ changedFiles: null }), NOW).state).toBe("idle");
  });

  it("ranks needs above review above working", () => {
    expect(STATE_RANK.needs).toBeLessThan(STATE_RANK.review);
    expect(STATE_RANK.review).toBeLessThan(STATE_RANK.working);
    expect(STATE_RANK.idle).toBeLessThan(STATE_RANK.stopped);
  });

  it("is blocked on a sign-in screen instead of idle", () => {
    const s = deriveTaskState(facts({ screen: "Please sign in to continue" }), NOW);
    expect(s.state).toBe("blocked");
    expect(s.blocked?.kind).toBe("auth");
  });

  it("is blocked even while the screen still redraws: a login page is not working", () => {
    const s = deriveTaskState(facts({ screen: "Please complete the CAPTCHA", lastOutputAt: NOW - 100 }), NOW);
    expect(s.state).toBe("blocked");
    expect(s.blocked?.kind).toBe("captcha");
  });

  it("an answerable prompt wins over a block notice", () => {
    const s = deriveTaskState(facts({ screen: "Please sign in to continue\n" + ASKING }), NOW);
    expect(s.state).toBe("needs");
  });

  it("ranks blocked with needs-you, above review", () => {
    expect(STATE_RANK.needs).toBeLessThan(STATE_RANK.blocked);
    expect(STATE_RANK.blocked).toBeLessThan(STATE_RANK.review);
  });
});

describe("statusLine", () => {
  it("says what the agent wants in plain words", () => {
    const run = deriveTaskState(facts({ screen: " Bash command\n\n   npm test\n\n" + ASKING }), NOW);
    expect(statusLine("Ana", run)).toBe("Ana wants to run npm test");
    expect(statusLine("Cy", deriveTaskState(facts({ changedFiles: 2 }), NOW))).toBe("Cy is done, changes ready to review");
    expect(statusLine("Hal", deriveTaskState(facts({}), NOW))).toBe("Hal is waiting for a task");
  });

  it("says what unblocks a blocked agent", () => {
    expect(statusLine("Ana", deriveTaskState(facts({ screen: "Please sign in to continue" }), NOW))).toBe("Ana is waiting on you to sign in");
    expect(statusLine("Bo", deriveTaskState(facts({ screen: "Complete the CAPTCHA" }), NOW))).toBe("Bo needs you to solve a CAPTCHA");
    expect(statusLine("Cy", deriveTaskState(facts({ screen: "Invalid API key" }), NOW))).toBe("Cy needs a valid API key");
  });

  it("idle after working is spent, and says so", () => {
    const s = deriveTaskState(facts({ worked: true }), NOW);
    expect(s.state).toBe("idle");
    expect(s.spent).toBe(true);
    expect(statusLine("Hal", s)).toBe("Hal is done, with no changes to review");
  });

  it("a working agent that is stuck says so", () => {
    const s: TaskStatus = { state: "working", ask: null, stuck: { kind: "stall", text: "no file changes in 11m across 8 steps", count: 8, sample: "stall", since: 1 } };
    expect(statusLine("Cy", s)).toBe("Cy looks stuck: no file changes in 11m across 8 steps");
  });

  it("flags a high-risk approval in plain words", () => {
    const run = deriveTaskState(facts({ screen: " Bash command\n\n   rm -rf /tmp/x\n\n" + ASKING }), NOW);
    expect(statusLine("Ana", run)).toBe("Ana wants to run rm -rf /tmp/x (high risk)");
  });
});
