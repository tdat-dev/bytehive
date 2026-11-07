// @vitest-environment happy-dom
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { Pane, Workspace } from "./panetypes";

type Bridge = {
  workspaces: () => Array<{ agents: Array<{ name: string }> }>;
  deliver: (dir: string, from: string | null, to: string | null, message: string) => void;
};

const io = vi.hoisted(() => ({
  sent: [] as string[],
  bridge: null as unknown,
  fleet: null as null | (() => Array<{ name: string }>),
  reveal: null as null | ((ws: string, pane: string, focus?: boolean) => boolean),
  staged: [] as unknown[],
}));

vi.mock("./ipc", () => ({
  sendMessage: async (id: string) => { io.sent.push(id); },
  setPaneScreenReader: () => {},
  setPaneBracketedPasteReader: () => {},
  onDragDrop: async () => {},
  onExit: async () => {},
}));
vi.mock("./panelayout", () => ({ focusPane: (_ws: unknown, _p: unknown, _ev: unknown, take = true) => { io.staged.push(take); } }));
vi.mock("./agentbridge", () => ({
  setAgentSender: () => {},
  setPaneTargeting: () => {},
  setFleet: (fn: () => Array<{ name: string }>) => { io.fleet = fn; },
  setAgentSenderById: () => {},
  setPaneFocuser: () => {},
  setFleetSnapshot: () => {},
  setPaneRevealer: (fn: typeof io.reveal) => { io.reveal = fn; },
}));
vi.mock("./fleetbridge", async (real) => ({
  ...(await real<typeof import("./fleetbridge")>()),
  initFleetBridge: (bridge: unknown) => { io.bridge = bridge; },
}));
vi.mock("./chatsource", () => ({ knownConversation: () => null }));
vi.mock("./ptyexit", () => ({ exitVerdict: () => null, holdExit: () => {}, setExitHandler: () => {} }));
vi.mock("./spawnmodal", () => ({ spawnForConductor: async () => {} }));
vi.mock("./company/engine", () => ({ initCompany: async () => {} }));
vi.mock("./fleet", () => ({ paneStatus: () => "idle" }));
vi.mock("./delegation", () => ({ showDelegation: () => {}, showDelegationToast: () => {} }));
vi.mock("./flow", () => ({ pushFlow: () => {} }));

import { setActiveWs, workspaces } from "./appstate";

const pane = (id: string, name: string, personId?: string) => ({
  id, color: "#fff", running: true, attention: false, spawnedAt: 1, lastOutputAt: 1,
  spec: { name, badge: "claude", ...(personId ? { personId } : {}) },
  term: { snapshot: () => "" },
}) as unknown as Pane;

const bridge = () => io.bridge as Bridge;

beforeAll(async () => {
  document.body.innerHTML = '<main id="workspaces"></main>';
  const { initBridges } = await import("./bridges");
  initBridges();
});

beforeEach(() => {
  io.sent = [];
  const panes = [pane("f", "Founder", "founder"), pane("a", "Ana"), pane("d", "Director")];
  const ws = { id: "w", name: "app", dir: "D:\\app", panes: new Map(panes.map((p) => [p.id, p])) } as unknown as Workspace;
  workspaces.clear();
  workspaces.set("w", ws);
  setActiveWs(ws);
});

describe("revealing a pane", () => {
  const stageWith = (focused: boolean) => {
    const ws = workspaces.get("w")!;
    (ws as unknown as { gridEl: HTMLElement }).gridEl = document.createElement("div");
    if (focused) (ws as unknown as { gridEl: HTMLElement }).gridEl.classList.add("has-focus");
    const p = ws.panes.get("a")!;
    const focus = vi.fn();
    (p as unknown as { term: { focus: () => void } }).term.focus = focus;
    (p as unknown as { el: HTMLElement }).el = document.createElement("div");
    io.staged = [];
    return focus;
  };

  it("takes the keyboard by default", () => {
    const focus = stageWith(false);
    io.reveal!("w", "a");
    expect(focus).toHaveBeenCalledTimes(1);
  });
  it("leaves the keyboard alone when told to, off the stage", () => {
    const focus = stageWith(false);
    io.reveal!("w", "a", false);
    expect(focus).not.toHaveBeenCalled();
  });
  it("leaves it alone on the stage too", () => {
    stageWith(true);
    io.reveal!("w", "a", false);
    expect(io.staged).toEqual([false]);
  });
});

describe("the Founder in a project", () => {
  it("is not on the board's list of agents", () => {
    expect(io.fleet!().map((a) => a.name)).toEqual(["Ana", "Director"]);
  });

  it("is not on the roster fleet_status reads, so a Director hands it nothing", () => {
    expect(bridge().workspaces()[0].agents.map((a) => a.name)).toEqual(["Ana", "Director"]);
  });

  it("gets nothing from a fleet_send without a name", () => {
    bridge().deliver("D:\\app", "Director", null, "everyone: start");
    expect(io.sent).toEqual(["a"]);
  });

  it("gets nothing from a fleet_send that names it", () => {
    bridge().deliver("D:\\app", "Director", "Founder", "take this card");
    expect(io.sent).toEqual([]);
  });

  it("leaves everyone else reachable", () => {
    bridge().deliver("D:\\app", "Director", "Ana", "take this card");
    expect(io.sent).toEqual(["a"]);
  });
});
