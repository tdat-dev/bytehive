import { beforeEach, describe, expect, it, vi } from "vitest";

const eng = vi.hoisted(() => ({ view: null as unknown, messagePerson: vi.fn(async () => {}), reveal: [] as unknown[][], workspaces: new Map<string, unknown>(), boards: new Map<string, unknown>() }));
vi.mock("../../boardfile", () => ({
  readBoardFile: async (dir: string) => {
    const b = eng.boards.get(dir);
    if (b instanceof Error) throw b;
    return b ? { board: b } : null;
  },
}));
vi.mock("../../agentbridge", () => ({ revealPane: (...a: unknown[]) => { eng.reveal.push(a); return true; } }));
vi.mock("../../appstate", () => ({ activeWs: null, workspaces: eng.workspaces }));
vi.mock("../../tasks", () => ({ onTasksChange: () => () => {}, taskOf: () => undefined }));
vi.mock("../engine", () => ({
  companyView: () => eng.view, messagePerson: eng.messagePerson,
  companyAlerts: () => [], companyFlights: () => [], companyElsewhere: () => null, companyError: () => null,
  giveJob() {}, approve() {}, reject() {}, raiseCap() {}, finishJob() {}, stopJob() {}, dismissAlert() {}, wakePerson() {},
}));

import { cardsByJob, cardsOnBoard, liveSource } from "./live";

const person = (id: string, rank: string, floor: string) => ({ id, name: id, rank, floor });
const company = (people: unknown[]) => ({ company: { floors: [{ id: "a", dir: "D:/a", name: "alpha" }], people } });

describe("showPerson", () => {
  beforeEach(() => {
    eng.reveal = [];
    eng.workspaces.clear();
    eng.workspaces.set("w", { id: "w", panes: new Map([["p", { id: "p", spec: { personId: "hai" } }]]) });
  });

  it("takes the keyboard unless told not to", () => {
    expect(liveSource().showPerson("hai")).toBe(true);
    expect(liveSource().showPerson("hai", false)).toBe(true);
    expect(eng.reveal).toEqual([["w", "p", true], ["w", "p", false]]);
  });
  it("says false for someone with no pane", () => {
    expect(liveSource().showPerson("nobody", false)).toBe(false);
  });
});

describe("messageFloor", () => {
  beforeEach(() => { eng.messagePerson.mockClear(); });

  it("goes to the Lead", async () => {
    eng.view = company([person("sol", "staff", "a"), person("hai", "lead", "a")]);
    await liveSource().messageFloor("a", "Hello");
    expect(eng.messagePerson).toHaveBeenCalledWith("hai", "Hello");
  });
  it("says so when the floor has nobody to receive it", async () => {
    eng.view = company([]);
    await expect(liveSource().messageFloor("a", "Hello")).rejects.toThrow("Nobody on alpha");
    expect(eng.messagePerson).not.toHaveBeenCalled();
  });
  it("says so when the Company is not running here", async () => {
    eng.view = null;
    await expect(liveSource().messageFloor("a", "Hello")).rejects.toThrow("Nobody on this floor");
  });
});

const card = (job?: string) => ({ id: "c", title: "t", checklist: [], ...(job === undefined ? {} : { job }) });
const lists = (todo: unknown[], done: unknown[]) => ({ lists: [{ title: "To do", cards: todo }, { title: " Done ", cards: done }] });

describe("cards by job", () => {
  it("counts a job's cards, and the ones in the Done list as done", () => {
    const m = cardsOnBoard(lists([card("J-1"), card("J-2"), card()], [card("J-1")]).lists as never);
    expect(Object.fromEntries(m)).toEqual({ "J-1": { done: 1, total: 2 }, "J-2": { done: 0, total: 1 } });
  });
  it("adds the floors up", () => {
    const a = new Map([["J-1", { done: 1, total: 2 }]]);
    const b = new Map([["J-1", { done: 0, total: 3 }]]);
    expect(cardsByJob([a, b])).toEqual({ "J-1": { done: 1, total: 5 } });
  });
  it("takes a job named like an object's own keys as a job, and leaves every object alone", () => {
    const m = cardsOnBoard(lists([card("__proto__"), card("constructor")], [card("__proto__")]).lists as never);
    const all = cardsByJob([m]);
    expect(Object.getOwnPropertyNames(all).sort()).toEqual(["__proto__", "constructor"]);
    expect(Object.getOwnPropertyDescriptor(all, "__proto__")?.value).toEqual({ done: 1, total: 2 });
    expect(({} as Record<string, unknown>).total).toBeUndefined();
    expect(({} as Record<string, unknown>).done).toBeUndefined();
  });
});

describe("reading the boards", () => {
  beforeEach(() => { eng.boards.clear(); vi.useFakeTimers(); });

  it("forgets a floor's count once its board file is gone", async () => {
    eng.view = { company: { floors: [{ id: "a", dir: "D:/a", name: "alpha" }], people: [{ ...person("hai", "lead", "a"), role: "lead", boss: "founder", cli: "claude", look: { shirt: "#fff", hair: "#000", prop: "none" } }] }, jobs: [], roles: [] };
    eng.boards.set("D:/a", lists([card("J-1")], []));
    const src = liveSource();
    const read = async () => {
      src.watching(true);
      await vi.advanceTimersByTimeAsync(0);
      src.watching(false);
    };
    await read();
    eng.boards.delete("D:/a");
    await read();
    const job = { id: "J-1", title: "x", ask: "x", status: "running", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 0 };
    eng.view = { ...(eng.view as object), jobs: [job] };
    const m = src.snapshot(0).model!;
    expect(m.jobs[0].cards).toBeNull();
    expect(m.floors[0].board).toBeNull();
    vi.useRealTimers();
  });
  it("keeps a floor's last count when its board cannot be read this time", async () => {
    eng.view = { company: { floors: [{ id: "a", dir: "D:/a", name: "alpha" }], people: [{ ...person("hai", "lead", "a"), role: "lead", boss: "founder", cli: "claude", look: { shirt: "#fff", hair: "#000", prop: "none" } }] }, jobs: [], roles: [] };
    eng.boards.set("D:/a", lists([card("J-1")], [card("J-1")]));
    const src = liveSource();
    const seen: unknown[] = [];
    const read = async () => {
      src.watching(true);
      await vi.advanceTimersByTimeAsync(0);
      src.watching(false);
    };
    await read();
    eng.boards.set("D:/a", new Error("busy"));
    await read();
    const job = { id: "J-1", title: "x", ask: "x", status: "running", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 0 };
    eng.view = { ...(eng.view as object), jobs: [job] };
    seen.push(src.snapshot(0).model?.jobs[0].cards);
    expect(seen).toEqual([{ done: 1, total: 2 }]);
    vi.useRealTimers();
  });
});
