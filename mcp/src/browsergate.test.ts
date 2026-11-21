import { describe, expect, it } from "vitest";
import { argsLine, browserMode, gateDescription, runGate } from "./browsergate.js";
import { browserTools } from "./browsertools.js";

const calls: Array<[string, Record<string, unknown>]> = [];
const hub = { call: async (tool: string, args: Record<string, unknown>) => (calls.push([tool, args]), { content: [{ type: "text" as const, text: `did ${tool}` }] }) };
const tools = browserTools(hub, "D:/p", "Ana");

describe("the single browser tool", () => {
  it("is for every client but Claude Code, unless told", () => {
    expect(browserMode("claude-code", undefined)).toBe("each");
    expect(browserMode("codex-mcp-client", undefined)).toBe("one");
    expect(browserMode(undefined, undefined)).toBe("one");
    expect(browserMode("claude-code", "one")).toBe("one");
    expect(browserMode("opencode", " EACH ")).toBe("each");
  });

  it("names every action with its arguments in a line", () => {
    const d = gateDescription(tools);
    expect(tools).toHaveLength(19);
    expect(d).toContain("- navigate {url, tabId?}: ");
    expect(d).toContain("- list: connected browsers");
    expect(d.split("\n")).toHaveLength(tools.length + 1);
  });

  it("is a fraction of the separate schemas", () => {
    const each = JSON.stringify(tools.map((t) => ({ name: t.name, description: t.description, schema: Object.keys(t.inputSchema) })));
    expect(gateDescription(tools).length).toBeLessThan(each.length * 0.6);
  });

  it("runs the action with its checked args", async () => {
    calls.length = 0;
    const r = await runGate(tools, "navigate", { url: "https://a.b" });
    expect(r.isError).toBeUndefined();
    expect(calls).toEqual([["navigate", { url: "https://a.b" }]]);
  });

  it("hands a wrong call the action's schema, and runs nothing", async () => {
    calls.length = 0;
    const r = await runGate(tools, "computer", { action: "fly" });
    const said = (x: { content: unknown[] }) => (x.content[0] as { text: string }).text;
    expect(r.isError).toBe(true);
    expect(said(r)).toContain("Bad args for computer");
    expect(said(r)).toContain("action: one of screenshot, left_click");
    expect(said(await runGate(tools, "teleport", {}))).toContain("No browser action \"teleport\"");
    expect(calls).toEqual([]);
  });

  it("writes optional args with a question mark", () => {
    expect(argsLine(tools.find((t) => t.name === "tabs")!.inputSchema)).toBe("all?");
  });
});
