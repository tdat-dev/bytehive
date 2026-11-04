// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ sent: [] as Array<[string, string]> }));
vi.mock("./ipc", () => ({
  sendMessage: async (id: string, text: string) => { io.sent.push([id, text]); },
  gitRepoRoot: async () => null,
  programsOnPath: async () => [],
}));

import { deliverTask } from "./spawnmodal";
import type { Pane, Workspace } from "./panetypes";

// Claude's first screen in a folder it hasn't seen, as the terminal shows it.
const TRUST = [
  " Quick safety check: Is this a project you created or one you trust?",
  " Claude Code'll be able to read, edit, and execute files here.",
  "",
  " ❯ No, exit",
  "   Yes, I trust this folder",
  "",
  " Enter to confirm · Esc to cancel",
].join("\n");
const PROMPT = "> \n  ? for shortcuts";

function setup(screen: { text: string }): { ws: Workspace; pane: Pane } {
  const pane = { id: "p1", running: true, spec: { name: "Ana" }, term: { snapshot: () => screen.text } } as unknown as Pane;
  const ws = { panes: new Map([["p1", pane]]) } as unknown as Workspace;
  return { ws, pane };
}

describe("deliverTask", () => {
  beforeEach(() => { io.sent = []; vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it("types the job once the CLI is at its prompt", async () => {
    const { ws } = setup({ text: PROMPT });
    deliverTask(ws, ["Ana"], "add mul()", 3000);
    await vi.advanceTimersByTimeAsync(2999);
    expect(io.sent).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    expect(io.sent).toEqual([["p1", "add mul()"]]);
  });

  it("waits while the CLI asks to trust the folder, so its Enter can't pick 'No, exit'", async () => {
    const screen = { text: TRUST };
    const { ws } = setup(screen);
    deliverTask(ws, ["Ana"], "add mul()", 3000, 1000);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(io.sent).toEqual([]);
    screen.text = PROMPT; // you answered "Yes, I trust this folder"
    // Claude restarts its session after that: the same head start again,
    // counted from the first look that finds the question gone
    await vi.advanceTimersByTimeAsync(3000);
    expect(io.sent).toEqual([]);
    await vi.advanceTimersByTimeAsync(1000);
    expect(io.sent).toEqual([["p1", "add mul()"]]);
    await vi.advanceTimersByTimeAsync(5000);
    expect(io.sent.length).toBe(1); // once
  });

  it("never answers the question for you, even when nobody does", async () => {
    const { ws } = setup({ text: TRUST });
    deliverTask(ws, ["Ana"], "add mul()", 1000, 1000, 5000);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(io.sent).toEqual([]);
    expect(vi.getTimerCount()).toBe(0); // and stops looking
  });

  it("drops an agent that stopped before it got the job", async () => {
    const { ws, pane } = setup({ text: TRUST });
    deliverTask(ws, ["Ana"], "add mul()", 1000, 1000);
    await vi.advanceTimersByTimeAsync(2000);
    pane.running = false;
    await vi.advanceTimersByTimeAsync(2000);
    expect(io.sent).toEqual([]);
    expect(vi.getTimerCount()).toBe(0);
  });
});
