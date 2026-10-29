import { describe, expect, it } from "vitest";
import { LOOP_COUNT, LOOP_WINDOW, STALL_MS, STALL_STEPS, StuckWatch, loopOf, stallSteps } from "./stuckwatch";
import type { ChatItem, StepItem } from "./chatmodel";

let n = 0;
const step = (over: Partial<StepItem> = {}): StepItem => ({
  kind: "step", id: `s${++n}`, tool: "Bash", verb: "Ran", target: "npm test", done: true, at: 1000 + n * 1000, ...over,
});
const user = (text = "fix it"): ChatItem => ({ kind: "user", id: `u${++n}`, text, images: 0, at: 500 });
const fail = (output: string, over: Partial<StepItem> = {}): StepItem =>
  step({ error: true, output, ...over });

describe("loopOf", () => {
  it(`flags ${LOOP_COUNT} same failures in the last ${LOOP_WINDOW} steps`, () => {
    const items: ChatItem[] = [user(), fail("FAIL src/app.test.ts"), fail("FAIL src/app.test.ts"), fail("FAIL src/app.test.ts")];
    expect(loopOf(items)).toMatchObject({ count: 3, sample: "FAIL src/app.test.ts" });
  });

  it("ignores different failures", () => {
    const items: ChatItem[] = [user(), fail("FAIL a"), fail("FAIL b"), fail("FAIL c")];
    expect(loopOf(items)).toBeNull();
  });

  it("ignores repeats that did not fail", () => {
    const items: ChatItem[] = [user(), step(), step(), step(), step(), step()];
    expect(loopOf(items)).toBeNull();
  });

  it("needs the full count", () => {
    const items: ChatItem[] = [user(), fail("FAIL x"), fail("FAIL x")];
    expect(loopOf(items)).toBeNull();
  });

  it("restarts counting after you speak", () => {
    const items: ChatItem[] = [fail("FAIL x"), fail("FAIL x"), fail("FAIL x"), user("try another way"), fail("FAIL x")];
    expect(loopOf(items)).toBeNull();
  });

  it("counts only finished steps", () => {
    const items: ChatItem[] = [user(), fail("FAIL x"), fail("FAIL x"), fail("FAIL x", { done: false })];
    expect(loopOf(items)).toBeNull();
  });

  it("normalizes numbers in the error line", () => {
    const items: ChatItem[] = [user(), fail("failed after 3 attempts"), fail("failed after 4 attempts"), fail("failed after 5 attempts")];
    expect(loopOf(items)?.count).toBe(3);
  });

  it("looks only at the recent window", () => {
    const old = [fail("FAIL old"), fail("FAIL old"), fail("FAIL old")];
    const fresh = Array.from({ length: LOOP_WINDOW }, (_, i) => step({ target: `cmd-${i}` }));
    expect(loopOf([user(), ...old, ...fresh])).toBeNull();
  });
});

describe("stallSteps", () => {
  const reads = (k: number) => Array.from({ length: k }, (_, i) => step({ tool: "Read", verb: "Read", target: `file-${i}.ts` }));

  it(`keeps ${STALL_STEPS} finished steps that touched no files`, () => {
    expect(stallSteps([user(), ...reads(STALL_STEPS)]))?.toHaveLength(STALL_STEPS);
  });

  it("an open step means something is still running, not stalled", () => {
    const items: ChatItem[] = [user(), ...reads(STALL_STEPS), step({ done: false })];
    expect(stallSteps(items)).toBeNull();
  });

  it("a step that changed a file breaks the stall", () => {
    const items: ChatItem[] = [user(), ...reads(STALL_STEPS - 1), step({ verb: "Edited", target: "app.ts", added: 3, removed: 1 })];
    expect(stallSteps(items)).toBeNull();
  });

  it("an edit verb counts as touching files even without counts", () => {
    const items: ChatItem[] = [user(), ...reads(STALL_STEPS - 1), step({ verb: "Wrote", target: "new.ts" })];
    expect(stallSteps(items)).toBeNull();
  });

  it("needs the full run after you speak", () => {
    const items: ChatItem[] = [...reads(STALL_STEPS), user("go on"), ...reads(2)];
    expect(stallSteps(items)).toBeNull();
  });
});

describe("StuckWatch", () => {
  const snap = (over: Record<string, unknown> = {}) => ({
    run: 1, state: "working", items: [] as ChatItem[], diff: { n: 0, add: 0, del: 0 }, now: 0, ...over,
  });
  const looped = (): ChatItem[] => [user(), fail("FAIL x"), fail("FAIL x"), fail("FAIL x")];
  const stalled = (): ChatItem[] =>
    [user(), ...Array.from({ length: STALL_STEPS }, (_, i) => step({ tool: "Read", verb: "Read", target: `f-${i}` }))];

  it("reports a loop with evidence", () => {
    const w = new StuckWatch();
    const e = w.step("a", snap({ items: looped() }));
    expect(e).toMatchObject({ kind: "loop", count: 3 });
    expect(e?.text).toContain("3 times");
    expect(e?.text).toContain("FAIL x");
  });

  it("stays quiet when the agent is not working", () => {
    const w = new StuckWatch();
    expect(w.step("a", snap({ state: "idle", items: looped() }))).toBeNull();
  });

  it("reports a stall after the diff sits still long enough", () => {
    const w = new StuckWatch();
    expect(w.step("a", snap({ items: stalled(), now: 0 }))).toBeNull();
    const e = w.step("a", snap({ items: stalled(), now: STALL_MS + 60_000 }));
    expect(e).toMatchObject({ kind: "stall", count: STALL_STEPS });
    expect(e?.text).toContain("11m");
  });

  it("a moving diff resets the stall clock", () => {
    const w = new StuckWatch();
    w.step("a", snap({ items: stalled(), now: 0 }));
    w.step("a", snap({ items: stalled(), now: STALL_MS + 60_000, diff: { n: 1, add: 5, del: 0 } }));
    expect(w.step("a", snap({ items: stalled(), now: STALL_MS + 120_000, diff: { n: 1, add: 5, del: 0 } }))).toBeNull();
  });

  it("an unknown diff cannot stall", () => {
    const w = new StuckWatch();
    w.step("a", snap({ items: stalled(), now: 0, diff: null }));
    expect(w.step("a", snap({ items: stalled(), now: STALL_MS + 60_000, diff: null }))).toBeNull();
  });

  it("a restart resets the stall clock", () => {
    const w = new StuckWatch();
    w.step("a", snap({ items: stalled(), now: 0 }));
    expect(w.step("a", snap({ items: stalled(), now: STALL_MS + 60_000, run: 2 }))).toBeNull();
  });

  it("sees nothing without a conversation", () => {
    const w = new StuckWatch();
    expect(w.step("a", snap({ now: STALL_MS + 60_000 }))).toBeNull();
  });
});
