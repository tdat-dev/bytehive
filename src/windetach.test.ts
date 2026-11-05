// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ opened: [] as string[], notes: [] as string[], dropped: [] as string[] }));
vi.mock("./ipc", () => ({
  openDetachWindow: async (key: string) => { io.opened.push(key); },
  destroyWindow: async () => {},
  onAppEvent: async () => () => {},
  emitAppEvent: async () => {},
}));
vi.mock("./workspace", () => ({ dropWorkspace: (ws: { id: string }) => { io.dropped.push(ws.id); } }));
vi.mock("./hint", () => ({ topNote: (msg: string) => { io.notes.push(msg); } }));

import { detachWorkspace } from "./windetach";
import type { Workspace } from "./panetypes";

const pane = (id: string, personId?: string) => ({ id, spec: { name: id, personId }, running: true, spawnedAt: 1, term: { dispose: () => {} }, el: { remove: () => {} } });
const project = (...panes: ReturnType<typeof pane>[]) =>
  ({ id: "w1", name: "shop", dir: "D:\\shop", repoRoot: null, isolated: false, panes: new Map(panes.map((p) => [p.id, p])) }) as unknown as Workspace;

beforeEach(() => {
  io.opened = [];
  io.notes = [];
  io.dropped = [];
  localStorage.clear();
});

describe("detachWorkspace", () => {
  it("refuses a tab that holds a member of the company, and says why", async () => {
    const ws = project(pane("a"), pane("b", "p1"));
    await detachWorkspace(ws);
    expect(io.opened).toEqual([]);
    expect(io.dropped).toEqual([]);
    expect(ws.panes.size).toBe(2);
    expect(localStorage.length).toBe(0);
    expect(io.notes).toEqual([expect.stringContaining("main window")]);
  });

  it("still moves a tab with nobody from the company in it", async () => {
    await detachWorkspace(project(pane("a")));
    expect(io.opened).toHaveLength(1);
    expect(io.dropped).toEqual(["w1"]);
    expect(io.notes).toEqual([]);
  });
});
