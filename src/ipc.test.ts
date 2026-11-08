import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// Hoisted so the mock factory can safely reference it.
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke,
  Channel: class {
    onmessage: ((m: unknown) => void) | null = null;
  },
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue("D:\\projects\\demo"),
}));

import { spawnPty, attachPty, sendInput, resizePty, killPty } from "./ipc";

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
});

describe("ipc", () => {
  it("spawnPty passes agentId, cwd, env + camelCase args including the channel", async () => {
    await spawnPty("agent-1", "powershell.exe", ["-NoLogo"], "D:\\projects\\demo", 80, 24,
      [["BYTEHIVE_AGENT", "Claude Code #1"]], () => {});
    expect(invoke).toHaveBeenCalledWith(
      "pty_spawn",
      expect.objectContaining({
        agentId: "agent-1",
        program: "powershell.exe",
        args: ["-NoLogo"],
        cwd: "D:\\projects\\demo",
        cols: 80,
        rows: 24,
        env: [["BYTEHIVE_AGENT", "Claude Code #1"]],
        onBytes: expect.anything(),
      }),
    );
  });

  it("input / resize / kill are addressed by agentId", async () => {
    await sendInput("agent-2", "ls\r");
    expect(invoke).toHaveBeenCalledWith("pty_input", { agentId: "agent-2", data: "ls\r" });

    await resizePty("agent-2", 120, 40);
    expect(invoke).toHaveBeenCalledWith("pty_resize", { agentId: "agent-2", cols: 120, rows: 40 });

    await killPty("agent-2");
    expect(invoke).toHaveBeenCalledWith("pty_kill", { agentId: "agent-2" });
  });
});

describe("output flow control", () => {
  /** The channel a spawn handed the backend and the epoch it named, so a test can play the backend's part. */
  async function channelOf(handler: Parameters<typeof spawnPty>[7]) {
    await spawnPty("agent-9", "powershell.exe", [], null, 80, 24, [], handler);
    const args = invoke.mock.calls[0][1];
    return { ch: args.onBytes as { onmessage: (m: ArrayBuffer) => void }, epoch: args.epoch as number };
  }
  const acks = () => invoke.mock.calls.filter((c) => c[0] === "pty_ack").map((c) => c[1]);

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(5_000_000);
  });
  afterEach(() => vi.useRealTimers());

  it("names the channel with an epoch and acks the running total once the terminal has parsed it", async () => {
    let parsed: () => void = () => {};
    const { ch, epoch } = await channelOf((_bytes, done) => { parsed = done; });
    expect(Number.isSafeInteger(epoch) && epoch > 0).toBe(true);
    invoke.mockClear();
    ch.onmessage(new ArrayBuffer(300));
    expect(acks()).toEqual([]); // not parsed yet
    parsed();
    parsed(); // a second call says nothing new
    expect(acks()).toEqual([{ agentId: "agent-9", epoch, parsed: 300 }]);
  });

  it("a chunk whose callback never comes is covered by the next one", async () => {
    const done: Array<() => void> = [];
    const { ch, epoch } = await channelOf((_bytes, d) => { done.push(d); });
    invoke.mockClear();
    ch.onmessage(new ArrayBuffer(300)); // xterm dropped this one's callback
    ch.onmessage(new ArrayBuffer(200));
    done[1]();
    expect(acks()).toEqual([{ agentId: "agent-9", epoch, parsed: 500 }]);
  });

  it("an ack that failed on the way is made good by the next", async () => {
    const done: Array<() => void> = [];
    const { ch, epoch } = await channelOf((_bytes, d) => { done.push(d); });
    invoke.mockClear();
    invoke.mockRejectedValueOnce(new Error("ipc hiccup"));
    ch.onmessage(new ArrayBuffer(100));
    done[0](); // lost
    vi.advanceTimersByTime(1000);
    ch.onmessage(new ArrayBuffer(50));
    done[1]();
    const all = acks();
    expect(all[all.length - 1]).toEqual({ agentId: "agent-9", epoch, parsed: 150 });
  });

  it("acks at once for a handler that does not ask for the callback", async () => {
    const { ch, epoch } = await channelOf(() => {});
    invoke.mockClear();
    ch.onmessage(new ArrayBuffer(50));
    expect(acks()).toEqual([{ agentId: "agent-9", epoch, parsed: 50 }]);
  });

  it("every spawn and attach gets an epoch of its own", async () => {
    await spawnPty("a", "x", [], null, 80, 24, [], () => {});
    await attachPty("a", () => {});
    const [spawn, attach] = invoke.mock.calls.map((c) => c[1].epoch);
    expect(spawn).not.toEqual(attach);
    expect(invoke).toHaveBeenLastCalledWith("pty_attach", expect.objectContaining({ agentId: "a", epoch: attach }));
  });
});
