/* Agent progress reports for the fleet dashboard: one small file per agent
 * at <workspace>/.bytehive/summary-<slug>.md, written through the summary_write
 * tool. The Supervisor watch loop reads these files — no model, no key.
 * The slug algorithm MUST match bytehive-supervisor's selfsummary.ts, or the
 * dashboard reads a different filename than the agent writes. */

import fs from "node:fs";
import path from "node:path";

export interface SummaryBody {
  doing: string;
  blocked: string;
  next: string;
}

/** Pane name → filename slug: lowercase, runs of non-alphanumerics become "-". */
export function slugPane(name: string): string {
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || "agent";
}

export function summaryPath(dir: string, agentName: string): string {
  return path.join(dir, ".bytehive", `summary-${slugPane(agentName)}.md`);
}

/** Overwrite the agent's summary file (latest wins). Creates .bytehive/ on
 *  first use. Throws on an empty report — a blank status helps nobody. */
export function writeSummary(
  dir: string,
  agentName: string,
  body: { doing: string; blocked: string; next: string },
): { path: string; agent: string } {
  const doing = body.doing.trim();
  const blocked = body.blocked.trim();
  const next = body.next.trim();
  if (!doing || !blocked || !next) {
    throw new Error("doing, blocked and next must all be non-empty (write blocked as no when unblocked)");
  }
  const agent = agentName.trim();
  if (!agent) throw new Error("agent must not be empty");
  const p = summaryPath(dir, agent);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  const text = `Doing: ${doing}\nBlocked: ${blocked}\nNext: ${next}\n`;
  fs.writeFileSync(p, text, "utf8");
  return { path: p, agent };
}
