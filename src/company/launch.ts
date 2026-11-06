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

/** Where a Founder with no pane starts: the active project, else the first open
 *  project with a folder. It works through MCP tools, so the folder limits nothing. */
export function founderFolder(active: { dir: string | null } | null, open: Array<{ dir: string | null }>): string | null {
  return active?.dir ?? open.find((w) => w.dir)?.dir ?? null;
}

const NEEDS_NODE = "ByteHive needs Node.js 22.13 or newer to run the Company.";

/** What `node --version` says about giving a job, as it prints v22.13.0. The MCP
 *  server needs that, from mcp/package.json. Only a clear answer refuses: a version
 *  too old, or no output from a run that failed. Anything else says nothing about Node. */
export function nodeVerdict(r: { printed: string; code: number | null } | { failed: string }): { refuse?: string; log?: string } {
  if ("failed" in r) return { log: `the Node.js check failed: ${r.failed}` };
  const printed = r.printed.trim();
  if (!printed) {
    if (r.code === 0 || r.code === null) return { log: `the Node.js check printed nothing, exit code ${r.code}` };
    return { refuse: `${NEEDS_NODE} Node.js was not found. If you just installed it, restart ByteHive.` };
  }
  const v = /^v(\d+)\.(\d+)\./.exec(printed);
  if (!v) return { log: `the Node.js check printed something else: ${printed}` };
  const [major, minor] = [Number(v[1]), Number(v[2])];
  return major > 22 || (major === 22 && minor >= 13) ? {} : { refuse: `${NEEDS_NODE} Found ${printed}.` };
}

/** The Founder's settings file: its hooks stay off, everyone else keeps the owner's. */
export const FOUNDER_SETTINGS = JSON.stringify({ disableAllHooks: true });

/** No --strict-mcp-config: the owner's other servers keep working. When they have
 *  a `bytehive` of their own, Claude starts this one instead (run to find out). */
export function buildRoleLaunch(o: { person: Person; role: RoleDef; promptFile: string; mcpConfig: string; pluginDir: string | null; settings?: string | null }): {
  args: string[];
  env: Array<[string, string]>;
} {
  const args = ["--append-system-prompt-file", o.promptFile, "--mcp-config", o.mcpConfig];
  if (o.pluginDir) args.push("--plugin-dir", o.pluginDir);
  if (o.role.denyTools.length) args.push("--disallowedTools", o.role.denyTools.join(","));
  if (o.settings) args.push("--settings", o.settings);
  return { args, env: [["BYTEHIVE_AGENT_ID", o.person.id]] };
}

/** Starts the server the app carries, so a member's tools match the app on any machine. */
export function mcpConfig(server: string): string {
  return JSON.stringify({ mcpServers: { bytehive: { command: "node", args: [server] } } }, null, 2);
}

export function pluginManifest(person: Person): string {
  const safe = person.id.replace(/[^a-z0-9]+/gi, "-").replace(/^-+|-+$/g, "").toLowerCase();
  return JSON.stringify({ name: `bytehive-role-${safe}`, version: "1.0.0" }, null, 2);
}
