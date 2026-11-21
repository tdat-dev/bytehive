/* What board_get hands an agent. The whole board.json as pretty JSON grew past
 * 70 KB on a busy project (most of it finished cards and their file lists),
 * and every agent reads it first, so the default is a short text view: the
 * open work in full, the finished work as its newest few titles. One card or
 * the whole board stay one call away. */

import type { Board, Card, List } from "./board.js";

/** Finished cards shown by title in the short view. */
export const DONE_SHOWN = 5;
/** A description longer than this is cut in the short view. */
export const DESC_SHOWN = 280;

const isDone = (l: List) => l.title.trim().toLowerCase() === "done";

const day = (ms: number) => (ms > 0 ? new Date(ms).toISOString().slice(0, 10) : "");

function head(c: Card): string {
  const bits = [`[${c.id}] ${c.title}`];
  if (c.assignee) bits.push(`@${c.assignee}`);
  if (c.labels.length) bits.push(c.labels.join(","));
  if (c.due) bits.push(`due ${c.due}`);
  if (c.job) bits.push(c.job);
  if (c.stopped) bits.push("stopped");
  if (c.returns) bits.push(`sent back ${c.returns}x`);
  if (c.checklist.length) bits.push(`${c.checklist.filter((i) => i.done).length}/${c.checklist.length}`);
  return bits.join(" · ");
}

function cut(text: string, max: number): string {
  const one = text.replace(/\s+/g, " ").trim();
  return one.length > max ? `${one.slice(0, max)}… (board_get card for all of it)` : one;
}

function openCard(c: Card, descMax: number): string[] {
  const out = [`- ${head(c)}`];
  if (c.desc.trim()) out.push(`  ${cut(c.desc, descMax)}`);
  for (const i of c.checklist) out.push(`  - [${i.done ? "x" : " "}] ${i.id} ${i.text}`);
  return out;
}

function doneCard(c: Card): string {
  const when = [c.done?.by, day(c.done?.at ?? 0)].filter(Boolean).join(", ");
  return `- [${c.id}] ${c.title}${when ? ` (${when})` : ""}`;
}

const newestFirst = (a: Card, b: Card) => (b.done?.at ?? 0) - (a.done?.at ?? 0);

/** The short view: every open card with its checklist ids, the newest finished ones by title. */
export function boardText(b: Board, all = false): string {
  const out: string[] = [];
  for (const l of b.lists) {
    out.push(`## ${l.title} [${l.id}] (${l.cards.length})`);
    if (!isDone(l)) {
      for (const c of l.cards) out.push(...openCard(c, all ? Infinity : DESC_SHOWN));
      continue;
    }
    const cards = [...l.cards].sort(newestFirst);
    const shown = all ? cards : cards.slice(0, DONE_SHOWN);
    out.push(...shown.map(doneCard));
    if (shown.length < cards.length) out.push(`- … ${cards.length - shown.length} older (board_get all for them)`);
  }
  return out.join("\n");
}

/** One card in full, with the list it sits in. */
export function cardView(b: Board, card: Card): { list: string; card: Card } {
  const list = b.lists.find((l) => l.cards.includes(card));
  return { list: list?.title ?? "", card };
}
