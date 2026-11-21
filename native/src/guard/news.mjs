// ByteHive news for an agent, at the start of each turn: what changed on the
// board and in the project memory since its last turn, so it learns that the
// person approved a card or another agent saved a fact without calling
// board_get or memory_get to look. Given per run beside the guards (guard.rs):
//
//   node news.mjs prompt   UserPromptSubmit: say what changed, in a few lines.
//   node news.mjs stop     Stop: take in the turn's own changes, so the agent is
//                          not told about the cards it moved itself.
//
// A change someone else makes during the agent's turn is taken in with it and
// not told; the next board_get shows it. The first turn says nothing: the laws
// have the agent read the board then anyway.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** Lines told at most; the rest is counted. */
export const MAX_LINES = 8;

const readJson = (file) => {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8").replace(/^﻿/, ""));
  } catch {
    return null;
  }
};

const cut = (s, n) => {
  const one = String(s ?? "").replace(/\s+/g, " ").trim();
  return one.length > n ? `${one.slice(0, n)}…` : one;
};

/** What is compared from turn to turn: each card's title and list, each fact's text. */
export function snapshotOf(board, memory) {
  const cards = {};
  for (const l of board?.lists ?? []) {
    for (const c of l.cards ?? []) {
      if (c?.id) cards[c.id] = { t: String(c.title ?? ""), l: String(l.title ?? ""), by: c.done?.by ?? c.assignee ?? "" };
    }
  }
  const facts = {};
  for (const e of memory?.entries ?? []) {
    if (e?.id) facts[e.id] = { t: String(e.text ?? ""), by: String(e.by ?? "") };
  }
  return { cards, facts };
}

/** The lines to tell `me` about what moved from `prev` to `now`. */
export function newsLines(prev, now, me = "") {
  if (!prev) return [];
  const out = [];
  for (const [id, c] of Object.entries(now.cards)) {
    const was = prev.cards[id];
    if (!was) out.push(`new card [${id}] "${cut(c.t, 80)}" in ${c.l}`);
    else if (was.l !== c.l && !(me && c.by === me)) out.push(`card [${id}] "${cut(c.t, 80)}" moved from ${was.l} to ${c.l}${c.by ? ` (${c.by})` : ""}`);
  }
  for (const [id, c] of Object.entries(prev.cards)) {
    if (!now.cards[id]) out.push(`card [${id}] "${cut(c.t, 80)}" was deleted`);
  }
  for (const [id, f] of Object.entries(now.facts)) {
    if (me && f.by === me) continue;
    const was = prev.facts[id];
    if (!was) out.push(`memory [${id}] new: ${cut(f.t, 160)}`);
    else if (was.t !== f.t) out.push(`memory [${id}] changed: ${cut(f.t, 160)}`);
  }
  return out;
}

/** The context the agent gets, or null when nothing changed. */
export function newsText(lines) {
  if (!lines.length) return null;
  const shown = lines.slice(0, MAX_LINES);
  const more = lines.length - shown.length;
  return `ByteHive news since your last turn (the board and the project memory, changed by someone else):\n${shown.map((l) => `- ${l}`).join("\n")}${more ? `\n- and ${more} more: see board_get` : ""}`;
}

const stateFile = (event) => {
  const safe = String(event.session_id ?? "none").replace(/[^\w-]/g, "").slice(0, 80) || "none";
  return path.join(os.tmpdir(), "bytehive-news", `${safe}.json`);
};

/** The project's board and memory now. */
function current(workspace) {
  const dir = path.join(workspace, ".bytehive");
  return snapshotOf(readJson(path.join(dir, "board.json")), readJson(path.join(dir, "memory.json")));
}

async function main(kind) {
  let raw = "";
  for await (const chunk of process.stdin) raw += chunk;
  let event = {};
  try {
    event = JSON.parse(raw);
  } catch {}
  const workspace = process.env.BYTEHIVE_WORKSPACE || String(event.cwd ?? process.cwd());
  if (!fs.existsSync(path.join(workspace, ".bytehive"))) return;
  const file = stateFile(event);
  const now = current(workspace);
  if (kind === "prompt") {
    const text = newsText(newsLines(readJson(file), now, process.env.BYTEHIVE_AGENT ?? ""));
    if (text) process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: text } }));
  }
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(now));
}

// Run as the hook, not when a test imports it. News that fails costs the agent nothing.
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv[2]).catch(() => {});
}
