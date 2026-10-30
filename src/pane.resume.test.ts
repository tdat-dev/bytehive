import { describe, expect, it, vi } from "vitest";
vi.mock("./terminal", () => ({ mountTerminal: () => ({}) }));
import { claudeSessionArgs, museSessionArgs, musePicksSession, codexSessionArgs, codexPicksSession, opencodeSessionArgs, opencodePicksSession } from "./pane";
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

const cxspec = (over: Partial<AgentSpec> = {}): AgentSpec => ({ ...spec(), program: "codex", badge: "codex", ...over });

describe("codexSessionArgs", () => {
  it("resumes the agent's own session when Codex still has it", async () => {
    const s = cxspec({ sessionId: "01a16223-53fa-79e2-bc31-fe774823ba71" });
    expect(await codexSessionArgs(s, async () => true)).toEqual(["resume", "01a16223-53fa-79e2-bc31-fe774823ba71"]);
    expect(s.sessionId).toBe("01a16223-53fa-79e2-bc31-fe774823ba71");
  });

  it("boots fresh when the session is gone, forgetting the dead id", async () => {
    const s = cxspec({ sessionId: "01a16223-53fa-79e2-bc31-fe774823ba71" });
    expect(await codexSessionArgs(s, async () => false)).toEqual([]);
    expect(s.sessionId).toBeUndefined();
    expect(await codexSessionArgs(cxspec(), async () => true)).toEqual([]);
  });

  it("spots args that already choose a session", () => {
    expect(codexPicksSession([])).toBe(false);
    expect(codexPicksSession(["--yolo"])).toBe(false);
    expect(codexPicksSession(["resume", "--last"])).toBe(true);
    expect(codexPicksSession(["exec", "hi"])).toBe(true);
    expect(codexPicksSession(["fork", "--last"])).toBe(true);
  });
});

const ocspec = (over: Partial<AgentSpec> = {}): AgentSpec => ({ ...spec(), program: "opencode", badge: "opencode", ...over });

describe("opencodeSessionArgs", () => {
  it("resumes the agent's own session when opencode still has it", async () => {
    const s = ocspec({ sessionId: "ses_abc123" });
    expect(await opencodeSessionArgs(s, async () => true)).toEqual(["--session", "ses_abc123"]);
    expect(s.sessionId).toBe("ses_abc123");
  });

  it("boots fresh when the session is gone, forgetting the dead id", async () => {
    const s = ocspec({ sessionId: "ses_abc123" });
    expect(await opencodeSessionArgs(s, async () => false)).toEqual([]);
    expect(s.sessionId).toBeUndefined();
    expect(await opencodeSessionArgs(ocspec(), async () => true)).toEqual([]);
  });

  it("spots args that already choose a session", () => {
    expect(opencodePicksSession([])).toBe(false);
    expect(opencodePicksSession(["--no-replay"])).toBe(false);
    expect(opencodePicksSession(["--continue"])).toBe(true);
    expect(opencodePicksSession(["-s", "ses_x"])).toBe(true);
    expect(opencodePicksSession(["--session=ses_x"])).toBe(true);
    expect(opencodePicksSession(["run", "hi"])).toBe(true);
  });
});
