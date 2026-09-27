import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ asked: [] as Array<[string, string]>, rows: [] as unknown[] }));
vi.mock("./ipc", () => ({
  claudeSessionUsage: async (dir: string, session: string) => { io.asked.push([dir, session]); return io.rows; },
}));

import { agentCost, forgetCost, onAgentCost } from "./agentcost";
import type { Pane } from "./panetypes";

const flush = async () => { for (let k = 0; k < 4; k++) await new Promise((r) => setTimeout(r, 0)); };
const pane = (over: Partial<Pane["spec"]> = {}, lastOutputAt = 0) =>
  ({ id: "p1", lastOutputAt, spec: { badge: "claude", ranIn: "D:/app", sessionId: "s-1", ...over } }) as unknown as Pane;

describe("what each agent has cost", () => {
  beforeEach(() => { io.asked = []; io.rows = []; forgetCost("p1"); });

  it("reads its own session once, then only after it has done something new", async () => {
    io.rows = [{ model: "claude-opus-5-5", input_tokens: 0, output_tokens: 1e6, cache_creation: 0, cache_read: 0, messages: 1 }];
    let told = 0;
    onAgentCost(() => { told++; });
    const p = pane();
    expect(agentCost(p, 1000)).toBeNull(); // not read yet
    await flush();
    expect(io.asked).toEqual([["D:/app", "s-1"]]);
    expect(told).toBe(1);
    expect(agentCost(p, 2000)).toBe(20);
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

  it("has nothing to say about other CLIs or an agent without its own session", () => {
    expect(agentCost(pane({ badge: "codex" }))).toBeNull();
    expect(agentCost(pane({ sessionId: undefined }))).toBeNull();
    expect(io.asked).toEqual([]);
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
