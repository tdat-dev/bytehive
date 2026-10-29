import { beforeEach, describe, expect, it, vi } from "vitest";

const sent: Array<[string, string]> = [];
const checkpoints: string[] = [];
let changed: Record<string, number> = {};
let changedPaths: Record<string, string[]> = {};
vi.mock("./ipc", () => ({
  gitChangedFiles: vi.fn(async (root: string) => (changedPaths[root] ?? Array.from({ length: changed[root] ?? 0 }, (_, i) => `f${i}`)).map((path) => ({ path, status: "M", added: 2, removed: 1 }))),
  sendInput: vi.fn(async (id: string, data: string) => { sent.push([id, data]); }),
  gitCheckpoint: vi.fn(async (dir: string) => { checkpoints.push(dir); return "abc1234def5678901234567890abcdef12345678"; }),
  // every agent here is a terminal
}));

import { workspaces } from "./appstate";
import { gitChangedFiles } from "./ipc";
import { allTasks, answerOption, answerText, describe as describeTask, historyOf, markMerged, onTasksChange, taskLine, taskOf, updateTasks } from "./tasks";
import { forgetFeed, setFeedItems } from "./gridcards";
import type { ChatItem, StepItem } from "./chatmodel";
import type { Pane, Workspace } from "./panetypes";

const ASK = `
 Bash command

   npm run build

 Do you want to proceed?
 ❯ 1. Yes
   2. Yes, and don't ask again for npm run build commands in D:\\p
   3. No, and tell Claude what to do differently (esc)
`;

const RISKY = [
  "Bash command",
  "",
  "   rm -rf /tmp/x",
  "",
  " Do you want to proceed?",
  " 1. Yes",
  " 2. Yes, and don't ask again (a)",
  " 3. No, and tell Claude what to do differently (esc)",
].join("\n");

function pane(id: string, name: string, over: Partial<Pane> & { screen?: string; worktree?: string } = {}): Pane {
  const { screen = "", worktree, ...rest } = over;
  return {
    id, el: {} as HTMLElement, running: true, spawnedAt: 0, lastOutputAt: 0, lastInputAt: 0,
    attention: false, attentionClearedAt: 0, attentionNotified: false, color: "#fff",
    spec: { program: "claude", args: [], cwd: "D:\\p", name, badge: "claude", color: "#fff", mono: "C", worktree, branch: worktree ? `${name.toLowerCase()}/x` : undefined },
    term: { snapshot: () => screen } as unknown as Pane["term"],
    ...rest,
  } as Pane;
}

function workspace(panes: Pane[]): Workspace {
  return { id: "ws-1", name: "maestro", dir: "D:\\p", repoRoot: "D:\\p", isolated: true, gridEl: {} as HTMLElement, panes: new Map(panes.map((p) => [p.id, p])), layout: new Map() };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

let itemN = 0;
const tStep = (over: Partial<StepItem> = {}): StepItem => ({ kind: "step", id: `t${++itemN}`, tool: "Bash", verb: "Ran", target: "npm test", done: true, at: 1000, ...over });
const tUser = (text = "fix it"): ChatItem => ({ kind: "user", id: `u${++itemN}`, text, images: 0, at: 500 });

beforeEach(() => {
  workspaces.clear(); sent.length = 0; checkpoints.length = 0; changed = {}; changedPaths = {}; forgetFeed();
  updateTasks(0); // drop tasks left from the previous test
});

describe("tasks", () => {
  it("derives each pane's state and orders the queue by what needs you", async () => {
    const now = 100_000;
    workspaces.set("ws-1", workspace([
      pane("a", "Ana", { screen: ASK, lastOutputAt: now - 30_000 }),
      pane("e", "Eli", { lastOutputAt: now - 500 }),
      pane("c", "Cy", { worktree: "D:\\wt\\cy", lastOutputAt: now - 30_000 }),
      pane("s", "Sam", { running: false }),
    ]));
    changed = { "D:\\wt\\cy": 3 };
    updateTasks(now);        // starts the diff lookup
    await flush();
    updateTasks(now + 10);   // …and uses it
    expect(allTasks().map((t) => [t.name, t.status.state])).toEqual([
      ["Ana", "needs"], ["Cy", "review"], ["Eli", "working"], ["Sam", "stopped"],
    ]);
    expect(taskOf("c")?.changedFiles).toBe(3);
    expect(taskLine("a")).toBe("Ana wants to run npm run build");
  });

  it("never marks a shared checkout ready to review: its diff isn't one agent's", async () => {
    workspaces.set("ws-1", workspace([pane("x", "Xo", { lastOutputAt: 0 })]));
    changed = { "D:\\p": 5 };
    updateTasks(100_000); await flush(); updateTasks(100_010);
    expect(taskOf("x")?.status.state).toBe("idle");
    expect(taskOf("x")?.changedFiles).toBeNull();
  });

  it("keeps when a state began, and tells listeners only about real changes", () => {
    const p = pane("a", "Ana", { screen: ASK, lastOutputAt: 0 });
    workspaces.set("ws-1", workspace([p]));
    const calls: number[] = [];
    const off = onTasksChange((l) => calls.push(l.length));
    updateTasks(1_000);
    updateTasks(2_000);
    expect(taskOf("a")?.since).toBe(1_000);
    expect(calls).toEqual([1]);
    (p.term as unknown as { snapshot: () => string }).snapshot = () => "";
    updateTasks(3_000);
    expect(taskOf("a")?.status.state).toBe("idle");
    expect(calls).toEqual([1, 1]);
    off();
  });

  it("forgets panes that were closed", () => {
    workspaces.set("ws-1", workspace([pane("a", "Ana")]));
    updateTasks(1);
    workspaces.clear();
    updateTasks(2);
    expect(allTasks()).toEqual([]);
  });

  it("answers with the exact keys the prompt showed", async () => {
    workspaces.set("ws-1", workspace([pane("a", "Ana", { screen: ASK })]));
    updateTasks(50_000);
    const ask = taskOf("a")!.status.ask!;
    await answerOption("a", ask.options[2]);
    await answerText("a", "use pnpm instead");
    expect(sent).toEqual([["a", "\x1b"], ["a", "use pnpm instead"], ["a", "\r"]]);
  });

  it("keeps a history of what each agent asked and what you answered", async () => {
    const p = pane("a", "Ana", { screen: ASK, lastOutputAt: 0 });
    workspaces.set("ws-1", workspace([p]));
    updateTasks(10_000);
    const opt = taskOf("a")!.status.ask!.options[0];
    await answerOption("a", opt);
    // Output bursts flip working on and off; only one "Started working" is kept.
    (p as { term: unknown }).term = { snapshot: () => "" };
    p.lastOutputAt = 20_000; updateTasks(20_100);
    p.lastOutputAt = 0; updateTasks(40_000);
    p.lastOutputAt = 40_500; updateTasks(40_600);
    await answerText("a", "  use pnpm ");
    expect(historyOf("a").map((e) => e.text)).toEqual([
      "Asked to run npm run build",
      "You picked “Yes”",
      "Started working",
      "You said “use pnpm”",
    ]);
  });

  it("counts the lines a branch adds and removes, and carries the job's title", async () => {
    const p = pane("c", "Cy", { worktree: "D:\\wt\\cy", lastOutputAt: 0 });
    p.spec.title = "Fix the flaky upload test";
    p.spec.race = { id: "r1", n: 2, of: 3 };
    workspaces.set("ws-1", workspace([p]));
    changed = { "D:\\wt\\cy": 1 };
    updateTasks(50_000);
    await flush(); await flush();
    updateTasks(50_010);
    const t = taskOf("c")!;
    expect([t.title, t.race?.n, t.added, t.removed]).toEqual(["Fix the flaky upload test", 2, 2, 1]);
  });

  it("calls a quiet agent done-with-nothing only after it has worked", async () => {
    const p = pane("c", "Cy", { lastOutputAt: 50_000 });
    workspaces.set("ws-1", workspace([p]));
    updateTasks(50_100); await flush();
    expect(taskOf("c")!.status.state).toBe("working");
    expect(taskLine("c")).toBe("Cy is working");
    updateTasks(500_000); await flush();
    expect(taskOf("c")!.status.state).toBe("idle");
    expect(taskLine("c")).toBe("Cy is done, with no changes to review");
  });

  it("marks a working agent stuck when its conversation loops the same failure", async () => {
    const p = pane("c", "Cy", { lastOutputAt: 100_000 });
    workspaces.set("ws-1", workspace([p]));
    const looped = (k: number) => [tUser(), ...Array.from({ length: k }, () => tStep({ error: true, output: "FAIL x" }))];
    setFeedItems("c", looped(3));
    updateTasks(100_100); await flush();
    const t = taskOf("c")!;
    expect(t.status.state).toBe("working");
    expect(t.status.stuck?.kind).toBe("loop");
    expect(historyOf("c").map((e) => e.text).join("\n")).toContain("Looks stuck");
    setFeedItems("c", looped(4));
    updateTasks(100_200); await flush();
    expect(historyOf("c").filter((e) => e.kind === "stuck")).toHaveLength(1);
  });

  it("warns when two agents in one project touch the same files", async () => {
    const a = pane("a", "Ana", { worktree: "D:\\wt\\a", lastOutputAt: 10_000 });
    const b = pane("b", "Bo", { worktree: "D:\\wt\\b", lastOutputAt: 10_000 });
    workspaces.set("ws-1", workspace([a, b]));
    changedPaths = { "D:\\wt\\a": ["src/app.ts"], "D:\\wt\\b": ["src/app.ts", "src/lib.ts"] };
    updateTasks(10_100); await flush();
    updateTasks(10_200); await flush();
    expect(taskOf("a")!.status.clash?.kind).toBe("file");
    expect(taskOf("b")!.status.clash?.files).toEqual(["src/app.ts"]);
    expect(historyOf("a").map((e) => e.text).join("\n")).toContain("Overlaps Bo in src/app.ts");
  });

  it("warns when two agents run the same task, unless they race it", async () => {
    const a = pane("a", "Ana", { lastOutputAt: 10_000 });
    const b = pane("b", "Bo", { lastOutputAt: 10_000 });
    a.spec.title = "Fix login"; b.spec.title = "Fix login";
    workspaces.set("ws-1", workspace([a, b]));
    updateTasks(10_100); await flush();
    expect(taskOf("a")!.status.clash?.kind).toBe("task");
    a.spec.race = { id: "r1", n: 1, of: 2 }; b.spec.race = { id: "r1", n: 2, of: 2 };
    updateTasks(10_200); await flush();
    expect(taskOf("a")!.status.clash).toBeNull();
  });

  it("checkpoints the worktree before approving a high-risk prompt", async () => {
    const p = pane("c", "Cy", { worktree: "D:\\wt\\cy", screen: RISKY });
    workspaces.set("ws-1", workspace([p]));
    updateTasks(100_000); await flush();
    const ask = taskOf("c")!.status.ask!;
    expect(ask.detail).toBe("rm -rf /tmp/x");
    await answerOption("c", ask.options[0]);
    expect(checkpoints).toEqual(["D:\\wt\\cy"]);
    expect(historyOf("c").map((e) => e.text).join("\n")).toContain("(checkpoint abc1234)");
    expect(sent).toEqual([["c", ask.options[0].key]]);
  });

  it("approves a low-risk prompt without a checkpoint", async () => {
    const p = pane("c", "Cy", { worktree: "D:\\wt\\cy", screen: ASK });
    workspaces.set("ws-1", workspace([p]));
    updateTasks(100_000); await flush();
    const ask = taskOf("c")!.status.ask!;
    await answerOption("c", ask.options[0]);
    expect(checkpoints).toEqual([]);
    expect(historyOf("c").map((e) => e.text).join("\n")).not.toContain("checkpoint");
  });

  it("stops asking git about an agent once it has stopped", async () => {
    const ask = vi.mocked(gitChangedFiles);
    const p = pane("c", "Cy", { worktree: "D:\\wt\\cy", lastOutputAt: 0 });
    workspaces.set("ws-1", workspace([p]));
    changed = { "D:\\wt\\cy": 2 };
    ask.mockClear();
    const t0 = Date.now(); // the poll clock is the wall clock
    updateTasks(t0 + 10_000); await flush();
    expect(ask).toHaveBeenCalledTimes(1);
    p.running = false;
    updateTasks(t0 + 20_000); await flush(); // one last look right after it stops
    updateTasks(t0 + 30_000); await flush();
    updateTasks(t0 + 40_000); await flush();
    expect(ask).toHaveBeenCalledTimes(2);
    expect(taskOf("c")?.changedFiles).toBe(2);
    p.running = true; // restarted: polled again
    updateTasks(t0 + 50_000); await flush();
    expect(ask).toHaveBeenCalledTimes(3);
  });

  it("shows a merged agent as merged until it starts again", () => {
    const p = pane("c", "Cy", { running: false, spawnedAt: 1_000 });
    workspaces.set("ws-1", workspace([p]));
    markMerged("c", "main", "cy/x", 2_000);
    updateTasks(3_000);
    expect(taskOf("c")!.status.state).toBe("merged");
    expect(taskOf("c")!.mergedInto).toBe("main");
    expect(historyOf("c").map((e) => e.text)).toContain("Merged cy/x into main");
    // started again after the merge: on something new
    p.running = true; p.spawnedAt = 4_000; p.lastOutputAt = 4_000;
    updateTasks(4_100);
    expect(taskOf("c")!.status.state).toBe("working");
    expect(taskOf("c")!.mergedInto).toBeNull();
  });
});

describe("describe", () => {
  it("writes a History line for a blocked agent", () => {
    expect(describeTask({ status: { state: "blocked", ask: null, blocked: { kind: "captcha", what: "CAPTCHA" } }, changedFiles: 0 })).toBe("Blocked on a CAPTCHA");
  });
});
