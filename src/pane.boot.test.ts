// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ state: "idle", worktreeFails: false, trustFails: false, written: [] as string[], spawns: 0, spawnFails: false, gitWaits: null as Promise<void> | null }));
vi.mock("./terminal", () => ({
  mountTerminal: () => ({
    fit: () => ({ cols: 80, rows: 24 }), setAutoFit: () => {}, onTitleChange: () => {}, write: (b: Uint8Array) => { io.written.push(new TextDecoder().decode(b)); },
    reset: () => {}, dispose: () => {}, setLook: () => {},
  }),
}));
vi.mock("./ipc", () => ({
  sendInput: async () => {},
  resizePty: async () => {},
  killPty: async () => {},
  spawnPty: async () => { io.spawns += 1; if (io.spawnFails) throw new Error("no terminal"); return io.spawns; },
  claudeSessionExists: async () => false,
  claudeTrustFolder: async () => { if (io.trustFails) throw new Error("the config is locked"); },
  worktreeAdd: async () => { if (io.gitWaits) await io.gitWaits; if (io.worktreeFails) throw new Error("branch exists"); return "D:\\wt\\ana"; },
  worktreeLeave: async () => null,
}));
vi.mock("@tauri-apps/api/path", () => ({ homeDir: async () => "H:\\" }));
vi.mock("./panelayout", () => ({ layoutGrid: () => {}, wirePaneRename: () => {}, toggleMax: () => {} }));
vi.mock("./session", () => ({ saveSession: () => {} }));
vi.mock("./replay", () => ({ openReplays: () => {}, REC_DIR_REL: ".bytehive/rec" }));
vi.mock("./background", () => ({ paneLook: () => ({}) }));
vi.mock("./zoom", () => ({ getZoom: () => 1, setZoom: () => {}, paneFont: () => 13, autoFitRows: () => 0 }));
vi.mock("./ctxmenu", () => ({ openMenu: () => {} }));
vi.mock("./clibrand", () => ({ cliMark: () => "" }));
vi.mock("./tasks", async (real) => ({ ...(await real<typeof import("./tasks")>()), taskOf: () => ({ status: { state: io.state } }) }));

import { clearAlert, standingAlerts } from "./company/alerts";
import { createAgent, updateAttention } from "./pane";
import { workspaces } from "./appstate";
import type { AgentSpec, Workspace } from "./panetypes";

const project = () => {
  const gridEl = document.createElement("div");
  gridEl.appendChild(document.createElement("div"));
  return { id: "w1", name: "shop", dir: "D:\\shop", repoRoot: "D:\\shop", isolated: false, gridEl, panes: new Map(), layout: new Map() } as unknown as Workspace;
};
const spec = (over: Partial<AgentSpec> = {}): AgentSpec => ({ program: "claude", args: [], cwd: "D:\\shop", name: "Ana", badge: "claude", color: "#fff", mono: "A", personId: "p1", ...over });
const alert = (key: string) => standingAlerts().find((a) => a.key === key);

beforeEach(() => {
  io.worktreeFails = false;
  io.trustFails = false;
  io.written = [];
  io.spawns = 0;
  io.spawnFails = false;
  io.gitWaits = null;
  for (const a of standingAlerts()) clearAlert(a.key);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

describe("the needs-you pill", () => {
  it("follows the task state, and silence alone never lights it", async () => {
    const ws = project();
    workspaces.set(ws.id, ws);
    await createAgent(ws, spec())();
    const pane = [...ws.panes.values()][0];
    pane.running = true;
    pane.lastOutputAt = 1; // long quiet after output: the old guess said it needs you
    io.state = "idle";
    updateAttention();
    expect(pane.attention).toBe(false);
    io.state = "needs";
    updateAttention();
    expect(pane.attention).toBe(true);
    io.state = "working";
    updateAttention();
    expect(pane.attention).toBe(false);
    workspaces.delete(ws.id);
  });
});

describe("a member whose worktree fails", () => {
  it("raises an alert, not only a line in the terminal", async () => {
    io.worktreeFails = true;
    await createAgent(project(), spec())();
    expect(alert("worktree-p1")).toMatchObject({ personId: "p1", text: expect.stringMatching(/Ana.*worktree.*branch exists/) });
    expect(io.written.join()).toContain("worktree failed");
  });
  it("drops the alert once they get one", async () => {
    io.worktreeFails = true;
    await createAgent(project(), spec())();
    io.worktreeFails = false;
    await createAgent(project(), spec())();
    expect(alert("worktree-p1")).toBeUndefined();
  });
  it("is nobody else's business: an agent outside the company raises nothing", async () => {
    io.worktreeFails = true;
    const ws = { ...project(), isolated: true };
    await createAgent(ws, spec({ personId: undefined }))();
    expect(standingAlerts()).toEqual([]);
  });
});

describe("a boot waiting on git", () => {
  const paneIn = (ws: Workspace) => [...ws.panes.values()][0];
  const turn = () => new Promise((r) => setTimeout(r));

  it("counts as booting from the start, so a message does not boot it again", async () => {
    let open = () => {};
    io.gitWaits = new Promise((r) => { open = r; });
    const ws = project();
    const first = createAgent(ws, spec())();
    await turn();
    expect(paneIn(ws).running).toBe(false);
    expect(paneIn(ws).spawning).toBe(true);
    open();
    await first;
    expect(paneIn(ws).spawning).toBe(false);
  });
  it("reaches the terminal once, whatever asks for a restart meanwhile", async () => {
    let open = () => {};
    io.gitWaits = new Promise((r) => { open = r; });
    const ws = project();
    const first = createAgent(ws, spec())();
    await turn();
    const again = paneIn(ws).restart!();
    await turn();
    open();
    await Promise.all([first, again]);
    expect(io.spawns).toBe(1);
  });
  it("is over when the spawn fails, so the pane can be started again", async () => {
    io.spawnFails = true;
    const ws = project();
    await createAgent(ws, spec())();
    expect(paneIn(ws).spawning).toBe(false);
    expect(paneIn(ws).error).toContain("no terminal");
    io.spawnFails = false;
    await paneIn(ws).restart!();
    expect(paneIn(ws).running).toBe(true);
  });
});

describe("a member whose folder cannot be trusted", () => {
  it("raises an alert, not only a console line", async () => {
    io.trustFails = true;
    await createAgent(project(), spec())();
    expect(alert("trust-p1")).toMatchObject({ personId: "p1", text: expect.stringMatching(/Ana.*question.*the config is locked/) });
  });
  it("drops the alert once it works", async () => {
    io.trustFails = true;
    await createAgent(project(), spec())();
    io.trustFails = false;
    await createAgent(project(), spec())();
    expect(alert("trust-p1")).toBeUndefined();
  });
});
