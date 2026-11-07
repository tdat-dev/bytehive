import { beforeEach, describe, expect, it, vi } from "vitest";

const sent: Array<[string, string]> = [];
let changed: Record<string, number> = {};
let folder: { head: string | null; files: Record<string, string>; committed: string[] } = { head: "h1", files: {}, committed: [] };
vi.mock("./ipc", () => ({
  // with `paths` it answers for those files, as a shared folder's agent asks
  agentChanges: vi.fn(async (dir: string, _from: string | null, paths: string[] | null) =>
    (paths ?? Array.from({ length: changed[dir] ?? 0 }, (_, i) => `f${i}`)).map((path) => ({ path, status: "M", added: 2, removed: 1 }))),
  folderState: vi.fn(async () => folder),
  sendInput: vi.fn(async (id: string, data: string) => { sent.push([id, data]); }),
  // every agent here is a terminal
}));

import { workspaces } from "./appstate";
import { agentChanges, folderState } from "./ipc";
import { allTasks, answerOption, answerText, changeScope, historyOf, markMerged, markReviewed, onTasksChange, taskLine, taskOf, updateTasks } from "./tasks";
import type { Pane, Workspace } from "./panetypes";

const ASK = `
 Bash command

   npm run build

 Do you want to proceed?
 ❯ 1. Yes
   2. Yes, and don't ask again for npm run build commands in D:\\p
   3. No, and tell Claude what to do differently (esc)
`;

function pane(id: string, name: string, over: Partial<Pane> & { screen?: string; worktree?: string } = {}): Pane {
  const { screen = "", worktree, ...rest } = over;
  return {
    id, el: {} as HTMLElement, running: true, spawnedAt: 0, lastOutputAt: 0, lastInputAt: 0,
    attention: false, attentionNotified: false, color: "#fff",
    spec: { program: "claude", args: [], cwd: "D:\\p", name, badge: "claude", color: "#fff", mono: "C", worktree, branch: worktree ? `${name.toLowerCase()}/x` : undefined },
    term: { snapshot: () => screen } as unknown as Pane["term"],
    ...rest,
  } as Pane;
}

function workspace(panes: Pane[]): Workspace {
  return { id: "ws-1", name: "bytehive", dir: "D:\\p", repoRoot: "D:\\p", isolated: true, gridEl: {} as HTMLElement, panes: new Map(panes.map((p) => [p.id, p])), layout: new Map() };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  workspaces.clear(); sent.length = 0; changed = {}; folder = { head: "h1", files: {}, committed: [] };
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

  describe("an agent with no worktree", () => {
    const QUIET = Date.now() + 100_000; // the poll clock is the wall clock
    /** One look at the folder, then the agents' own changes read back. */
    const look = async (now: number, files: Record<string, string>) => {
      folder = { ...folder, files };
      updateTasks(now); await flush();
      updateTasks(now + 1); await flush(); await flush();
      updateTasks(now + 2);
    };

    it("is not ready to review for what was dirty before it started", async () => {
      workspaces.set("ws-1", workspace([pane("x", "Xo", { lastOutputAt: 0 })]));
      await look(QUIET, { "old.txt": "1" });
      await look(QUIET + 6_000, { "old.txt": "1" });
      expect(taskOf("x")?.status.state).toBe("idle");
    });

    it("is ready to review once it really changed something, and Review reads the same files", async () => {
      const x = pane("x", "Xo", { lastOutputAt: 0 });
      workspaces.set("ws-1", workspace([x]));
      await look(QUIET, {});
      x.lastOutputAt = QUIET + 1_000; // it worked, then went quiet
      await look(QUIET + 6_000, { "a.txt": "1" });
      await look(QUIET + 12_000, { "a.txt": "1" });
      expect(taskOf("x")).toMatchObject({ changedFiles: 1, shared: 0, status: { state: "review" } });
      expect(changeScope(x)).toEqual({ dir: "D:\\p", from: "h1", paths: ["a.txt"], shared: 0 });
    });

    it("gets no credit for what changed while it was quiet", async () => {
      const x = pane("x", "Xo", { lastOutputAt: 0 });
      const y = pane("y", "Yan", { lastOutputAt: 0 });
      workspaces.set("ws-1", workspace([x, y]));
      await look(QUIET, {});
      y.lastOutputAt = QUIET + 1_000;
      await look(QUIET + 6_000, { "a.txt": "1" });
      await look(QUIET + 12_000, { "a.txt": "1" });
      expect(taskOf("y")?.status.state).toBe("review");
      expect(taskOf("x")).toMatchObject({ changedFiles: 0, status: { state: "idle" } });
    });

    it("two at work in one folder both show the file, marked as maybe the other's", async () => {
      const x = pane("x", "Xo", { lastOutputAt: 0 });
      const y = pane("y", "Yan", { lastOutputAt: 0 });
      workspaces.set("ws-1", workspace([x, y]));
      await look(QUIET, {});
      x.lastOutputAt = y.lastOutputAt = QUIET + 1_000;
      await look(QUIET + 6_000, { "a.txt": "1" });
      await look(QUIET + 12_000, { "a.txt": "1" });
      expect(taskOf("x")).toMatchObject({ changedFiles: 1, shared: 1, status: { state: "review" } });
      expect(taskOf("y")).toMatchObject({ changedFiles: 1, shared: 1, status: { state: "review" } });
    });

    it("leaves Ready to review once you mark it reviewed", async () => {
      const x = pane("x", "Xo", { lastOutputAt: 0 });
      workspaces.set("ws-1", workspace([x]));
      await look(QUIET, {});
      x.lastOutputAt = QUIET + 1_000;
      await look(QUIET + 6_000, { "a.txt": "1" });
      await look(QUIET + 12_000, { "a.txt": "1" });
      markReviewed("x", QUIET + 13_000);
      await look(QUIET + 18_000, { "a.txt": "1" });
      expect(taskOf("x")?.status.state).toBe("idle");
      expect(historyOf("x").slice(-1)[0]?.text).toBe("You reviewed its changes");
    });

    it("stays unknown outside a repo", async () => {
      vi.mocked(folderState).mockRejectedValueOnce("Not a git repository").mockRejectedValueOnce("Not a git repository");
      workspaces.set("ws-1", workspace([pane("x", "Xo", { lastOutputAt: 0 })]));
      await look(QUIET, {});
      expect(taskOf("x")?.changedFiles).toBeNull();
      expect(taskOf("x")?.status.state).toBe("idle");
    });

    it("a plain shell in the folder claims nothing", async () => {
      const s = pane("s", "Shell", { lastOutputAt: 0 });
      s.spec.badge = "shell";
      workspaces.set("ws-1", workspace([s]));
      await look(QUIET, {});
      s.lastOutputAt = QUIET + 1_000;
      await look(QUIET + 6_000, { "a.txt": "1" });
      expect(taskOf("s")?.changedFiles).toBeNull();
    });
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

  it("stops asking git about an agent once it has stopped", async () => {
    const ask = vi.mocked(agentChanges);
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
