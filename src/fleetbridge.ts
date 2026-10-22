/* App side of the fleet file-bridge (maestro-mcp is the other side).
 *
 *  - Publishes each workspace's live agent roster to <dir>/.maestro/fleet.json
 *    so an agent can call the MCP `fleet_status` tool and see who else is here.
 *  - Watches <dir>/.maestro/outbox.jsonl, where the MCP `fleet_send` tool
 *    appends messages, and delivers each new line into the target agent's PTY.
 *
 * The pure helpers (serialize/parse) are DOM- and IPC-free for unit tests; the
 * init function owns the timer and the fs calls. */

import { fsWriteFile, fsCreateDir, fsCreateFile, fsReadLinesFrom, fsTruncateIfSize } from "./ipc";

const MAESTRO_DIR = ".maestro";
const FLEET_REL = ".maestro\\fleet.json";
const OUTBOX_REL = ".maestro\\outbox.jsonl";
const SPAWN_REL = ".maestro\\spawn-requests.jsonl";

export interface BridgeAgent {
  id: string;
  name: string;
  status: "needs" | "active" | "idle" | "stopped";
  screen?: string; // recent on-screen text, for the MCP agent_output tool
  /** For the MCP agent_context tool: its CLI, its conversation when known (a
   *  Claude session id, a Codex rollout path, an OpenCode session id), the
   *  folder it runs in and when it started, so its conversation can be found. */
  cli?: string;
  session?: string;
  folder?: string;
  since?: number;
}
export interface BridgeWorkspace {
  dir: string;
  name: string;
  agents: BridgeAgent[];
}

/** The roster JSON the MCP `fleet_status` / `agent_output` tools read. */
export function serializeFleet(ws: BridgeWorkspace): string {
  return JSON.stringify({
    agents: ws.agents.map((a) => ({
      id: a.id,
      name: a.name,
      status: a.status,
      workspace: ws.name,
      screen: a.screen ?? "",
      ...(a.cli ? { cli: a.cli } : {}),
      ...(a.session ? { session: a.session } : {}),
      ...(a.folder ? { folder: a.folder } : {}),
      ...(a.since ? { since: a.since } : {}),
    })),
  });
}

export interface OutboxLine {
  from: string | null;
  to: string | null;
  message: string;
}

/** Parse one outbox jsonl line to a deliverable message, or null if unusable.
 *  `from` (the sender's MAESTRO_AGENT name) is optional on the wire — older
 *  lines and broadcasts from an unidentified sender parse to null. */
export function parseOutboxLine(line: string): OutboxLine | null {
  const t = line.trim();
  if (!t) return null;
  let o: unknown;
  try {
    o = JSON.parse(t);
  } catch {
    return null;
  }
  if (!o || typeof o !== "object") return null;
  const r = o as Record<string, unknown>;
  if (typeof r.message !== "string" || !r.message.trim()) return null;
  return {
    from: typeof r.from === "string" && r.from.trim() ? r.from : null,
    to: typeof r.to === "string" && r.to.trim() ? r.to : null,
    message: r.message,
  };
}

const sameName = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** Which agents one outbox line should reach.
 *
 *  A directed line ("to": "Bob") goes to the agent of that name — matched
 *  loosely, since an agent types the name by hand into the MCP tool. A
 *  broadcast ("to": null) goes to the whole running fleet EXCEPT the sender:
 *  a Director that dispatches to everyone would otherwise dispatch to itself,
 *  read its own order as a new instruction, and burn tokens in a loop.
 *
 *  Stopped agents are never targets — typing into a dead PTY goes nowhere. */
export function deliveryTargets<T>(
  panes: T[],
  info: (p: T) => { name: string; running: boolean },
  from: string | null,
  to: string | null,
): T[] {
  return panes.filter((p) => {
    const { name, running } = info(p);
    if (!running) return false;
    if (to) return sameName(name, to);
    return !from || !sameName(name, from);
  });
}

export interface SpawnLine {
  cli: string;
  task: string | null;
  count: number;
}

/** Parse one spawn-request jsonl line, or null if unusable. */
export function parseSpawnLine(line: string): SpawnLine | null {
  const t = line.trim();
  if (!t) return null;
  let o: unknown;
  try {
    o = JSON.parse(t);
  } catch {
    return null;
  }
  if (!o || typeof o !== "object") return null;
  const r = o as Record<string, unknown>;
  if (typeof r.cli !== "string" || !r.cli.trim()) return null;
  const count = typeof r.count === "number" ? Math.max(1, Math.min(r.count, 6)) : 1;
  return { cli: r.cli, task: typeof r.task === "string" && r.task.trim() ? r.task : null, count };
}

export interface FleetBridgeHost {
  /** Every workspace that has a folder, with its current agents. */
  workspaces(): BridgeWorkspace[];
  /** Deliver a message into a workspace's agent(s): `from` name (or null when
   *  unknown), `to` name, or null = all. */
  deliver(dir: string, from: string | null, to: string | null, message: string): void;
  /** Spawn worker agent(s) an agent requested (conductor grows its crew). */
  spawn(dir: string, req: SpawnLine): void;
}

// Per-dir byte offset up to which each file has been read (just past its last
// whole line). First sight of an existing file skips to its end so a restart
// doesn't replay the whole backlog; a file that wasn't there yet starts at 0,
// so the message that creates it is delivered too.
const consumed = new Map<string, number>();
const spawnConsumed = new Map<string, number>();
// Per-dir last fleet.json we wrote, to skip no-op writes (status unchanged).
const lastFleet = new Map<string, string>();

/** A fully-read outbox bigger than this is emptied (see fs_truncate_if_size). */
export const COMPACT_BYTES = 1024 * 1024;
/** Slices read in one tick at most; the rest waits for the next tick. */
const MAX_SLICES = 8;

async function ensureMaestro(dir: string): Promise<void> {
  try {
    await fsCreateDir(dir, MAESTRO_DIR);
  } catch {
    /* exists */
  }
}

/** One tick: publish every roster, deliver every new message and spawn request. */
export async function fleetTick(host: FleetBridgeHost): Promise<void> {
  const jobs: Promise<void>[] = [];
  const drained = new Set<string>();
  for (const ws of host.workspaces()) {
    if (!ws.dir) continue;
    jobs.push(publishRoster(ws));
    // Two workspaces on one folder share its files: drain them once.
    if (drained.has(ws.dir)) continue;
    drained.add(ws.dir);
    jobs.push(
      drainJsonl(ws.dir, OUTBOX_REL, consumed, (line) => {
        const parsed = parseOutboxLine(line);
        if (parsed) host.deliver(ws.dir, parsed.from, parsed.to, parsed.message);
      }),
    );
    jobs.push(
      drainJsonl(ws.dir, SPAWN_REL, spawnConsumed, (line) => {
        const parsed = parseSpawnLine(line);
        if (parsed) host.spawn(ws.dir, parsed);
      }),
    );
  }
  await Promise.all(jobs);
}

/** Start the publish + deliver loop. Returns a stop function.
 *
 *  Each tick is scheduled only after the previous one has finished, so two
 *  never overlap (an overlap read the same lines twice and delivered twice).
 *  While the window is hidden it runs half as often: messages between agents
 *  must keep flowing in the background, just not every 1.5 s. */
export function initFleetBridge(host: FleetBridgeHost, intervalMs = 1500): () => void {
  let stopped = false;
  let timer = 0;
  const next = () => {
    if (stopped) return;
    timer = window.setTimeout(run, document.hidden ? intervalMs * 2 : intervalMs);
  };
  const run = () => {
    fleetTick(host).catch(() => {}).finally(next);
  };
  next();
  return () => {
    stopped = true;
    window.clearTimeout(timer);
  };
}

async function publishRoster(ws: BridgeWorkspace): Promise<void> {
  const content = serializeFleet(ws);
  if (lastFleet.get(ws.dir) === content) return; // nothing changed
  try {
    await ensureMaestro(ws.dir);
    try {
      await fsCreateFile(ws.dir, FLEET_REL);
    } catch {
      /* exists */
    }
    await fsWriteFile(ws.dir, FLEET_REL, content, null);
    lastFleet.set(ws.dir, content);
  } catch {
    /* folder unwritable this tick — try again next time */
  }
}

/** Hand each new whole line of an append-only jsonl file to `handle`.
 *
 *  Reads from the byte offset the last read stopped at, never the whole file:
 *  the outbox only grows, and re-reading it all every tick eventually hit the
 *  2 MB cap of fs_read_file, after which nothing was delivered again. A line
 *  the MCP server is still writing is left for the next tick. Once everything
 *  has been read and the file is big, it's emptied so it can't grow forever. */
export async function drainJsonl(
  dir: string,
  rel: string,
  offsets: Map<string, number>,
  handle: (line: string) => void,
): Promise<void> {
  const at = offsets.get(dir);
  let chunk: { text: string; next: number; from: number; size: number };
  try {
    chunk = await fsReadLinesFrom(dir, rel, at ?? null);
  } catch {
    // Not there yet: whatever appears in it from now on is new.
    if (at === undefined) offsets.set(dir, 0);
    return;
  }
  if (at === undefined) {
    offsets.set(dir, chunk.next); // skip backlog on first sight
    return;
  }
  for (let i = 0; ; i += 1) {
    // Moved on before handling, so a line that throws is never handed over twice.
    offsets.set(dir, chunk.next);
    for (const line of chunk.text.split("\n")) {
      if (!line.trim()) continue;
      try {
        handle(line);
      } catch {
        /* one bad line never stops the rest */
      }
    }
    if (!chunk.text || chunk.next >= chunk.size || i + 1 >= MAX_SLICES) break;
    try {
      chunk = await fsReadLinesFrom(dir, rel, chunk.next);
    } catch {
      return;
    }
  }
  if (chunk.next === chunk.size && chunk.size > COMPACT_BYTES) {
    try {
      if (await fsTruncateIfSize(dir, rel, chunk.size)) offsets.set(dir, 0);
    } catch {
      /* try again once more has been read */
    }
  }
}
