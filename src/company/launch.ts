/* What a member of the company is launched with. Claude only for now; Codex
 * and OpenCode have different flags and come later. */
import { FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS } from "../laws";
import { personById } from "./org";
import type { Company, Person, RoleDef } from "./types";

const LAWS = { founder: FOUNDER_LAWS, lead: LEAD_LAWS, staff: STAFF_LAWS };

export function composePrompt(person: Person, role: RoleDef, c: Company): string {
  const boss = person.boss ? personById(c, person.boss)?.name : null;
  const floor = c.floors.find((f) => f.id === person.floor)?.name;
  const place = [
    `Your name is ${person.name}.`,
    boss ? `Your boss is ${boss}.` : "You answer only to the person who owns the company.",
    floor ? `You work on the ${floor} floor.` : "",
  ].filter(Boolean).join(" ");
  return `${LAWS[person.rank]}\n\n${place}\n\n${role.prompt}`;
}

export function buildRoleLaunch(o: { person: Person; role: RoleDef; promptFile: string; pluginDir: string | null }): {
  args: string[];
  env: Array<[string, string]>;
} {
  const args = ["--append-system-prompt-file", o.promptFile];
  if (o.pluginDir) args.push("--plugin-dir", o.pluginDir);
  if (o.role.denyTools.length) args.push("--disallowedTools", o.role.denyTools.join(","));
  return { args, env: [["BYTEHIVE_AGENT_ID", o.person.id]] };
}

export function pluginManifest(person: Person): string {
  const safe = person.id.replace(/[^a-z0-9]+/gi, "-").replace(/^-+|-+$/g, "").toLowerCase();
  return JSON.stringify({ name: `bytehive-role-${safe}`, version: "1.0.0" }, null, 2);
}
