// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ sent: [] as Array<[string, string]>, repo: null as string | null }));
vi.mock("./ipc", () => ({
  sendMessage: async (id: string, text: string) => { io.sent.push([id, text]); },
  gitRepoRoot: async () => io.repo,
  programsOnPath: async () => [],
}));

import { cliLookOf, configureSpawnModal, deliverTask, paneAsking, paneAtPrompt, spawnPerson } from "./spawnmodal";
import type { AgentSpec, Pane, Workspace } from "./panetypes";

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

describe("deliverTask to a Claude still starting", () => {
  beforeEach(() => { io.sent = []; vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  const box = "──────────────\n> \n──────────────\n  ⏵⏵ bypass permissions on";
  const claude = (screen: { text: string }) => {
    const s = setup(screen);
    s.pane.spec.badge = "claude";
    return s;
  };

  it("holds the job until its input box is on screen, however long a resume takes", async () => {
    const screen = { text: " Loading…" };
    const { ws } = claude(screen);
    deliverTask(ws, ["Ana"], "add mul()", 1000, 1000);
    await vi.advanceTimersByTimeAsync(40_000);
    expect(io.sent).toEqual([]);
    screen.text = box;
    await vi.advanceTimersByTimeAsync(1000);
    expect(io.sent).toEqual([["p1", "add mul()"]]);
  });

  it("types anyway after a while: a screen it can't read must not lose the job", async () => {
    const { ws } = claude({ text: " something new" });
    deliverTask(ws, ["Ana"], "add mul()", 1000, 1000);
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(io.sent).toEqual([["p1", "add mul()"]]);
  });

  it("doesn't look for a box in a CLI that draws none", async () => {
    const { ws } = setup({ text: " aider>" });
    deliverTask(ws, ["Ana"], "add mul()", 1000, 1000);
    await vi.advanceTimersByTimeAsync(1000);
    expect(io.sent).toEqual([["p1", "add mul()"]]);
  });
});

describe("paneAtPrompt", () => {
  const screen = (text: string, badge: string) => {
    const { pane } = setup({ text });
    pane.spec.badge = badge;
    return pane;
  };
  it("is true once Claude has drawn its box", () => {
    expect(paneAtPrompt(screen("──────────────\n> \n──────────────", "claude"))).toBe(true);
    expect(paneAtPrompt(screen(" Loading…", "claude"))).toBe(false);
  });
  it("is always true for a CLI with no box to look for", () => {
    expect(paneAtPrompt(screen(" Loading…", "aider"))).toBe(true);
  });
});

describe("deliverTask after a question", () => {
  beforeEach(() => { io.sent = []; vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it("gives a full head start after an answer, however short the first wait was", async () => {
    const screen = { text: TRUST };
    const { ws } = setup(screen);
    deliverTask(ws, ["Ana"], "add mul()", 100, 1000, 60_000, 8000);
    await vi.advanceTimersByTimeAsync(5000);
    screen.text = PROMPT;
    await vi.advanceTimersByTimeAsync(8000);
    expect(io.sent).toEqual([]);
    await vi.advanceTimersByTimeAsync(1000);
    expect(io.sent).toEqual([["p1", "add mul()"]]);
  });
});

describe("paneAsking", () => {
  it("sees a question on the pane's screen", () => {
    expect(paneAsking(setup({ text: TRUST }).pane)).toBe(true);
    expect(paneAsking(setup({ text: PROMPT }).pane)).toBe(false);
  });
});

describe("on a tall pane", () => {
  /** A terminal `rows` high: what is drawn sits at the top, blank rows below it,
   *  and snapshot(40) only reads the buffer's last 40 lines. */
  const tall = (rows: number, text: string, badge = "claude") => {
    const lines = [...text.split("\n"), ...Array(rows).fill("")].slice(0, rows);
    const term = {
      snapshot: (n = 40) => lines.slice(-n).join("\n"),
      screen: () => lines.join("\n"),
    };
    return { id: "p1", running: true, spec: { name: "Ana", badge }, term } as unknown as Pane;
  };
  const BOX = "──────────────\n> \n──────────────";

  it.each([52, 60, 120])("sees Claude's box on %i rows, drawn above the last 40", (rows) => {
    expect(paneAtPrompt(tall(rows, BOX))).toBe(true);
  });
  it.each([52, 60, 120])("sees a question on %i rows", (rows) => {
    expect(paneAsking(tall(rows, TRUST))).toBe(true);
  });
  it("still reads a terminal with no screen of its own", () => {
    const { pane } = setup({ text: BOX });
    pane.spec.badge = "claude";
    expect(paneAtPrompt(pane)).toBe(true);
  });
});

describe("spawnPerson", () => {
  it("finds the repo for the member without touching the project's setting", async () => {
    const ws = { dir: "D:\\a", repoRoot: null, isolated: false, panes: new Map() } as unknown as Workspace;
    const atBoot: Array<[string | null, boolean]> = [];
    configureSpawnModal({
      createWorkspace: () => ws,
      createAgent: () => async () => { atBoot.push([ws.repoRoot, ws.isolated]); },
      cliLook: () => ({ color: "#123456", mono: "C" }),
    });
    io.repo = "D:\\a";
    await spawnPerson("D:\\a", {} as AgentSpec, true);
    expect(atBoot).toEqual([["D:\\a", false]]);
    expect(ws.isolated).toBe(false);
  });

  it("reads the CLI look main.ts set, not the placeholder", () => {
    configureSpawnModal({
      createWorkspace: () => { throw new Error("unused"); },
      createAgent: () => async () => {},
      cliLook: () => ({ color: "#123456", mono: "C" }),
    });
    expect(cliLookOf("claude", "Claude Code")).toEqual({ color: "#123456", mono: "C" });
  });
});
