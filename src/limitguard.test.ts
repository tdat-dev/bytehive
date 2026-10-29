import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({
  tasks: [] as unknown[],
  sent: [] as Array<[string, string]>,
  notes: [] as string[],
  prefs: { autoResume: true, limitNotify: true } as Record<string, boolean>,
  plans: [] as unknown[],
  planReset: null as number | null,
  spawned: [] as Array<{ preset: string; task: string }>,
  panes: new Map<string, unknown>(),
  gitFiles: {} as Record<string, Array<{ path: string }>>,
}));
vi.mock("./tasks", () => ({ allTasks: () => io.tasks, onTasksChange: () => () => {} }));
vi.mock("./ipc", () => ({
  sendMessage: async (id: string, text: string) => { io.sent.push([id, text]); },
  gitChangedFiles: async (root: string) => io.gitFiles[root] ?? [],
  notify: async (title: string) => { io.notes.push(title); },
}));
vi.mock("./prefs", () => ({ getPref: (k: string) => io.prefs[k] }));
vi.mock("./planmeter", () => ({
  planSnapshot: () => io.plans,
  planResetFor: () => io.planReset,
  limitName: (k: string) => (k === "session" ? "5-hour limit" : "weekly limit"),
  limitResetsAt: (l: { resets_ms?: number }) => l.resets_ms ?? null,
}));
vi.mock("./appstate", () => ({ workspaces: new Map([["w1", { id: "w1", panes: io.panes }]]) }));
vi.mock("./chatsource", () => ({ sourceOf: () => null }));
vi.mock("./crew", () => ({
  CLI_PRESETS: [
    { id: "claude", label: "Claude Code", program: "claude", badge: "claude" },
    { id: "codex", label: "Codex", program: "codex", badge: "codex" },
    { id: "gemini", label: "Gemini", program: "gemini", badge: "gemini" },
    { id: "opencode", label: "OpenCode", program: "opencode", badge: "opencode" },
    { id: "powershell", label: "PowerShell", program: "powershell.exe", badge: "shell", shell: true },
  ],
}));
vi.mock("./spawnmodal", () => ({
  presetAvailable: (p: string) => p !== "gemini",
  loadCrew: () => ({ skipPerms: true }),
  spawnAgents: async (ws: { panes: Map<string, unknown> }, preset: string, _n: number, task: string) => {
    io.spawned.push({ preset, task });
    ws.panes.set("p2", { id: "p2", spec: { name: "Bob", badge: preset } });
    return ["Bob"];
  },
}));

import { CARRY_ON, checkLimits, handOff, handOverToAgent, handoffTargets, openTargets, stopLimitGuard } from "./limitguard";
import { agentUsage, forgetCost } from "./agentcost";

const RESET = Date.parse("2026-09-27T12:00:00Z");
const pane = (over: object = {}) => ({ id: "p1", running: true, spec: { name: "Ana", badge: "claude", ranIn: "D:/app", title: "Fix the header" }, ...over });
const task = (limit: object | null, state = "idle") => ({ paneId: "p1", wsId: "w1", name: "Ana", status: { state, ask: null, limit } });

describe("Limit Guard", () => {
  beforeEach(() => {
    io.sent = []; io.notes = []; io.plans = []; io.planReset = null; io.spawned = []; io.gitFiles = {};
    io.prefs = { autoResume: true, limitNotify: true };
    io.panes.clear();
    io.panes.set("p1", pane());
    stopLimitGuard(); // forgets what it saw
  });

  it("says once that an agent is out of usage, and tells it to carry on once the limit has reset", () => {
    io.tasks = [task({ what: "You've hit your limit", resetsAt: RESET })];
    checkLimits(RESET - 3600e3);
    checkLimits(RESET - 1800e3);
    expect(io.notes).toEqual(["Ana is out of usage"]);
    // at the reset it waits a little, then carries on, once
    checkLimits(RESET + 10e3);
    expect(io.sent).toEqual([]);
    checkLimits(RESET + 120e3);
    checkLimits(RESET + 150e3);
    expect(io.sent).toEqual([["p1", CARRY_ON]]);
    expect(io.notes).toContain("Ana is back at work");
  });

  it("takes the reset from the plan when the screen doesn't say, and never nudges a working or stopped agent", () => {
    io.planReset = RESET;
    io.tasks = [task({ what: "usage limit reached", resetsAt: null })];
    checkLimits(RESET - 60e3); // seen before the reset
    io.tasks = [task({ what: "usage limit reached", resetsAt: null }, "working")];
    checkLimits(RESET + 120e3);
    expect(io.sent).toEqual([]);
    io.panes.set("p1", pane({ running: false }));
    io.tasks = [task({ what: "usage limit reached", resetsAt: null })];
    checkLimits(RESET + 130e3);
    expect(io.sent).toEqual([]);
    io.panes.set("p1", pane());
    io.planReset = null; // passed: the plan knows only the next one now
    checkLimits(RESET + 140e3);
    expect(io.sent).toEqual([["p1", CARRY_ON]]);
  });

  it("keeps the reset it first read, even when the same notice reads as tomorrow's later", () => {
    io.tasks = [task({ what: "You've hit your limit · resets 7pm", resetsAt: RESET })];
    checkLimits(RESET - 3600e3);
    // after 7pm the same words would mean tomorrow's 7pm
    io.tasks = [task({ what: "You've hit your limit · resets 7pm", resetsAt: RESET + 86400e3 })];
    checkLimits(RESET + 120e3);
    expect(io.sent).toEqual([["p1", CARRY_ON]]);
    expect(io.notes.filter((n) => n.includes("out of usage"))).toHaveLength(1);
  });

  it("leaves it waiting when Carry on is off", () => {
    io.prefs.autoResume = false;
    io.tasks = [task({ what: "You've hit your limit", resetsAt: RESET })];
    checkLimits(RESET + 600e3);
    expect(io.sent).toEqual([]);
  });

  it("warns once as a plan passes 90%", () => {
    io.tasks = [];
    io.plans = [{ provider: "Claude", plan: "max", limits: [{ kind: "session", percent: 93, resets_ms: RESET, severity: "normal", model: null }, { kind: "weekly_all", percent: 40, resets_ms: RESET + 5 * 86400e3, severity: "normal", model: null }] }];
    checkLimits(RESET - 3600e3);
    checkLimits(RESET - 1800e3);
    expect(io.notes).toEqual(["Claude 5-hour limit: 93% used"]);
  });

  it("hands the task to another installed CLI, briefed, with a title that says whose task it is", async () => {
    const p = io.panes.get("p1") as never;
    expect(handoffTargets(p).map((t) => t.id)).toEqual(["codex", "opencode"]); // not itself, not a shell, not one not installed
    io.tasks = [task({ what: "You've hit your limit", resetsAt: RESET })];
    const name = await handOff("p1", "codex");
    expect(name).toBe("Bob");
    expect(io.spawned[0].preset).toBe("codex");
    expect(io.spawned[0].task).toContain("taking over a task from Ana (Claude Code)");
    expect(io.spawned[0].task).toContain("Its task: Fix the header");
    expect((io.panes.get("p2") as { spec: { title: string } }).spec.title).toBe("Carries on Ana's task: Fix the header");
  });

  it("briefs the new agent with the files git says changed", async () => {
    (io.panes.get("p1") as { spec: Record<string, unknown> }).spec.worktree = "D:/wt/a";
    io.gitFiles = { "D:/wt/a": [{ path: "src/app.ts" }, { path: "src/lib.ts" }] };
    io.tasks = [task({ what: "You've hit your limit", resetsAt: RESET })];
    expect(await handOff("p1", "codex")).toBe("Bob");
    expect(io.spawned[0].task).toContain("Changed on disk (git, 2 files)");
    expect(io.spawned[0].task).toContain("- src/app.ts");
  });

  describe("handing over to an agent already open", () => {
    const other = (id: string, name: string, badge: string, over: object = {}) =>
      ({ id, running: true, spec: { name, badge, ranIn: "d:\\APP\\", ...over } });
    const state = (paneId: string, s: string) => ({ paneId, wsId: "w1", name: paneId, status: { state: s, ask: null } });

    beforeEach(() => {
      io.panes.set("p3", other("p3", "Bo", "opencode"));
      io.panes.set("p4", other("p4", "Cy", "codex"));
      io.panes.set("p5", other("p5", "Di", "codex", { ranIn: "D:/elsewhere" }));
      io.panes.set("p6", other("p6", "Ed", "gemini", { role: "conductor" }));
      io.panes.set("p7", other("p7", "Fy", "claude"));
      io.tasks = [task(null), state("p3", "idle"), state("p4", "working"), state("p5", "idle"), state("p6", "idle"), state("p7", "idle")];
    });

    it("offers the free agents of another CLI in the same folder", () => {
      expect(openTargets(io.panes.get("p1") as never).map((t) => t.name)).toEqual(["Bo"]);
      io.tasks = [task(null), state("p3", "idle"), state("p4", "review")];
      expect(openTargets(io.panes.get("p1") as never).map((t) => `${t.name} (${t.cli})`)).toEqual(["Bo (OpenCode)", "Cy (Codex)"]);
    });

    it("tells it to read the first agent's context, and the first one is no longer told to carry on", async () => {
      expect(await handOverToAgent("p1", "p3", "you asked")).toBe("Bo");
      expect(io.sent[0][0]).toBe("p3");
      expect(io.sent[0][1]).toContain("Take over the unfinished task of Ana (Claude Code), which stopped because you asked.");
      expect(io.sent[0][1]).toContain("agent_context tool with who set to Ana");
      const ana = io.panes.get("p1") as { handedTo?: string; spec: { title: string } };
      expect(ana.handedTo).toBe("Bo");
      expect(ana.spec.title).toBe("Handed to Bo: Fix the header");
      io.sent = [];
      io.tasks = [task({ what: "You've hit your limit", resetsAt: RESET })];
      checkLimits(RESET + 600e3);
      expect(io.sent).toEqual([]);
    });
  });

  describe("handing over by itself when it runs out", () => {
    const limited = () => task({ what: "You've hit your limit", resetsAt: RESET });
    const flush = () => new Promise((r) => setTimeout(r, 0));
    beforeEach(() => {
      io.prefs.autoHandover = true;
    });

    it("gives the task to a free agent already open in its folder, once, and never tells it to carry on", async () => {
      io.panes.set("p3", { id: "p3", running: true, spec: { name: "Bo", badge: "opencode", ranIn: "D:/app" } });
      io.tasks = [limited(), { paneId: "p3", wsId: "w1", name: "Bo", status: { state: "idle", ask: null } }];
      checkLimits(RESET - 3600e3);
      await flush();
      checkLimits(RESET - 1800e3);
      await flush();
      expect(io.sent.map(([id]) => id)).toEqual(["p3"]);
      expect(io.notes).toEqual(["Ana ran out of usage"]);
      expect((io.panes.get("p1") as { handedTo?: string }).handedTo).toBe("Bo");
      checkLimits(RESET + 600e3);
      expect(io.sent.map(([id]) => id)).toEqual(["p3"]);
    });

    it("starts a new agent of a CLI with room when none is open", async () => {
      io.tasks = [limited()];
      checkLimits(RESET - 3600e3);
      await flush();
      expect(io.spawned.map((x) => x.preset)).toEqual(["codex"]);
    });

    it("hands over to the cheapest open agent with room", async () => {
      const pricey = pane({ id: "p2", spec: { name: "Bo", badge: "aider", ranIn: "D:/app" }, term: { snapshot: () => "Cost: $0.01 message, $5.00 session." } });
      const cheap = pane({ id: "p3", spec: { name: "Cy", badge: "aider", ranIn: "D:/app" }, term: { snapshot: () => "Cost: $0.01 message, $1.00 session." } });
      io.panes.set("p2", pricey);
      io.panes.set("p3", cheap);
      io.tasks = [
        { paneId: "p1", wsId: "w1", name: "Ana", status: { state: "working", ask: null, limit: null } },
        { paneId: "p2", wsId: "w1", name: "Bo", status: { state: "idle", ask: null, limit: null } },
        { paneId: "p3", wsId: "w1", name: "Cy", status: { state: "idle", ask: null, limit: null } },
      ];
      agentUsage(pricey as never); agentUsage(cheap as never);
      await flush();
      expect(openTargets(pane() as never).map((t) => t.paneId)).toEqual(["p3", "p2"]);
      forgetCost("p2"); forgetCost("p3");
    });

    it("waits for the reset when every other plan is out too", () => {
      io.planReset = RESET; // every plan out
      io.tasks = [limited()];
      checkLimits(RESET - 3600e3);
      expect(io.spawned).toEqual([]);
      expect(io.notes).toEqual(["Ana is out of usage"]);
    });

    it("leaves it be when the setting is off", () => {
      io.prefs.autoHandover = false;
      io.tasks = [limited()];
      checkLimits(RESET - 3600e3);
      expect(io.spawned).toEqual([]);
      expect(io.sent).toEqual([]);
    });
  });
});
