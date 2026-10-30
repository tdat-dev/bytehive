// What the Grid's cards say about the agents beside its terminal.
//
// A card is plain text that wraps at any width, so it stays readable where a
// terminal of the same size would be mush. It says what the agent last said
// and the step it is on, read from its CLI's own conversation file (the same
// reading History uses, chatsource.ts), a little at a time: the first read
// takes the end of the file, later ones only what was added. The same feed
// serves stuck detection and the review drawer, so feeds live with their
// panes and are forgotten when panes close, not when cards go away.

import { sourceOf } from "./chatsource";
import { createChat } from "./chatmodel";
import type { Chat, ChatItem } from "./chatmodel";
import type { Pane } from "./panetypes";

export interface Glance {
  /** The end of its last reply, as plain text. */
  said: string;
  /** The step it is on ("Edited billing.ts"), while one is open after that reply. */
  doing: string;
  /** What you last asked it, while it hasn't answered yet. */
  asked: string;
}

const EMPTY: Glance = { said: "", doing: "", asked: "" };
/** How much of a reply a card keeps: its end, where the conclusion is. */
export const SAID_MAX = 360;
/** The first read of a long conversation starts this far from its end. */
const FIRST_READ = 256 * 1024;
/** Most conversation a card feed keeps: the tail is all the card, the
 *  stuck watch and the digest ever read (the head survives as firstAsks). */
export const FEED_KEEP = 600;
/** How often running agents' conversations are re-read (cards and the task tick share it). */
export const FEED_READ_MS = 2000;

/** Markdown to the words it shows: no fences, stars, backticks or heading marks. */
export function plain(md: string): string {
  return md
    .replace(/```[^\n]*\n?/g, "")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/\*\*([^*]+)\*\*|__([^_]+)__/g, "$1$2")
    .replace(/^\s{0,3}#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "• ")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .split("\n").map((l) => l.trimEnd()).filter((l, i, all) => l || (i > 0 && all[i - 1])).join("\n")
    .trim();
}

/** The end of a text, cut at a word, with "…" where it was cut. */
export function tail(text: string, max = SAID_MAX): string {
  if (text.length <= max) return text;
  const cut = text.slice(text.length - max);
  const at = cut.search(/\s/);
  return `…${(at >= 0 && at < 40 ? cut.slice(at) : cut).trimStart()}`;
}

/** The chat's verbs say what a step did; a card says what it is doing. */
const NOW: Record<string, string> = {
  Edited: "Editing", Ran: "Running", Read: "Reading", Wrote: "Writing", Searched: "Searching", Listed: "Listing",
  Looked: "Looking", Watched: "Watching", Used: "Using", Asked: "Asking", Took: "Taking", Loaded: "Loading",
  Updated: "Updating", Proposed: "Proposing",
};

/** "Editing invoice.ts" for an open step; a command it described says what it is for. */
export function doingOf(step: { verb: string; target: string; code?: boolean; tool: string }): string {
  if (step.verb === "Ran" && !step.code) return step.target;
  const [first, ...rest] = step.verb.split(" ");
  return [NOW[first] ?? first, ...rest, step.target].join(" ").trim();
}

/** What a card says of an agent, from its conversation. */
export function glanceOf(items: ChatItem[]): Glance {
  let doing = "";
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i];
    if (it.kind === "step") {
      if (!doing && !it.done) doing = doingOf(it);
      continue;
    }
    if (it.kind === "text" && it.text.trim()) return { said: tail(plain(it.text)), doing, asked: "" };
    // asked and not answered yet: what it is working on is what you asked
    if (it.kind === "user" && it.text.trim()) return { said: "", doing, asked: tail(plain(it.text), 200) };
  }
  return { ...EMPTY, doing };
}

interface Feed { run: number; chat: Chat; offset: number; path: string; busy: boolean; glance: Glance }
const feeds = new Map<string, Feed>();

/** What the card of this agent says now (empty until its first read). */
export function glanceFor(paneId: string): Glance {
  return feeds.get(paneId)?.glance ?? EMPTY;
}

/** The conversation the card feed has read for this agent (empty until its first read). */
export function itemsFor(paneId: string): ChatItem[] {
  return feeds.get(paneId)?.chat.items ?? [];
}

/** The conversation object behind the feed (for digests), or null before its first read. */
export function chatFor(paneId: string): Chat | null {
  return feeds.get(paneId)?.chat ?? null;
}

/** Tests: seed what the card feed last read for an agent. */
export function setFeedItems(paneId: string, items: ChatItem[]): void {
  const f = feeds.get(paneId);
  if (f) { f.chat.items.length = 0; f.chat.items.push(...items); return; }
  const chat = createChat();
  chat.items.push(...items);
  feeds.set(paneId, { run: 0, chat, offset: 0, path: "", busy: false, glance: EMPTY });
}

/** Forget what the card feed read (closed panes; tests clear all). */
export function forgetFeed(paneId?: string): void {
  if (paneId === undefined) feeds.clear();
  else feeds.delete(paneId);
}

/** Read what the agent wrote since the last look. True when its card changes. */
export async function refreshGlance(pane: Pane): Promise<boolean> {
  const src = sourceOf(pane);
  const dir = pane.spec.ranIn;
  if (!src || !dir || !pane.spawnedAt) return false;
  let f = feeds.get(pane.id);
  if (!f || f.run !== pane.spawnedAt) {
    f = { run: pane.spawnedAt, chat: src.createChat(), offset: 0, path: "", busy: false, glance: EMPTY };
    feeds.set(pane.id, f);
  }
  if (f.busy) return false;
  f.busy = true;
  try {
    const r = await src.read(pane, dir, f.offset, f.offset === 0 && src.tails ? FIRST_READ : undefined);
    // another conversation (a /clear, a new session): start it from its end
    if (r.path && f.path && r.path !== f.path) {
      f.chat = src.createChat();
      f.offset = 0;
      f.path = r.path;
      return false;
    }
    if (r.path) f.path = r.path;
    if (r.text) f.chat.feed(r.text);
    f.chat.trim(FEED_KEEP);
    f.offset = r.next;
    const g = glanceOf(f.chat.items);
    const changed = g.said !== f.glance.said || g.doing !== f.glance.doing || g.asked !== f.glance.asked;
    f.glance = g;
    return changed;
  } catch {
    return false;
  } finally {
    f.busy = false;
  }
}


