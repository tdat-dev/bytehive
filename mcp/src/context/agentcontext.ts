/* agent_context: what every coding agent did in this project, from their own
 * conversations. Without `who` a list (one line each); with it one agent's
 * full context: what it was asked, its plan, the files it changed, its last
 * words. Agents ByteHive runs get their names from fleet.json; others show
 * their conversation id. */

import { asksSection, clip, digestOf, filesSection, lastWordsSection, oneBlock, planProgress, planSection, type Digest } from "../app/convdigest";
import { readFleet, type FleetAgent } from "../fleet.js";
import { findConversations, projectFolders, readConversation, sameDir, type Cli, type Conversation, type Env } from "./stores.js";

const LABEL: Record<Cli, string> = { claude: "Claude Code", codex: "Codex", opencode: "OpenCode" };
const WORDS: Record<string, Cli> = { claude: "claude", "claude code": "claude", codex: "codex", opencode: "opencode", "open code": "opencode" };
/** A run's conversation began a little before ByteHive saw it start. */
const SLACK_MS = 5000;

export const shortId = (c: Conversation) => c.id.slice(0, 8);

export function ago(ms: number, now: number): string {
  const min = Math.round((now - ms) / 60_000);
  if (min < 1) return "just now";
  if (min < 60) return `${min} min ago`;
  const h = Math.round(min / 60);
  if (h < 48) return `${h} h ago`;
  return `${Math.round(h / 24)} days ago`;
}

/** The ByteHive name of a conversation's agent: the agent that says it is in
 *  it, else the agent of that CLI in that folder whose run it is the newest
 *  conversation of. */
export function nameOf(c: Conversation, fleet: FleetAgent[], all: Conversation[]): string | null {
  const own = fleet.find((a) => a.session && (a.session === c.id || a.session === c.path));
  if (own) return own.name;
  for (const a of fleet) {
    if (a.session || a.cli !== c.cli || !a.folder || !sameDir(a.folder, c.folder)) continue;
    const since = (a.since ?? 0) - SLACK_MS;
    const newest = all
      .filter((x) => x.cli === c.cli && sameDir(x.folder, c.folder) && x.updated >= since)
      .sort((x, y) => y.updated - x.updated)[0];
    if (newest === c) return a.name;
  }
  return null;
}

/** An agent name, a conversation id (4+ characters of it), or a CLI (its newest). */
export function resolveWho(who: string, convs: Conversation[], fleet: FleetAgent[]): Conversation | null {
  const w = who.trim().replace(/^\[|\]$/g, "").toLowerCase();
  if (!w) return null;
  const named = fleet.find((a) => a.name.trim().toLowerCase() === w);
  if (named) {
    const c = convs.find((x) => nameOf(x, fleet, convs) === named.name);
    if (c) return c;
  }
  if (w.length >= 4) {
    const c = convs.find((x) => x.id.toLowerCase().startsWith(w));
    if (c) return c;
  }
  const cli = WORDS[w];
  return cli ? convs.find((x) => x.cli === cli) ?? null : null;
}

const stateOf = (c: Conversation) => (c.running === true ? "running, " : c.running === false ? "stopped, " : "");

export function listLine(c: Conversation, d: Digest, name: string | null, dir: string, now: number): string {
  const task = d.asks[0] ? ` Task: ${clip(oneBlock(d.asks[0]).replace(/\s+/g, " "), 120)}.` : "";
  const plan = planProgress(d.todos);
  return `- ${name ? `${name} ` : ""}[${shortId(c)}] ${LABEL[c.cli]}, ${stateOf(c)}active ${ago(c.updated, now)}.${task}${plan ? ` Plan ${plan}.` : ""}${sameDir(c.folder, dir) ? "" : ` Folder: ${c.folder}`}`;
}

export function detailText(c: Conversation, d: Digest, name: string | null, now: number): string {
  const who = name ?? `${/^[AEIOU]/.test(LABEL[c.cli]) ? "An" : "A"} ${LABEL[c.cli]} agent`;
  const parts = [
    `${who} [${shortId(c)}] (${LABEL[c.cli]}), ${stateOf(c)}started ${ago(c.started, now)}, last active ${ago(c.updated, now)}.\nFolder: ${c.folder}`,
    asksSection(d.asks, "What it was asked, in order"),
    planSection(d.todos, "Its plan:"),
    filesSection(d.files, "Files it changed"),
    lastWordsSection(d.lastWords, "Its last words:"),
  ];
  if (!d.asks.length && !d.lastWords) parts.push("Its conversation has no messages yet.");
  if (d.todos.some((t) => t.state !== "completed")) {
    parts.push(`To carry this on: work in ${c.folder}, check git status there, then finish the open items of its plan. Don't redo what is done.`);
  }
  return parts.filter(Boolean).join("\n\n");
}

/** The fleets ByteHive published in any of the project's folders. */
function fleetOf(folders: string[]): FleetAgent[] {
  return folders.flatMap((f) => readFleet(f));
}

export async function agentContext(dir: string, who: string | undefined, env: Env): Promise<string> {
  const folders = projectFolders(dir);
  const { list, notes } = await findConversations(folders, env);
  const fleet = fleetOf(folders);
  const days = env.days ?? 14;
  if (who?.trim()) {
    const c = resolveWho(who, list, fleet);
    if (!c) {
      return `No agent "${who}" here. ${list.length ? "These are the conversations in this project:\n" + (await listing(list, fleet, dir, env)) : `No conversations in this project in the last ${days} days.`}`;
    }
    const { chat, firstAsks } = await readConversation(c, env);
    return detailText(c, digestOf(chat, firstAsks), nameOf(c, fleet, list), env.now);
  }
  if (!list.length) {
    return [`No conversations from Claude Code, Codex or OpenCode in this project in the last ${days} days.`, ...notes].join("\n");
  }
  return [
    "Conversations in this project (newest first):",
    await listing(list, fleet, dir, env),
    ...notes,
    "",
    "Call agent_context with who set to a name or an [id] to read one agent's full context.",
  ].join("\n");
}

async function listing(list: Conversation[], fleet: FleetAgent[], dir: string, env: Env): Promise<string> {
  const lines: string[] = [];
  for (const c of list) {
    const { chat, firstAsks } = await readConversation(c, env);
    lines.push(listLine(c, digestOf(chat, firstAsks), nameOf(c, fleet, list), dir, env.now));
  }
  return lines.join("\n");
}
