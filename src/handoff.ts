// Hand an agent's task to an agent of another CLI: when one stops at its
// plan's usage limit (or you'd rather another CLI carried on), a new agent
// starts in the same folder with a brief written from the first one's
// conversation: what it was asked, its plan, the files it changed, and where
// it got to. The work itself is already in the folder; the brief is what the
// new agent needs to pick it up without starting over.

import type { Chat } from "./chatmodel";

/** Most of each of your messages carried over; the brief stays readable. */
const ASK_CHARS = 700;
const LAST_WORDS_CHARS = 1500;
const MAX_ASKS = 6;
const MAX_FILES = 30;

const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1).trimEnd()}…` : s);
const oneBlock = (s: string) => s.replace(/\r\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();

export interface HandoffFrom {
  /** The agent handing over ("Ana"). */
  name: string;
  /** Its CLI ("Claude Code"). */
  cli: string;
  /** Why it is handing over ("it hit its usage limit"). */
  why: string;
  /** What Maestro knows it was asked, when there is no conversation to read. */
  title?: string | null;
  /** Its first asks, when `chat` holds only the end of a long conversation. */
  firstAsks?: string[];
}

/** The brief a new agent starts with. `chat` is the first agent's
 *  conversation, when its CLI's is readable; without one the brief says what
 *  Maestro knows and asks the new agent to look at the folder. */
export function handoffBrief(from: HandoffFrom, chat: Chat | null): string {
  const parts: string[] = [];
  parts.push(`You're taking over a task from ${from.name} (${from.cli}), another coding agent working in this same folder. It stopped because ${from.why}. Its work so far is already in the folder.`);
  const later = (chat?.items ?? []).filter((i) => i.kind === "user").map((i) => ("text" in i ? oneBlock(i.text) : "")).filter(Boolean);
  const first = (from.firstAsks ?? []).map(oneBlock).filter((a) => a && !later.includes(a));
  const asks = [...first, ...later];
  if (asks.length) {
    // the first ask says what the task is; the latest ones where it went
    const shown = asks.length > MAX_ASKS ? [asks[0], ...asks.slice(-(MAX_ASKS - 1))] : asks;
    parts.push(`What it was asked, in order${asks.length > shown.length ? ` (the first, then the latest ${shown.length - 1} of ${asks.length})` : ""}:\n${shown.map((a, k) => `${k + 1}. ${clip(a, ASK_CHARS).replace(/\n/g, "\n   ")}`).join("\n")}`);
  } else if (from.title) {
    parts.push(`Its task: ${from.title}`);
  }
  const todos = chat?.meta.todos ?? [];
  if (todos.length) {
    const mark = { completed: "done", in_progress: "doing", pending: "to do" } as const;
    parts.push(`Its plan when it stopped:\n${todos.map((t) => `- [${mark[t.state]}] ${t.text}`).join("\n")}`);
  }
  const files = chat?.meta.files ?? [];
  if (files.length) {
    const shown = files.slice(0, MAX_FILES).map((f) => `- ${f.path}${f.deleted ? " (deleted)" : f.isNew ? " (new)" : ""}`);
    parts.push(`Files it changed${files.length > MAX_FILES ? ` (the latest ${MAX_FILES} of ${files.length})` : ""}:\n${shown.join("\n")}`);
  }
  const last = [...(chat?.items ?? [])].reverse().find((i) => i.kind === "text");
  if (last && "text" in last && last.text.trim()) {
    parts.push(`Its last words:\n${clip(oneBlock(last.text), LAST_WORDS_CHARS).split("\n").map((l) => `> ${l}`).join("\n")}`);
  }
  parts.push(
    chat && (asks.length || files.length)
      ? "Look at the current state of the files first (git status and the files above), then carry on and finish what's left. Don't redo what's already done, and say briefly what you picked up from before you start."
      : "Its conversation couldn't be read, so look at the folder first (git status, recent changes) to see where it got to, then carry on and finish the task.",
  );
  return parts.join("\n\n");
}
