// Hand an agent's task to an agent of another CLI: when one stops at its
// plan's usage limit (or you'd rather another CLI carried on), a new agent
// starts in the same folder with a brief written from the first one's
// conversation: what it was asked, its plan, the files it changed, and where
// it got to. The work itself is already in the folder; the brief is what the
// new agent needs to pick it up without starting over.

import type { Chat } from "./chatmodel";
import { asksSection, digestOf, filesSection, lastWordsSection, planSection } from "./convdigest";

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
  const d = digestOf(chat, from.firstAsks);
  const parts: string[] = [];
  parts.push(`You're taking over a task from ${from.name} (${from.cli}), another coding agent working in this same folder. It stopped because ${from.why}. Its work so far is already in the folder.`);
  if (d.asks.length) parts.push(asksSection(d.asks, "What it was asked, in order"));
  else if (from.title) parts.push(`Its task: ${from.title}`);
  parts.push(planSection(d.todos, "Its plan when it stopped:"));
  parts.push(filesSection(d.files, "Files it changed"));
  parts.push(lastWordsSection(d.lastWords, "Its last words:"));
  parts.push(
    chat && (d.asks.length || d.files.length)
      ? "Look at the current state of the files first (git status and the files above), then carry on and finish what's left. Don't redo what's already done, and say briefly what you picked up from before you start."
      : "Its conversation couldn't be read, so look at the folder first (git status, recent changes) to see where it got to, then carry on and finish the task.",
  );
  return parts.filter(Boolean).join("\n\n");
}
