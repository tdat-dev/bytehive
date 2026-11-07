import { describe, expect, it } from "vitest";
import { Gate, type GateDeps } from "./gate";
import { sha256 } from "./hash";
import { normalize } from "./normalize";
import { defaultTools } from "./tools";
import type { BlockedEntry, LogEntry, ToolResult } from "./types";

const CASE_1 = "báo giá 120k input, 30k output, model claude-sonnet-5";
const CASE_2 = "250 nghìn token vào với 40 nghìn token ra của opus thì hết bao nhiêu?";

function setup(over: Partial<GateDeps> = {}) {
  const sent: Array<{ to: string; text: string }> = [];
  const results: ToolResult[] = [];
  const logs: LogEntry[] = [];
  const blocked: BlockedEntry[] = [];
  const store = new Map<string, ToolResult>();
  let clock = 1_000_000;
  let classifyCalls = 0;
  const base: GateDeps = {
    send: async (to, text) => {
      sent.push({ to, text });
      return true;
    },
    sendResult: (_req, result) => {
      results.push(result);
      return true;
    },
    classify: async () => {
      classifyCalls += 1;
      return { result: { is_actionable: true, data: null } };
    },
    isAwaiting: () => false,
    settings: () => ({ enabled: true, ttlDays: 7, dailyCallCap: 500, dailyTokenCap: 500_000, loopWindowMin: 10 }),
    cache: { get: async (k) => store.get(k) ?? null, set: async (k, v) => void store.set(k, v) },
    tools: defaultTools(),
    log: (entry) => logs.push(entry),
    block: async (entry) => {
      blocked.push(entry);
    },
    canCall: () => true,
    hash: sha256,
    id: () => `b${blocked.length}`,
    now: () => clock,
  };
  const gate = new Gate({ ...base, ...over });
  return {
    gate,
    sent,
    results,
    logs,
    blocked,
    store,
    calls: () => classifyCalls,
    advance: (ms: number) => {
      clock += ms;
    },
  };
}

describe("the gate's four scenarios", () => {
  it("1. answers a clear cost question with code and caches it", async () => {
    const t = setup();
    const out = await t.gate.send({ source: "kanban", to: "a1", text: CASE_1 });
    expect(out.kind).toBe("answered");
    expect(t.sent).toHaveLength(0);
    expect(t.results).toHaveLength(1);
    expect(t.results[0].values.usd as number).toBeCloseTo(0.54, 9);
    expect(t.store.size).toBe(1);
    expect(t.calls()).toBe(0);
  });

  it("2. reads a natural sentence and prices opus", async () => {
    const t = setup();
    const out = await t.gate.send({ source: "kanban", to: "a1", text: CASE_2 });
    expect(out.kind).toBe("answered");
    expect(t.results[0].values.usd as number).toBeCloseTo(1.8, 9);
    expect(t.store.size).toBe(1);
    expect(t.calls()).toBe(0);
  });

  it("3. blocks small talk before it reaches a pane", async () => {
    const t = setup();
    const out = await t.gate.send({ source: "kanban", to: "a2", text: "cảm ơn nhé" });
    expect(out).toMatchObject({ kind: "blocked", reason: "smalltalk" });
    expect(t.sent).toHaveLength(0);
    expect(t.results).toHaveLength(0);
    expect(t.blocked).toHaveLength(1);
    expect(t.blocked[0].text).toBe("cảm ơn nhé");
    expect(t.calls()).toBe(0);
  });

  it("4. answers an identical resend from cache, with no work", async () => {
    const t = setup();
    await t.gate.send({ source: "kanban", to: "a1", text: CASE_1 });
    const out = await t.gate.send({ source: "kanban", to: "a1", text: CASE_1 });
    expect(out.kind).toBe("cache-hit");
    expect(t.store.size).toBe(1);
    expect(t.calls()).toBe(0);
    expect(t.results).toHaveLength(2);
    const last = t.logs[t.logs.length - 1];
    expect(last?.path).toBe("cache-hit");
    expect(last?.ms.total).toBeLessThan(5);
    expect(last?.tokensSavedEstimate).toBeGreaterThan(0);
  });
});

describe("the gate's edges", () => {
  it("hashes the same message the same whatever the spacing", async () => {
    expect(await sha256(normalize("  cảm   ơn "))).toBe(await sha256(normalize("cảm ơn")));
  });

  it("lets an answer through while the agent is waiting on a question", async () => {
    const t = setup({ isAwaiting: () => true });
    const out = await t.gate.send({ source: "fleet", from: "Ana", to: "a1", text: "ok" });
    expect(out.kind).toBe("sent");
    expect(t.sent[0].text).toBe("ok");
    expect(t.blocked).toHaveLength(0);
  });

  it("blocks the same agent-to-agent message twice inside the window", async () => {
    const t = setup();
    const req = { source: "fleet" as const, from: "Ana", to: "a1", text: "sửa lỗi này" };
    await t.gate.send(req);
    const out = await t.gate.send(req);
    expect(out).toMatchObject({ kind: "blocked", reason: "loop" });
  });

  it("sends the message through when the provider throws", async () => {
    const t = setup({ classify: async () => Promise.reject(new Error("429")) });
    const out = await t.gate.send({ source: "fleet", from: "Ana", to: "a1", text: "sửa lỗi test" });
    expect(out.kind).toBe("sent");
    expect(t.sent[0].text).toBe("sửa lỗi test");
    expect(t.logs[t.logs.length - 1]?.error).toContain("429");
  });

  it("sends the message through when the daily cap is reached", async () => {
    const t = setup({ canCall: () => false });
    const out = await t.gate.send({ source: "fleet", from: "Ana", to: "a1", text: "sửa lỗi test" });
    expect(out.kind).toBe("sent");
    expect(t.logs[t.logs.length - 1]?.error).toBe("provider.capped");
  });

  it("blocks a confident guardrail answer from the provider", async () => {
    const t = setup({
      classify: async () => ({ result: { is_actionable: false, data: null }, usage: { in: 10, out: 5 } }),
    });
    const out = await t.gate.send({ source: "kanban", to: "a1", text: "hello world, just chatting" });
    expect(out).toMatchObject({ kind: "blocked", reason: "guardrail" });
    expect(t.blocked[0].reason).toBe("guardrail");
  });

  it("sends the message through when the provider answer is unusable", async () => {
    const t = setup({ classify: async () => ({ result: null, error: "bad_response" }) });
    const out = await t.gate.send({ source: "kanban", to: "a1", text: "hello world, just chatting" });
    expect(out.kind).toBe("sent");
    expect(t.logs[t.logs.length - 1]?.error).toBe("bad_response");
  });

  it("keeps the order of messages to one agent while the first waits on the provider", async () => {
    let release = (): void => {};
    const held = new Promise<void>((r) => (release = r));
    const t = setup({
      classify: async () => {
        await held;
        return { result: { is_actionable: true, data: null } };
      },
    });
    const first = t.gate.send({ source: "kanban", to: "a1", text: "sửa lỗi test" });
    const second = t.gate.send({ source: "kanban", to: "a1", text: "yes" });
    await new Promise((r) => setTimeout(r, 0));
    expect(t.sent).toHaveLength(0);
    release();
    await Promise.all([first, second]);
    expect(t.sent.map((m) => m.text)).toEqual(["sửa lỗi test", "yes"]);
  });

  it("does not hold one agent's message behind another agent's", async () => {
    let release = (): void => {};
    const held = new Promise<void>((r) => (release = r));
    const t = setup({
      classify: async () => {
        await held;
        return { result: { is_actionable: true, data: null } };
      },
    });
    const first = t.gate.send({ source: "kanban", to: "a1", text: "sửa lỗi test" });
    await t.gate.send({ source: "kanban", to: "a2", text: "yes" });
    expect(t.sent.map((m) => m.to)).toEqual(["a2"]);
    release();
    await first;
  });

  it("a failed send does not stop the next message to that agent", async () => {
    let n = 0;
    const t = setup({
      send: async () => {
        n += 1;
        if (n === 1) throw new Error("pane gone");
        return true;
      },
    });
    const a = await t.gate.send({ source: "kanban", to: "a1", text: "yes" });
    const b = await t.gate.send({ source: "kanban", to: "a1", text: "no" });
    expect(a.kind).toBe("failed");
    expect(b.kind).toBe("sent");
  });

  it("sends the message on when nobody can receive a computed answer", async () => {
    const t = setup({ sendResult: () => false });
    const out = await t.gate.send({ source: "fleet", to: "a1", text: CASE_1 });
    expect(out.kind).toBe("sent");
    expect(t.sent[0].text).toBe(CASE_1);
  });

  it("sends the message on when nobody can receive a cached answer", async () => {
    let open = true;
    const t = setup({ sendResult: () => open });
    await t.gate.send({ source: "kanban", to: "a1", text: CASE_1 });
    open = false;
    const out = await t.gate.send({ source: "fleet", to: "a1", text: CASE_1 });
    expect(out.kind).toBe("sent");
    expect(t.sent[0].text).toBe(CASE_1);
  });

  it("changes nothing while the switch is off", async () => {
    const t = setup({
      settings: () => ({ enabled: false, ttlDays: 7, dailyCallCap: 500, dailyTokenCap: 500_000, loopWindowMin: 10 }),
    });
    const out = await t.gate.send({ source: "kanban", to: "a1", text: "cảm ơn nhé" });
    expect(out.kind).toBe("sent");
    expect(t.sent).toHaveLength(1);
    expect(t.logs).toHaveLength(0);
  });
});
