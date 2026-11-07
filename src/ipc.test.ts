import { describe, it, expect, vi, beforeEach } from "vitest";

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

import { spawnPty, sendInput, resizePty, killPty } from "./ipc";

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
  /** The channel a spawn handed the backend, so a test can play the backend's part. */
  async function channelOf(handler: Parameters<typeof spawnPty>[7]) {
    await spawnPty("agent-9", "powershell.exe", [], null, 80, 24, [], handler);
    return invoke.mock.calls[0][1].onBytes as { onmessage: (m: ArrayBuffer) => void };
  }

  it("acks the bytes of a chunk once the terminal says it has parsed them", async () => {
    let parsed: () => void = () => {};
    const ch = await channelOf((_bytes, done) => { parsed = done; });
    invoke.mockClear();
    ch.onmessage(new ArrayBuffer(300));
    expect(invoke).not.toHaveBeenCalledWith("pty_ack", expect.anything()); // not parsed yet
    parsed();
    parsed(); // a second call counts nothing
    expect(invoke).toHaveBeenCalledExactlyOnceWith("pty_ack", { agentId: "agent-9", bytes: 300 });
  });

  it("acks at once for a handler that does not ask for the callback", async () => {
    const ch = await channelOf(() => {});
    invoke.mockClear();
    ch.onmessage(new ArrayBuffer(50));
    expect(invoke).toHaveBeenCalledWith("pty_ack", { agentId: "agent-9", bytes: 50 });
  });
});
