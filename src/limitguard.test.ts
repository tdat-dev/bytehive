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
}));
vi.mock("./tasks", () => ({ allTasks: () => io.tasks, onTasksChange: () => () => {} }));
vi.mock("./ipc", () => ({
  sendMessage: async (id: string, text: string) => { io.sent.push([id, text]); },
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
vi.mock("./chatview", () => ({ chatOf: () => null }));
vi.mock("./chatsource", () => ({ sourceOf: () => null }));
vi.mock("./crew", () => ({
  CLI_PRESETS: [
    { id: "claude", label: "Claude Code", program: "claude", badge: "claude" },
    { id: "codex", label: "Codex", program: "codex", badge: "codex" },
    { id: "gemini", label: "Gemini", program: "gemini", badge: "gemini" },
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

import { CARRY_ON, checkLimits, handOff, handoffTargets, stopLimitGuard } from "./limitguard";

const RESET = Date.parse("2026-09-27T12:00:00Z");
const pane = (over: object = {}) => ({ id: "p1", running: true, spec: { name: "Ana", badge: "claude", ranIn: "D:/app", title: "Fix the header" }, ...over });
const task = (limit: object | null, state = "idle") => ({ paneId: "p1", wsId: "w1", name: "Ana", status: { state, ask: null, limit } });

describe("Limit Guard", () => {
  beforeEach(() => {
    io.sent = []; io.notes = []; io.plans = []; io.planReset = null; io.spawned = [];
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
    expect(handoffTargets(p).map((t) => t.id)).toEqual(["codex"]); // not itself, not a shell, not one not installed
    io.tasks = [task({ what: "You've hit your limit", resetsAt: RESET })];
    const name = await handOff("p1", "codex");
    expect(name).toBe("Bob");
    expect(io.spawned[0].preset).toBe("codex");
    expect(io.spawned[0].task).toContain("taking over a task from Ana (Claude Code)");
    expect(io.spawned[0].task).toContain("Its task: Fix the header");
    expect((io.panes.get("p2") as { spec: { title: string } }).spec.title).toBe("Carries on Ana's task: Fix the header");
  });
});
