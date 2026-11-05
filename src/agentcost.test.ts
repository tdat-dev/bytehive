import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ asked: [] as Array<[string, string]>, rows: [] as unknown[], other: [] as unknown[][], answer: null as unknown }));
vi.mock("./ipc", () => ({
  claudeSessionUsage: async (dir: string, session: string) => { io.asked.push([dir, session]); return io.rows; },
  agentUsage: async (badge: string, dir: string, since: number | null) => { io.other.push([badge, dir, since]); return io.answer; },
}));

import { agentCost, agentUsage, forgetCost, onAgentCost, usageOnScreen } from "./agentcost";
import type { Pane } from "./panetypes";

const flush = async () => { for (let k = 0; k < 4; k++) await new Promise((r) => setTimeout(r, 0)); };
const pane = (over: Partial<Pane["spec"]> = {}, lastOutputAt = 0, screen = "") =>
  ({ id: "p1", lastOutputAt, spawnedAt: 10_000, term: { snapshot: () => screen }, spec: { badge: "claude", ranIn: "D:/app", sessionId: "s-1", ...over } }) as unknown as Pane;

describe("what each agent has used", () => {
  beforeEach(() => { io.asked = []; io.rows = []; io.other = []; io.answer = null; forgetCost("p1"); });

  it("reads a Claude agent's own session once, then only after it has done something new", async () => {
    io.rows = [{ model: "claude-opus-5-5", input_tokens: 0, output_tokens: 1e6, cache_creation: 0, cache_read: 0, messages: 1 }];
    let told = 0;
    onAgentCost(() => { told++; });
    const p = pane();
    expect(agentCost(p, 1000)).toBeNull(); // not read yet
    await flush();
    expect(io.asked).toEqual([["D:/app", "s-1"]]);
    expect(told).toBe(1);
    expect(agentUsage(p, 2000)).toEqual({ usd: 20, tokens: 1e6, fresh: 1e6, contextPct: null });
    // quiet since: no new read, however long it has been
    agentCost(p, 1000 + 120_000);
    await flush();
    expect(io.asked.length).toBe(1);
    // it worked again: read again, but not more than every 30s
    (p as { lastOutputAt: number }).lastOutputAt = 5000;
    agentCost(p, 1000 + 10_000);
    await flush();
    expect(io.asked.length).toBe(1);
    agentCost(p, 1000 + 40_000);
    await flush();
    expect(io.asked.length).toBe(2);
  });

  it("asks where Codex, opencode, Gemini, Qwen and grok keep theirs, for the session begun since it started", async () => {
    io.answer = { usd: null, tokens: 54_000, context_pct: 25, model: "gpt-5.3-codex" };
    const p = pane({ badge: "codex", sessionId: undefined });
    agentUsage(p, 1000);
    await flush();
    expect(io.other).toEqual([["codex", "D:/app", 5000]]);
    expect(agentUsage(p, 2000)).toEqual({ usd: null, tokens: 54_000, fresh: 54_000, contextPct: 25 });
  });

  it("takes what any other CLI's status line says, and has nothing for a shell", async () => {
    const p = pane({ badge: "aider", sessionId: undefined }, 0, "Tokens: 2.1k sent, 250 received. Cost: $0.01 message, $0.08 session.");
    agentUsage(p, 1000);
    await flush();
    expect(agentUsage(p, 2000)).toEqual({ usd: 0.08, tokens: null, fresh: null, contextPct: null });
    expect(io.other).toEqual([]);
    expect(agentUsage(pane({ badge: "shell" }))).toBeNull();
  });

  it("counts fresh tokens without the cache re-reads a long session piles up", async () => {
    io.rows = [{ model: "claude-opus-5-5", input_tokens: 1000, output_tokens: 2000, cache_creation: 500, cache_read: 9_000_000, messages: 40 }];
    const p = pane();
    agentUsage(p, 1000);
    await flush();
    expect(agentUsage(p, 2000)).toMatchObject({ tokens: 9_003_500, fresh: 3500 });
  });

  it("starts over on another conversation", async () => {
    const p = pane();
    agentCost(p, 1000);
    await flush();
    p.spec.sessionId = "s-2";
    agentCost(p, 1500);
    await flush();
    expect(io.asked).toEqual([["D:/app", "s-1"], ["D:/app", "s-2"]]);
  });
});

describe("usage a CLI shows on its screen", () => {
  it("reads Aider's session cost and a spent figure", () => {
    expect(usageOnScreen("Tokens: 12k sent, 1.1k received. Cost: $0.03 message, $1,204.50 session.").usd).toBe(1204.5);
    expect(usageOnScreen("  total cost $0.42 · 3 turns").usd).toBe(0.42);
    expect(usageOnScreen("nothing here").usd).toBeUndefined();
  });

  it("reads how full the context is from what is left", () => {
    expect(usageOnScreen("› \n  ⏎ send   42% context left").contextPct).toBe(58);
    expect(usageOnScreen("Context left until auto-compact: 12%").contextPct).toBe(88);
  });
});
