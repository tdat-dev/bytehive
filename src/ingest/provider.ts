/** Turn the provider's answer into the two-field result. The model may wrap
 *  JSON in a code fence; anything else is a miss, never a guess. */
import type { Classified } from "./types";

export function parseClassified(content: string): Classified | null {
  let text = content.trim();
  const fence = text.match(/^```(?:json)?\s*([\s\S]*?)\s*```$/i);
  if (fence) text = fence[1].trim();
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return null;
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const o = value as Record<string, unknown>;
  if (typeof o.is_actionable !== "boolean") return null;
  if (o.data !== null && (typeof o.data !== "object" || Array.isArray(o.data))) return null;
  return { is_actionable: o.is_actionable, data: (o.data as Record<string, unknown> | null) ?? null };
}
