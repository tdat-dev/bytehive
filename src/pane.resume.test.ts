import { describe, expect, it, vi } from "vitest";
vi.mock("./terminal", () => ({ mountTerminal: () => ({}) }));
import { claudeSessionArgs, museSessionArgs, musePicksSession } from "./pane";
import type { AgentSpec } from "./panetypes";

const spec = (over: Partial<AgentSpec> = {}): AgentSpec => ({ program: "claude", args: [], cwd: "D:/app", name: "Ana", badge: "claude", color: "", mono: "", ...over });

describe("claudeSessionArgs", () => {
  it("carries on the agent's own conversation when Claude still has it", async () => {
    const s = spec({ sessionId: "11111111-2222-3333-4444-555555555555" });
    expect(await claudeSessionArgs(s, "D:/app", async () => true)).toEqual(["--resume", "11111111-2222-3333-4444-555555555555"]);
    expect(s.sessionId).toBe("11111111-2222-3333-4444-555555555555");
  });

  it("starts a new conversation when there is none to carry on", async () => {
    const s = spec({ sessionId: "11111111-2222-3333-4444-555555555555" });
    const args = await claudeSessionArgs(s, "D:/app", async () => false);
    expect(args[0]).toBe("--session-id");
    expect(args[1]).toBe(s.sessionId);
    expect(s.sessionId).not.toBe("11111111-2222-3333-4444-555555555555");
    const fresh = spec();
    expect((await claudeSessionArgs(fresh, "D:/app", async () => true))[0]).toBe("--session-id");
  });

  it("leaves the session to a preset that picks one itself", async () => {
    const s = spec({ args: ["--continue"], sessionId: "x" });
    expect(await claudeSessionArgs(s, "D:/app", async () => true)).toEqual([]);
    expect(s.sessionId).toBeUndefined();
  });
});

const mspec = (over: Partial<AgentSpec> = {}): AgentSpec => ({ ...spec(), program: "muse", badge: "muse", ...over });

describe("museSessionArgs", () => {
  it("resumes the agent's own session when Muse still has it", async () => {
    const s = mspec({ sessionId: "01a18216-5bd8-7b20-9b10-9f481ee54a74" });
    expect(await museSessionArgs(s, async () => true)).toEqual(["resume", "01a18216-5bd8-7b20-9b10-9f481ee54a74"]);
    expect(s.sessionId).toBe("01a18216-5bd8-7b20-9b10-9f481ee54a74");
  });

  it("boots fresh when the session is gone, forgetting the dead id", async () => {
    const s = mspec({ sessionId: "01a18216-5bd8-7b20-9b10-9f481ee54a74" });
    expect(await museSessionArgs(s, async () => false)).toEqual([]);
    expect(s.sessionId).toBeUndefined();
    expect(await museSessionArgs(mspec(), async () => true)).toEqual([]);
  });

  it("leaves the session to args that pick one (resume, exec)", async () => {
    const s = mspec({ args: ["resume", "--last"], sessionId: "x" });
    expect(await museSessionArgs(s, async () => true)).toEqual([]);
    expect(s.sessionId).toBeUndefined();
    const e = mspec({ args: ["exec", "hi"] });
    expect(await museSessionArgs(e, async () => true)).toEqual([]);
    expect(e.sessionId).toBeUndefined();
  });

  it("spots args that already choose a session", () => {
    expect(musePicksSession([])).toBe(false);
    expect(musePicksSession(["--yolo"])).toBe(false);
    expect(musePicksSession(["--yolo", "resume"])).toBe(true);
    expect(musePicksSession(["resume", "--last"])).toBe(true);
    expect(musePicksSession(["exec", "hi"])).toBe(true);
  });
});
