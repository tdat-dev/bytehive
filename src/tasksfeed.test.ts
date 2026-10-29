import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./ipc", () => ({
  gitChangedFiles: vi.fn(async () => []),
  sendInput: vi.fn(async () => {}),
}));
vi.mock("./gridcards", () => ({
  FEED_READ_MS: 2000,
  refreshGlance: vi.fn(async () => false),
  itemsFor: () => [],
  forgetFeed: vi.fn(),
}));
vi.mock("./agentcost", () => ({ forgetCost: vi.fn() }));
vi.mock("./chatsource", () => ({ forgetConversation: vi.fn() }));

import { forgetCost } from "./agentcost";
import { workspaces } from "./appstate";
import { forgetConversation } from "./chatsource";
import { FEED_READ_MS, forgetFeed, refreshGlance } from "./gridcards";
import { updateTasks } from "./tasks";
import type { Pane, Workspace } from "./panetypes";

function pane(id: string, over: Record<string, unknown> = {}): Pane {
  return {
    id, el: {} as HTMLElement, running: true, spawnedAt: 1000, lastOutputAt: 0, lastInputAt: 0,
    attention: false, attentionClearedAt: 0, attentionNotified: false, color: "#fff",
    spec: { program: "claude", args: [], cwd: "D:\\p", name: id, badge: "claude", color: "#fff", mono: "C" },
    term: { snapshot: () => "" } as unknown as Pane["term"],
    ...over,
  } as Pane;
}

function workspace(panes: Pane[]): Workspace {
  return { id: "ws-1", name: "maestro", dir: "D:\\p", repoRoot: "D:\\p", isolated: true, gridEl: {} as HTMLElement, panes: new Map(panes.map((p) => [p.id, p])), layout: new Map() };
}

const readIds = () => vi.mocked(refreshGlance).mock.calls.map((c) => (c[0] as Pane).id);

beforeEach(() => {
  workspaces.clear();
  vi.mocked(refreshGlance).mockClear();
  vi.mocked(forgetFeed).mockClear();
  updateTasks(0); // drop tasks left from the previous test
});

describe("transcript feeds on the task tick", () => {
  it("reads running panes that have spawned, skips the rest", () => {
    workspaces.set("ws-1", workspace([pane("a"), pane("b", { running: false }), pane("c", { spawnedAt: 0 })]));
    updateTasks(10_000);
    expect(readIds()).toEqual(["a"]);
  });

  it("re-reads on the shared cadence", () => {
    workspaces.set("ws-1", workspace([pane("a")]));
    updateTasks(10_000);
    updateTasks(10_000 + FEED_READ_MS - 1);
    expect(readIds()).toEqual(["a"]);
    updateTasks(10_000 + FEED_READ_MS);
    expect(readIds()).toEqual(["a", "a"]);
  });

  it("forgets feeds of closed panes", () => {
    workspaces.set("ws-1", workspace([pane("a")]));
    updateTasks(10_000);
    workspaces.clear();
    updateTasks(11_000);
    expect(vi.mocked(forgetFeed)).toHaveBeenCalledWith("a");
  });

  it("forgets cost and conversation caches of closed panes", () => {
    workspaces.set("ws-1", workspace([pane("a")]));
    updateTasks(10_000);
    workspaces.clear();
    updateTasks(11_000);
    expect(vi.mocked(forgetCost)).toHaveBeenCalledWith("a");
    expect(vi.mocked(forgetConversation)).toHaveBeenCalledWith("a");
  });
});
