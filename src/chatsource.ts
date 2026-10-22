// Where each agent CLI keeps its conversation, and how to read it into the
// chat view's items: Claude Code's JSONL transcript, Codex's rollout, opencode's
// session store. The view polls `read` and feeds what comes back to the chat.

import { createChat, type Chat } from "./chatmodel";
import { createCodexChat } from "./codexmodel";
import { createOpencodeChat } from "./opencodemodel";
import { claudeTranscript, codexTranscript, opencodeTranscript } from "./ipc";
import type { Pane } from "./panetypes";

/** `from`: where the text starts in the conversation's file; more than 0 when
 *  a first read took only the end of a long one. */
export interface Chunk { path: string; text: string; next: number; from?: number }

export interface ChatSource {
  createChat(): Chat;
  /** What was written after `offset`. `path` names the conversation; another
   *  one means the agent is in a new conversation. "" until there is one. */
  read(pane: Pane, dir: string, offset: number, tailBytes?: number): Promise<Chunk>;
  /** It can start a first read near the end of a long conversation. */
  tails?: boolean;
  /** It needs its own session id (or a start time) to know which conversation is its own. */
  needsSession?: boolean;
}

/** Only a conversation begun after this run started can be its own. */
const since = (pane: Pane) => (pane.spawnedAt ? pane.spawnedAt - 5000 : null);

/** How often a known conversation is checked for a newer one (/new in the CLI). */
export const RECHECK_MS = 10_000;

/** The conversation found for each agent, so a poll every 800 ms doesn't search
 *  for it from scratch: for Codex that meant listing every day folder and
 *  opening the head of each recent rollout, for opencode querying its sessions.
 *  Forgotten when the agent runs again; searched again now and then in case the
 *  agent began a new one, and at once when the known one is gone. */
interface Known { path: string; run: number | null; checkedAt: number }
const known = new Map<string, Known>();

/** Read with the remembered conversation when there is one (and it's not time
 *  to look for a newer one), remembering whatever the read settled on. */
async function remembered(pane: Pane, read: (path: string | null) => Promise<Chunk>): Promise<Chunk> {
  const run = pane.spawnedAt ?? null;
  const k = known.get(pane.id);
  const now = Date.now();
  const fresh = k && k.run === run && now - k.checkedAt < RECHECK_MS ? k : null;
  const r = await read(fresh ? fresh.path : null);
  if (!r.path) {
    known.delete(pane.id); // gone (or none yet): search next time
  } else if (!fresh || r.path !== fresh.path) {
    known.set(pane.id, { path: r.path, run, checkedAt: now });
  }
  return r;
}

/** The conversation last found for an agent (a Codex rollout path, an
 *  OpenCode session id), or null before anything read it. */
export function knownConversation(paneId: string): string | null {
  return known.get(paneId)?.path ?? null;
}

/** Forget what was remembered for an agent (tests; a closed pane). */
export function forgetConversation(paneId?: string): void {
  if (paneId === undefined) known.clear();
  else known.delete(paneId);
}

export const SOURCES: Record<string, ChatSource> = {
  claude: {
    createChat,
    read: (pane, dir, offset, tail) => claudeTranscript(dir, pane.spec.sessionId ?? null, since(pane), offset, tail),
    needsSession: true,
    tails: true,
  },
  // Codex and opencode pick their own session ids: the newest one begun in the
  // agent's folder since it started is its own.
  codex: {
    createChat: createCodexChat,
    read: (pane, dir, offset, tail) => remembered(pane, (path) => codexTranscript(dir, since(pane), path, offset, tail)),
    tails: true,
  },
  opencode: {
    createChat: createOpencodeChat,
    read: (pane, dir, offset) => remembered(pane, (session) => opencodeTranscript(dir, since(pane), session, offset)),
  },
};

export function sourceOf(pane: Pane): ChatSource | null {
  return SOURCES[pane.spec.badge] ?? null;
}
