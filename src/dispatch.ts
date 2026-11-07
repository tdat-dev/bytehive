/* Structured dispatch: turn a kanban card into the prompt typed into an
 * agent's terminal. Pure (no DOM/IPC) so it stays unit-testable. */

import type { Card } from "./board";

/** Plain task text: title, description, open checklist items. */
export function cardToAgentText(card: Card): string {
  const lines = [`Task: ${card.title.trim()}`];
  const desc = card.desc.trim();
  if (desc) lines.push("", desc);
  const todo = card.checklist.filter((i) => !i.done);
  if (todo.length) {
    lines.push("");
    for (const i of todo) lines.push(`- [ ] ${i.text}`);
  }
  return lines.join("\n");
}

/** The card's own words, without the prompt around them, for the message gate:
 *  a card titled "thanks" is small talk however much dispatch wraps it. */
export function cardGist(card: Card): string {
  return cardToAgentText(card).replace(/^Task: /, "");
}

/** Full dispatch prompt: the task plus board-reporting instructions. Agents
 *  with the bytehive MCP tools keep the board in step; agents without them
 *  just do the task. */
export function dispatchPrompt(card: Card): string {
  const ref = JSON.stringify(card.title.trim());
  return (
    cardToAgentText(card) +
    "\n\n" +
    `When you start, move this card to "Doing" with the bytehive MCP tool ` +
    `card_move (card: ${ref}). When finished, call card_done (card: ${ref}) ` +
    `with a one-line summary. If you don't have bytehive tools, just do the task.`
  );
}
