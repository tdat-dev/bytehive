import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({
  tasks: [] as unknown[],
  notes: [] as Array<{ title: string; body: string; open: string }>,
  prefs: { notifyDone: true } as Record<string, boolean>,
  panes: new Map<string, unknown>(),
  stops: new Map<string, number>(),
}));
vi.mock("./tasks", () => ({ allTasks: () => io.tasks }));
vi.mock("./ipc", () => ({
  notifyAgent: async (title: string, body: string, open: string) => { io.notes.push({ title, body, open }); },
  onNotificationOpen: async () => () => {},
  stopAskedAt: (id: string) => io.stops.get(id) ?? null,
}));
vi.mock("./prefs", () => ({ getPref: (k: string) => io.prefs[k] }));
vi.mock("./appstate", () => ({ workspaces: new Map([["w1", { id: "w1", panes: io.panes }]]) }));

import { checkFinishes, lastFinish, startFinishWatch, stopFinishWatch } from "./finishwatch";

const task = (id: string, name: string, state: string, over: object = {}) =>
  ({ paneId: id, wsId: "w1", name, project: "shop", status: { state, ask: null, limit: null }, changedFiles: null, ...over });

/** Agents a and b work for `run` s and go idle at the same time; play `total` s. */
function play(states: (t: number) => unknown[], total: number, t0 = 0): void {
  for (let t = 0; t <= total; t++) {
    io.tasks = states(t);
    checkFinishes(t0 + t * 1000);
  }
}

describe("finish notifications", () => {
  let screen: string | null = null;
  beforeEach(() => {
    io.notes = []; io.tasks = []; io.prefs = { notifyDone: true }; io.panes.clear(); io.stops.clear();
    stopFinishWatch();
    screen = null;
    startFinishWatch({ screen: () => screen, open: () => {} });
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
  });

  it("sends one notification for agents that finish together", () => {
    play((t) => [task("a", "Ana", t < 30 ? "working" : "idle"), task("b", "Bo", t < 32 ? "working" : "review", { changedFiles: 2 })], 50);
    expect(io.notes).toHaveLength(1);
    expect(io.notes[0].title).toBe("2 agents finished");
    expect(io.notes[0].open).toBe("b"); // changes to review come first
    expect(lastFinish("a")?.kind).toBe("done");
  });

  it("stays quiet about the agent on screen while Maestro has focus", () => {
    screen = "a";
    play((t) => [task("a", "Ana", t < 30 ? "working" : "idle")], 50);
    expect(io.notes).toEqual([]);
  });

  it("tells about the agent on screen when Maestro is in the background", () => {
    screen = "a";
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    play((t) => [task("a", "Ana", t < 30 ? "working" : "idle")], 50);
    expect(io.notes.map((n) => n.title)).toEqual(["Ana is done"]);
  });

  it("says nothing when the setting is off, but the dashboard still knows", () => {
    io.prefs.notifyDone = false;
    play((t) => [task("a", "Ana", t < 30 ? "working" : "idle")], 50);
    expect(io.notes).toEqual([]);
    expect(lastFinish("a")?.kind).toBe("done");
  });

  it("tells a crash, not a stop you asked for", () => {
    io.panes.set("a", { id: "a", exitCode: 1 });
    io.panes.set("b", { id: "b", exitCode: 1 });
    io.stops.set("b", 29_000);
    play((t) => [task("a", "Ana", t < 30 ? "working" : "stopped"), task("b", "Bo", t < 30 ? "working" : "stopped")], 50);
    expect(io.notes.map((n) => n.title)).toEqual(["Ana stopped with an error"]);
  });
});
