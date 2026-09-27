import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ runs: 0, out: "" }));
vi.mock("./ipc", () => ({ runCapture: async () => { io.runs++; return io.out; } }));

import { cliFacts, contextFill, parseClaudeInit, profileOf } from "./cliprofile";

const init = (o: object) => JSON.stringify({ type: "system", subtype: "init", ...o });

describe("cliprofile", () => {
  beforeEach(() => { io.runs = 0; });

  it("reads Claude Code's own commands, telling skills and plugins apart", () => {
    const facts = parseClaudeInit([
      JSON.stringify({ type: "system", subtype: "hook_started" }),
      init({ model: "claude-opus-5-5[1m]", slash_commands: ["compact", "impeccable", "codex:rescue"], skills: [{ name: "impeccable" }] }),
    ].join("\n"));
    expect(facts).toEqual({
      model: "claude-opus-5-5[1m]",
      commands: [{ name: "compact", kind: "command" }, { name: "impeccable", kind: "skill" }, { name: "codex:rescue", kind: "plugin" }],
    });
    expect(parseClaudeInit("nothing here")).toBeNull();
  });

  it("asks the CLI once per folder and keeps the answer", async () => {
    io.out = init({ slash_commands: ["review"] });
    const a = await cliFacts("claude", "claude", "D:/one")!;
    await cliFacts("claude", "claude", "D:/one");
    expect(io.runs).toBe(1);
    await cliFacts("claude", "claude", "D:/two");
    expect(io.runs).toBe(2);
    expect(a.commands.map((c) => c.name)).toEqual(["review"]);
    await cliFacts("claude", "claude", "D:/one", true);
    expect(io.runs).toBe(3);
  });

  it("leaves CLIs that can't list their commands to their own menu, with their own model command", () => {
    expect(cliFacts("codex", "codex", "D:/one")).toBeNull();
    expect(profileOf("codex").modelCommand).toBe("/model");
    expect(profileOf("opencode").modelCommand).toBe("/models");
    expect(profileOf("aider").modelCommand).toBeUndefined();
  });
});

describe("how full an agent's context is", () => {
  it("reads Claude against 200k, or a million for a [1m] model or one already past 200k", () => {
    expect(contextFill("claude", 100_000, null, "claude-opus-5-5")).toBe(50);
    expect(contextFill("claude", 100_000, null, "claude-opus-5-5[1m]")).toBe(10);
    expect(contextFill("claude", 300_000, null, "claude-sonnet-5")).toBe(30);
    // what the conversation says wins over the folder's default
    expect(contextFill("claude", 100_000, 200_000, "claude-opus-5-5[1m]")).toBe(50);
  });

  it("uses the window a CLI reports, and can't tell without one", () => {
    expect(contextFill("codex", 136_000, 272_000, null)).toBe(50);
    expect(contextFill("codex", 136_000, null, null)).toBeNull();
    expect(contextFill("claude", 0, null, null)).toBeNull();
  });
});
