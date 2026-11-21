/* The browser tools as one tool, `browser`, for clients that send every tool's
 * schema with every request (Codex, OpenCode, Antigravity): 19 schemas were
 * about 9 KB of each request even when the agent never opened a page. The
 * single tool names each action with its arguments in a line; the arguments
 * are still checked against the action's own schema, and a wrong call gets
 * that schema back. Measured 2026-11-21: Codex does not pick up tools added
 * mid-session (tools/list_changed), so lazy loading was not an option. */

import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import type { BrowserTool } from "./browsertools.js";

/** `url, tabId?` for an action's schema. */
export function argsLine(schema: Record<string, z.ZodTypeAny>): string {
  return Object.entries(schema)
    .map(([k, v]) => (v.isOptional() ? `${k}?` : k))
    .join(", ");
}

export function gateDescription(tools: BrowserTool[]): string {
  const lines = tools.map((t) => {
    const a = argsLine(t.inputSchema);
    return `- ${t.name}${a ? ` {${a}}` : ""}: ${t.brief}`;
  });
  return `The user's real Chrome, in your own tab group. Pick an action and pass its args (tabId defaults to your most recent tab; refs come from read_page or find):\n${lines.join("\n")}`;
}

const failure = (text: string) => ({ content: [{ type: "text" as const, text }], isError: true });

/** What a call with bad args is told: the action's whole schema. */
function schemaOf(t: BrowserTool): string {
  const shape = Object.entries(t.inputSchema).map(([k, v]) => `${k}${v.isOptional() ? " (optional)" : ""}: ${v.description ?? describeType(v)}`);
  return `${t.name}: ${t.description}${shape.length ? `\nargs:\n  ${shape.join("\n  ")}` : ""}`;
}

function describeType(v: z.ZodTypeAny): string {
  const inner = v instanceof z.ZodOptional ? v.unwrap() : v;
  if (inner instanceof z.ZodEnum) return `one of ${(inner.options as string[]).join(", ")}`;
  if (inner instanceof z.ZodArray) return "array";
  if (inner instanceof z.ZodUnion) return "string, number or boolean";
  return inner._def.typeName.replace(/^Zod/, "").toLowerCase();
}

/** Run one action of the single tool. */
export async function runGate(tools: BrowserTool[], action: string, args: unknown) {
  const t = tools.find((x) => x.name === action);
  if (!t) return failure(`No browser action "${action}". Actions: ${tools.map((x) => x.name).join(", ")}`);
  const parsed = z.object(t.inputSchema).safeParse(args ?? {});
  if (!parsed.success) {
    const why = parsed.error.issues.map((i) => `${i.path.join(".") || "args"}: ${i.message}`).join("; ");
    return failure(`Bad args for ${action} (${why}).\n${schemaOf(t)}`);
  }
  return t.run(parsed.data);
}

export function registerBrowserGate(server: McpServer, tools: BrowserTool[]) {
  const names = tools.map((t) => t.name) as [string, ...string[]];
  server.registerTool(
    "browser",
    {
      description: gateDescription(tools),
      inputSchema: {
        action: z.enum(names),
        args: z.record(z.unknown()).optional().describe("The action's arguments, as listed"),
      },
    },
    async ({ action, args }) => (await runGate(tools, action, args)) as never,
  );
}

/** each: one tool per action (browser_navigate, ...). one: the single browser tool. */
export type BrowserMode = "each" | "one";

/** Claude Code loads an MCP tool's schema only when it needs it, so it keeps the
 *  separate tools; every other client gets the one. BYTEHIVE_BROWSER_TOOLS
 *  (each or one) overrides. */
export function browserMode(client: string | undefined, forced: string | undefined = process.env.BYTEHIVE_BROWSER_TOOLS): BrowserMode {
  const f = forced?.trim().toLowerCase();
  if (f === "each" || f === "one") return f;
  return client === "claude-code" ? "each" : "one";
}

export function registerBrowser(server: McpServer, tools: BrowserTool[], mode: BrowserMode) {
  if (mode === "one") return registerBrowserGate(server, tools);
  for (const t of tools)
    server.registerTool(`browser_${t.name}`, { description: t.description, inputSchema: t.inputSchema }, async (args: Record<string, unknown>) => (await t.run(args)) as never);
}
