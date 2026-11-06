// src/company/watch.test.ts
import { describe, it, expect } from "vitest";
import type { Card } from "../board";
import { cardsToWatch, watchBudget, watchWork, type WatchedCard } from "./watch";
import { DEFAULT_LIMITS, type Job, type Limits } from "./types";

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

describe("cardsToWatch", () => {
  const running = [{ id: "J1", status: "running" }] as Job[];
  const card = (id: string, over: object = {}) => ({ id, title: id, job: "J1", ...over }) as Card;
  const board = (todo: Card[], doing: Card[] = []) => ({ lists: [{ title: "To do", cards: todo }, { title: "Doing", cards: doing }] });
  const base = { jobs: running, contact: "lead", isWorking: () => false, now: 100 };

  it("watches a Job's To do card through the floor's contact", () => {
    const quiet = new Map<string, number>();
    const r = cardsToWatch({ ...base, quiet, board: board([card("c1")]) });
    expect(r.cards).toEqual([{ id: "c1", title: "c1", job: "J1", assigneeId: "lead", quietSince: 100, returns: 0, list: "todo" }]);
  });
  it("uses the person a To do card names", () => {
    const r = cardsToWatch({ ...base, quiet: new Map(), board: board([card("c1", { assigneeId: "ana" })]) });
    expect(r.cards[0].assigneeId).toBe("ana");
  });
  it("leaves To do alone while a card of the same Job is in Doing", () => {
    const quiet = new Map([["todo:c1", 5]]);
    const r = cardsToWatch({ ...base, quiet, board: board([card("c1")], [card("c2", { assigneeId: "ana" })]) });
    expect(r.cards.map((c) => c.id)).toEqual(["c2"]);
    expect(quiet.has("todo:c1")).toBe(false);
  });
  it("starts a card's quiet time over when it moves to Doing", () => {
    const quiet = new Map([["todo:c1", 5]]);
    const r = cardsToWatch({ ...base, now: 200, quiet, board: board([], [card("c1", { assigneeId: "ana" })]) });
    expect(r.cards[0].quietSince).toBe(200);
  });
  it("clears a Doing card's quiet time while its person works", () => {
    const quiet = new Map([["doing:c1", 5]]);
    const r = cardsToWatch({ ...base, isWorking: () => true, quiet, board: board([], [card("c1", { assigneeId: "ana" })]) });
    expect(r.cards[0].quietSince).toBeNull();
  });
  it("ignores a paused Job, cards with no Job, and a To do card with nobody to ask", () => {
    const quiet = new Map<string, number>();
    const paused = [{ id: "J1", status: "paused" }] as Job[];
    expect(cardsToWatch({ ...base, jobs: paused, quiet, board: board([card("c1")]) }).cards).toEqual([]);
    expect(cardsToWatch({ ...base, quiet, board: board([card("c1", { job: undefined })]) }).cards).toEqual([]);
    expect(cardsToWatch({ ...base, contact: null, quiet, board: board([card("c1")]) }).cards).toEqual([]);
  });
  it("names every card it read, for clearing old quiet times", () => {
    expect(cardsToWatch({ ...base, quiet: new Map(), board: board([card("c1")], [card("c2")]) }).seen).toEqual(["doing:c2", "todo:c1"]);
  });
});

describe("watchWork on a To do card", () => {
  const limits = { stallMinutes: 10, maxReturns: 3 } as Limits;
  const c = { id: "c1", title: "Fix it", job: "J1", assigneeId: "lead", quietSince: 0, returns: 0, list: "todo" as const };
  it("nudges the boss in its own words, then raises a row", () => {
    const first = watchWork({ cards: [c], nudges: new Map(), limits, bossOf: () => "founder", now: 600_000 });
    expect(first.effects[0]).toEqual({ kind: "deliver", personId: "founder", text: '"Fix it" has waited in To do for 10 minutes. Hand it out or start it.' });
    const second = watchWork({ cards: [c], nudges: first.nudges, limits, bossOf: () => "founder", now: 1_200_000 });
    expect(second.alerts[0].text).toBe('"Fix it" has not been picked up.');
  });
});
