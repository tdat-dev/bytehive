// What a conversation comes down to for another agent: what it was asked, its
// plan, the files it changed and its last words. Pure, for both the app (a
// handoff brief) and maestro-mcp (agent_context), which reads the same CLI
// conversations with the same parsers.

import type { Chat, FileChange, Todo } from "./chatmodel";

/** Most of each of your messages carried over; the digest stays readable. */
const ASK_CHARS = 700;
const LAST_WORDS_CHARS = 1500;
const MAX_ASKS = 6;
const MAX_FILES = 30;

export const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1).trimEnd()}…` : s);
export const oneBlock = (s: string) => s.replace(/\r\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();

export interface Digest {
  /** Everything it was asked, in order. */
  asks: string[];
  /** The plan as it stands. */
  todos: Todo[];
  /** Newest change first. */
  files: FileChange[];
  /** Its latest reply. */
  lastWords: string;
}

/** `firstAsks`: the start of a long conversation, when `chat` holds only its end. */
export function digestOf(chat: Chat | null, firstAsks: string[] = []): Digest {
  const later = (chat?.items ?? []).filter((i) => i.kind === "user").map((i) => ("text" in i ? oneBlock(i.text) : "")).filter(Boolean);
  const first = firstAsks.map(oneBlock).filter((a) => a && !later.includes(a));
  const last = [...(chat?.items ?? [])].reverse().find((i) => i.kind === "text");
  return {
    asks: [...first, ...later],
    todos: chat?.meta.todos ?? [],
    files: chat?.meta.files ?? [],
    lastWords: last && "text" in last ? last.text.trim() : "",
  };
}

/** The first ask says what the task is; the latest ones where it went. */
export function asksSection(asks: string[], intro: string): string {
  if (!asks.length) return "";
  const shown = asks.length > MAX_ASKS ? [asks[0], ...asks.slice(-(MAX_ASKS - 1))] : asks;
  const note = asks.length > shown.length ? ` (the first, then the latest ${shown.length - 1} of ${asks.length})` : "";
  return `${intro}${note}:\n${shown.map((a, k) => `${k + 1}. ${clip(a, ASK_CHARS).replace(/\n/g, "\n   ")}`).join("\n")}`;
}

const MARK = { completed: "done", in_progress: "doing", pending: "to do" } as const;

export function planSection(todos: Todo[], intro: string): string {
  return todos.length ? `${intro}\n${todos.map((t) => `- [${MARK[t.state]}] ${t.text}`).join("\n")}` : "";
}

/** "3/7 done", or "" when it made no plan. */
export function planProgress(todos: Todo[]): string {
  return todos.length ? `${todos.filter((t) => t.state === "completed").length}/${todos.length} done` : "";
}

export function filesSection(files: FileChange[], intro: string): string {
  if (!files.length) return "";
  const shown = files.slice(0, MAX_FILES).map((f) => `- ${f.path}${f.deleted ? " (deleted)" : f.isNew ? " (new)" : ""}`);
  return `${intro}${files.length > MAX_FILES ? ` (the latest ${MAX_FILES} of ${files.length})` : ""}:\n${shown.join("\n")}`;
}

export function lastWordsSection(text: string, intro: string): string {
  return text ? `${intro}\n${clip(oneBlock(text), LAST_WORDS_CHARS).split("\n").map((l) => `> ${l}`).join("\n")}` : "";
}
