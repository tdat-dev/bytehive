import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

const codex = vi.fn();
const opencode = vi.fn();
const muse = vi.fn();
vi.mock("./ipc", () => ({
  claudeTranscript: vi.fn(),
  codexTranscript: (...a: unknown[]) => codex(...a),
  opencodeTranscript: (...a: unknown[]) => opencode(...a),
  museTranscript: (...a: unknown[]) => muse(...a),
}));

import { SOURCES, RECHECK_MS, forgetConversation } from "./chatsource";
import type { Pane } from "./panetypes";

const pane = (id: string, spawnedAt: number, spec = {}) => ({ id, spawnedAt, spec: { badge: "codex", ...spec } }) as unknown as Pane;

describe("chat sources remember the conversation they found", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
    forgetConversation();
    codex.mockReset();
    opencode.mockReset();
    muse.mockReset();
  });
  afterEach(() => vi.useRealTimers());

  it("codex: searches once, then passes the rollout path back", async () => {
    codex.mockResolvedValue({ path: "C:/r/rollout-a.jsonl", text: "", next: 0 });
    const p = pane("p1", 900_000);
    await SOURCES.codex.read(p, "D:/app", 0);
    await SOURCES.codex.read(p, "D:/app", 10);
    await SOURCES.codex.read(p, "D:/app", 20);
    expect(codex.mock.calls.map((c) => c[2])).toEqual([null, "C:/r/rollout-a.jsonl", "C:/r/rollout-a.jsonl"]);
  });

  it("codex: looks again now and then for a newer conversation, and after a restart", async () => {
    codex.mockResolvedValue({ path: "C:/r/a.jsonl", text: "", next: 0 });
    const p = pane("p1", 900_000);
    await SOURCES.codex.read(p, "D:/app", 0);
    vi.setSystemTime(1_000_000 + RECHECK_MS + 1);
    await SOURCES.codex.read(p, "D:/app", 0);
    expect(codex.mock.calls[1][2]).toBeNull();
    // the agent ran again: what it had is not its conversation any more
    const again = pane("p1", 2_000_000);
    await SOURCES.codex.read(again, "D:/app", 0);
    expect(codex.mock.calls[2][2]).toBeNull();
  });

  it("codex: a rollout that's gone is searched for again next time", async () => {
    codex.mockResolvedValueOnce({ path: "C:/r/a.jsonl", text: "", next: 0 });
    codex.mockResolvedValueOnce({ path: "", text: "", next: 0 });
    codex.mockResolvedValue({ path: "C:/r/b.jsonl", text: "", next: 0 });
    const p = pane("p1", 900_000);
    await SOURCES.codex.read(p, "D:/app", 0);
    await SOURCES.codex.read(p, "D:/app", 0);
    await SOURCES.codex.read(p, "D:/app", 0);
    await SOURCES.codex.read(p, "D:/app", 0);
    expect(codex.mock.calls.map((c) => c[2])).toEqual([null, "C:/r/a.jsonl", null, "C:/r/b.jsonl"]);
  });

  it("opencode: passes the session back once known, per agent", async () => {
    opencode.mockImplementation(async (dir: string) => ({ path: dir === "D:/a" ? "ses_a" : "ses_b", text: "", next: 0 }));
    const a = pane("pa", 900_000);
    const b = pane("pb", 900_000);
    await SOURCES.opencode.read(a, "D:/a", 0);
    await SOURCES.opencode.read(b, "D:/b", 0);
    await SOURCES.opencode.read(a, "D:/a", 5);
    await SOURCES.opencode.read(b, "D:/b", 5);
    expect(opencode.mock.calls.map((c) => c[2])).toEqual([null, null, "ses_a", "ses_b"]);
  });

  it("muse: reads its own session id, nothing until the capture lands it", async () => {
    muse.mockResolvedValue({ path: "s1", text: "{}", next: 5 });
    const p = pane("pm", 900_000, { badge: "muse", sessionId: "s1" });
    const r = await SOURCES.muse.read(p, "D:/app", 0, 99);
    expect(muse.mock.calls).toEqual([["s1", 0, 99]]);
    expect(r).toEqual({ path: "s1", text: "{}", next: 5 });
    const none = pane("pn", 900_000, { badge: "muse" });
    expect(await SOURCES.muse.read(none, "D:/app", 7)).toEqual({ path: "", text: "", next: 7 });
    expect(muse).toHaveBeenCalledTimes(1);
  });
});
