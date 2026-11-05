// src/company/watch.test.ts
import { describe, it, expect } from "vitest";
import { watchBudget, watchWork, type WatchedCard } from "./watch";
import { DEFAULT_LIMITS, type Job } from "./types";

const MIN = 60_000;
const card = (over: Partial<WatchedCard>): WatchedCard => ({ id: "c1", title: "Login", job: "J-1", assigneeId: "d1", quietSince: null, returns: 0, ...over });
const run = (cards: WatchedCard[], nudges = new Map<string, number>(), now = 100 * MIN) =>
  watchWork({ cards, nudges, limits: DEFAULT_LIMITS, bossOf: () => "l1", now });

describe("watchWork", () => {
  it("leaves a working person alone", () => {
    expect(run([card({})]).effects).toEqual([]);
  });
  it("leaves someone quiet for under 10 minutes alone", () => {
    expect(run([card({ quietSince: 91 * MIN })]).effects).toEqual([]);
  });
  it("nudges the boss the first time", () => {
    const r = run([card({ quietSince: 80 * MIN })]);
    expect(r.effects).toEqual([{ kind: "deliver", personId: "l1", text: expect.stringContaining("Login") }]);
    expect(r.alerts).toEqual([]);
    expect(r.nudges.get("c1")).toBe(1);
  });
  it("does not nudge again in the same quiet stretch until another 10 minutes pass", () => {
    const first = run([card({ quietSince: 85 * MIN })]);
    expect(run([card({ quietSince: 85 * MIN })], first.nudges, 101 * MIN).effects).toEqual([]);
  });
  it("asks you the second time", () => {
    const r = run([card({ quietSince: 70 * MIN })], new Map([["c1", 1]]));
    expect(r.alerts[0]).toMatchObject({ job: "J-1", personId: "d1", level: "warn" });
  });
  it("forgets the nudges of a card that started moving", () => {
    expect(run([card({})], new Map([["c1", 1]])).nudges.has("c1")).toBe(false);
  });
  it("stops a card returned 3 times", () => {
    const r = run([card({ returns: 3 })]);
    expect(r.alerts[0].text).toContain("3");
  });
});

const job = (used: number, cap = 1000): Job => ({ id: "J-1", title: "t", ask: "a", status: "running", plan: null, tokenCap: cap, tokensUsed: used, tokenBase: {}, people: [], createdAt: 1 });

describe("watchBudget", () => {
  it("is quiet under 80%", () => {
    expect(watchBudget(job(799))).toEqual({ alert: null, pause: false });
  });
  it("warns at 80%", () => {
    expect(watchBudget(job(800))).toMatchObject({ alert: { level: "warn" }, pause: false });
  });
  it("pauses at 100%", () => {
    expect(watchBudget(job(1000))).toMatchObject({ alert: { level: "stop" }, pause: true });
  });
  it("ignores a job that is not running", () => {
    expect(watchBudget({ ...job(5000), status: "paused" }).pause).toBe(false);
  });
});
