/* Where each agent CLI keeps its conversations, read straight from disk so any
 * agent with this server (in ByteHive or not, the app running or not) can see
 * what the others did in a project:
 *   Claude Code  ~/.claude/projects/<slug of folder>/<session>.jsonl
 *   Codex        ~/.codex/sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl
 *   OpenCode     ~/.local/share/opencode/opencode.db (SQLite)
 * The app reads the same stores in Rust (src-tauri/src/transcripts); the
 * parsing is the app's own (src/chatmodel.ts and friends). */

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { createChat, type Chat } from "../../../src/chatmodel";
import { createCodexChat } from "../../../src/codexmodel";
import { createOpencodeChat } from "../../../src/opencodemodel";

export type Cli = "claude" | "codex" | "opencode";

export interface Conversation {
  cli: Cli;
  id: string;
  /** The folder it ran in. */
  folder: string;
  /** When it last changed, ms. */
  updated: number;
  /** When it began, ms. */
  started: number;
  /** Whether its CLI is still running it; null when the store can't tell. */
  running: boolean | null;
  /** Claude, Codex: its file. OpenCode: its session id. */
  path: string;
}

export interface Env {
  home: string;
  now: number;
  /** How far back to look. */
  days?: number;
  /** Most conversations listed. */
  limit?: number;
}

const DAYS = 14;
const LIMIT = 20;
const HEAD_BYTES = 256 * 1024;
const TAIL_BYTES = 4 * 1024 * 1024;
const OPENCODE_HEAD_ROWS = 300;
const OPENCODE_TAIL_ROWS = 4000;
const DAY_MS = 86_400_000;

const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();

/** "D:\\App\\" and "d:/app" are the same folder. */
export function sameDir(a: string, b: string): boolean {
  return norm(a) === norm(b);
}

/** Claude Code's folder name for a project: every non-alphanumeric is "-". */
export function claudeSlug(dir: string): string {
  return dir.replace(/[^A-Za-z0-9]/g, "-");
}

/** The project folder and every git worktree of its repo (ByteHive's agents
 *  work in worktrees outside the repo). */
export function projectFolders(dir: string): string[] {
  const out = [dir];
  try {
    const list = execFileSync("git", ["worktree", "list", "--porcelain"], { cwd: dir, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
    for (const line of list.split(/\r?\n/)) {
      const m = /^worktree (.+)$/.exec(line);
      if (m) out.push(path.normalize(m[1]));
    }
  } catch { /* not a repo: just the folder */ }
  return out.filter((d, i) => out.findIndex((e) => sameDir(d, e)) === i);
}

const statOf = (p: string) => { try { return fs.statSync(p); } catch { return null; } };
const listDir = (p: string) => { try { return fs.readdirSync(p); } catch { return [] as string[]; } };

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === "EPERM";
  }
}

/** Claude sessions whose process is alive, from ~/.claude/sessions/<pid>.json. */
function liveClaudeSessions(home: string): Set<string> {
  const dir = path.join(home, ".claude", "sessions");
  const live = new Set<string>();
  for (const f of listDir(dir)) {
    if (!f.endsWith(".json")) continue;
    try {
      const o = JSON.parse(fs.readFileSync(path.join(dir, f), "utf8")) as { pid?: number; sessionId?: string };
      if (typeof o.pid === "number" && typeof o.sessionId === "string" && alive(o.pid)) live.add(o.sessionId);
    } catch { /* being written */ }
  }
  return live;
}

function claudeIn(folders: string[], env: Env, from: number): Conversation[] {
  const live = liveClaudeSessions(env.home);
  const out: Conversation[] = [];
  for (const folder of folders) {
    const dir = path.join(env.home, ".claude", "projects", claudeSlug(folder));
    for (const f of listDir(dir)) {
      if (!f.endsWith(".jsonl")) continue;
      const p = path.join(dir, f);
      const st = statOf(p);
      if (!st?.isFile() || st.mtimeMs < from) continue;
      const id = f.slice(0, -".jsonl".length);
      out.push({ cli: "claude", id, folder, updated: st.mtimeMs, started: st.birthtimeMs || st.mtimeMs, running: live.has(id), path: p });
    }
  }
  return out;
}

/** The first line of a file, parsed (a Codex rollout's session_meta). */
function firstLine(p: string): Record<string, unknown> | null {
  let fd: number | null = null;
  try {
    fd = fs.openSync(p, "r");
    const buf = Buffer.alloc(HEAD_BYTES);
    const n = fs.readSync(fd, buf, 0, HEAD_BYTES, 0);
    const text = buf.subarray(0, n).toString("utf8");
    const nl = text.indexOf("\n");
    return JSON.parse(nl >= 0 ? text.slice(0, nl) : text) as Record<string, unknown>;
  } catch {
    return null;
  } finally {
    if (fd !== null) fs.closeSync(fd);
  }
}

function codexIn(folders: string[], env: Env, from: number): Conversation[] {
  const root = path.join(env.home, ".codex", "sessions");
  const out: Conversation[] = [];
  const firstDay = new Date(from).toISOString().slice(0, 10);
  for (const y of listDir(root)) {
    for (const m of listDir(path.join(root, y))) {
      for (const d of listDir(path.join(root, y, m))) {
        if (`${y}-${m}-${d}` < firstDay) continue;
        const dir = path.join(root, y, m, d);
        for (const f of listDir(dir)) {
          if (!f.startsWith("rollout-") || !f.endsWith(".jsonl")) continue;
          const p = path.join(dir, f);
          const st = statOf(p);
          if (!st?.isFile() || st.mtimeMs < from) continue;
          const meta = (firstLine(p)?.payload ?? {}) as { id?: unknown; cwd?: unknown; timestamp?: unknown };
          const folder = typeof meta.cwd === "string" ? folders.find((x) => sameDir(x, meta.cwd as string)) : undefined;
          if (!folder) continue;
          const began = typeof meta.timestamp === "string" ? Date.parse(meta.timestamp) : NaN;
          out.push({
            cli: "codex",
            id: typeof meta.id === "string" ? meta.id : f.slice("rollout-".length, -".jsonl".length),
            folder,
            updated: st.mtimeMs,
            started: Number.isFinite(began) ? began : st.birthtimeMs || st.mtimeMs,
            running: null,
            path: p,
          });
        }
      }
    }
  }
  return out;
}

interface SqliteDb {
  prepare(sql: string): { all(...params: unknown[]): unknown[] };
  close(): void;
}

/** OpenCode's database, read only, or null when there is none (or Node has no node:sqlite). */
async function openOpencode(home: string): Promise<{ db: SqliteDb | null; note?: string }> {
  const file = path.join(home, ".local", "share", "opencode", "opencode.db");
  if (!fs.existsSync(file)) return { db: null };
  try {
    const { DatabaseSync } = (await import("node:sqlite")) as unknown as { DatabaseSync: new (p: string, o: object) => SqliteDb };
    return { db: new DatabaseSync(file, { readOnly: true }) };
  } catch {
    return { db: null, note: "OpenCode conversations are left out: reading them needs Node 22.13 or newer." };
  }
}

async function opencodeIn(folders: string[], env: Env, from: number): Promise<{ list: Conversation[]; note?: string }> {
  const { db, note } = await openOpencode(env.home);
  if (!db) return { list: [], note };
  try {
    const rows = db.prepare("SELECT id, directory, time_created, time_updated FROM session WHERE parent_id IS NULL AND time_updated >= ? ORDER BY time_updated DESC LIMIT 500").all(Math.floor(from)) as Array<{ id: string; directory: string; time_created: number; time_updated: number }>;
    const list: Conversation[] = [];
    for (const r of rows) {
      const folder = folders.find((x) => sameDir(x, r.directory));
      if (folder) list.push({ cli: "opencode", id: r.id, folder, updated: r.time_updated, started: r.time_created, running: null, path: r.id });
    }
    return { list };
  } catch {
    return { list: [], note: "OpenCode's database could not be read." };
  } finally {
    db.close();
  }
}

/** The conversations of every CLI in these folders, newest first. `notes`
 *  say what could not be read. */
export async function findConversations(folders: string[], env: Env): Promise<{ list: Conversation[]; notes: string[] }> {
  const from = env.now - (env.days ?? DAYS) * DAY_MS;
  const oc = await opencodeIn(folders, env, from);
  const list = [...claudeIn(folders, env, from), ...codexIn(folders, env, from), ...oc.list]
    .sort((a, b) => b.updated - a.updated)
    .slice(0, env.limit ?? LIMIT);
  return { list, notes: oc.note ? [oc.note] : [] };
}

/** Whole lines of a file: its start and its end (the same text when it is short). */
function slices(p: string): { head: string; tail: string } {
  const size = statOf(p)?.size ?? 0;
  if (size <= TAIL_BYTES) {
    const all = fs.readFileSync(p, "utf8");
    return { head: all, tail: all };
  }
  const fd = fs.openSync(p, "r");
  try {
    const hb = Buffer.alloc(HEAD_BYTES);
    const hn = fs.readSync(fd, hb, 0, HEAD_BYTES, 0);
    const head = hb.subarray(0, hn).toString("utf8");
    const tb = Buffer.alloc(TAIL_BYTES);
    const tn = fs.readSync(fd, tb, 0, TAIL_BYTES, size - TAIL_BYTES);
    const tail = tb.subarray(0, tn).toString("utf8");
    return { head: head.slice(0, head.lastIndexOf("\n") + 1), tail: tail.slice(tail.indexOf("\n") + 1) };
  } finally {
    fs.closeSync(fd);
  }
}

/** OpenCode rows as the app's Rust reader hands them to the chat. */
function opencodeRows(db: SqliteDb, session: string, newest: boolean, limit: number): string {
  const rows = db.prepare(
    `SELECT kind, id, message_id, time_created, time_updated, data FROM (
       SELECT 'message' AS kind, id, id AS message_id, time_created, time_updated, data FROM message WHERE session_id = ?
       UNION ALL
       SELECT 'part' AS kind, id, message_id, time_created, time_updated, data FROM part WHERE session_id = ?
     ) ORDER BY time_updated ${newest ? "DESC" : "ASC"}, kind, id LIMIT ?`,
  ).all(session, session, limit) as Array<{ kind: string; id: string; message_id: string; time_created: number; time_updated: number; data: string }>;
  if (newest) rows.reverse();
  return rows.map((r) => {
    let data: unknown = r.data;
    try { data = JSON.parse(r.data); } catch { /* kept as text */ }
    return JSON.stringify({ kind: r.kind, id: r.id, messageId: r.message_id, created: r.time_created, updated: r.time_updated, data });
  }).join("\n") + "\n";
}

const firstAsksOf = (chat: Chat) => chat.items.flatMap((i) => (i.kind === "user" && i.text ? [i.text] : [])).slice(0, 2);

/** A conversation parsed: its end as a chat, its first asks from its start. */
export async function readConversation(c: Conversation, env: Env): Promise<{ chat: Chat; firstAsks: string[] }> {
  if (c.cli === "opencode") {
    const chat = createOpencodeChat();
    const head = createOpencodeChat();
    const { db } = await openOpencode(env.home);
    if (db) {
      try {
        head.feed(opencodeRows(db, c.path, false, OPENCODE_HEAD_ROWS));
        chat.feed(opencodeRows(db, c.path, true, OPENCODE_TAIL_ROWS));
      } catch { /* what it has */ } finally {
        db.close();
      }
    }
    return { chat, firstAsks: firstAsksOf(head) };
  }
  const make = c.cli === "claude" ? createChat : createCodexChat;
  const chat = make();
  const head = make();
  try {
    const s = slices(c.path);
    head.feed(s.head);
    chat.feed(s.tail);
  } catch { /* gone since it was listed */ }
  return { chat, firstAsks: firstAsksOf(head) };
}
