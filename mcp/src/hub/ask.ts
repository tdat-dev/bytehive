/* Asking the user about a held browser step while ByteHive is closed: an MCP
 * elicitation, which the client shows to the person (the model never sees or
 * answers it). A client that can't show one, or any failure, is a "no". */

import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";

/** As long as a held step may wait (HOLD_MS in hub.ts). */
const ASK_TIMEOUT_MS = 10 * 60_000;

export async function askUser(server: McpServer, what: string, url: string): Promise<boolean | "cannot-ask"> {
  if (!server.server.getClientCapabilities()?.elicitation) return "cannot-ask";
  try {
    const r = await server.server.elicitInput(
      {
        message: `ByteHive is closed, so the agent asks you here before this step in your Chrome:\n\n${what}${url ? `\non ${url}` : ""}\n\nAllow it?`,
        requestedSchema: {
          type: "object",
          properties: { allow: { type: "boolean", title: "Allow", description: "Yes lets the agent do this one step." } },
          required: ["allow"],
        },
      },
      { timeout: ASK_TIMEOUT_MS },
    );
    return r.action === "accept" && r.content?.allow === true;
  } catch {
    return "cannot-ask";
  }
}
