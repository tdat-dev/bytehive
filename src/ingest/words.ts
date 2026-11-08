/** How the gate talks to a person, shared by the toast and the panel. */
import { money } from "../pricing";
import type { Source, ToolResult } from "./types";

export const TOOLS: Record<string, string> = {
  estimate_cost: "Cost estimate",
  convert_currency: "Currency",
  remaining_budget: "Budget left",
};

/** What a computed answer says, in the words a person would read it. */
export function answerText(r: ToolResult): string {
  if (r.status === "error") return `Failed: ${r.error ?? "no answer"}`;
  if (r.metadata.tool === "estimate_cost" && typeof r.values.usd === "number") return money(r.values.usd);
  return Object.entries(r.values)
    .map(([k, v]) => `${k} ${typeof v === "number" ? Math.round(v * 1e6) / 1e6 : v}`)
    .join(", ");
}

/** The answer with its name in front: "Cost estimate $0.32". */
export const answerLine = (r: ToolResult): string => `${TOOLS[r.metadata.tool] ?? r.metadata.tool} ${answerText(r)}`;

/** Who sent it: Kanban and the phone are you; an unnamed agent is not. */
export function senderName(from: string | undefined, source: Source): string {
  if (from) return from;
  return source === "kanban" || source === "phone" ? "You" : "An agent";
}

/** A held message as the toast quotes it. */
export const heldLine = (text: string): string => `held back “${text.trim()}”`;
