/** Wire the gate to the app: one instance per project, files under
 *  `.bytehive/`, settings from prefs, and the classifier from Rust. */
import { cardDelivered } from "../agentbridge";
import { workspaces } from "../appstate";
import { showDelegationToast } from "../delegation";
import { fsCreateFile, fsReadFile, fsWriteFile, sendMessage } from "../ipc";
import { getPrefs, onPrefs } from "../prefs";
import { pricesFingerprint } from "../pricing";
import { taskOf } from "../tasks";
import { BlockedStore, liveTarget, type BlockedList } from "./blocked";
import { ResultCache } from "./cache";
import { Gate } from "./gate";
import { sha256 } from "./hash";
import { IngestLog, type LogStats } from "./log";
import { classifyWithProvider } from "./provider";
import { defaultTools } from "./tools";
import type { BlockedEntry, GateSettings, Outcome, Request, ToolResult } from "./types";

const CACHE = ".bytehive/ingest-cache.json";
const BLOCKED = ".bytehive/ingest-blocked.json";
const LOG = ".bytehive/ingest.jsonl";

function fileIO(root: string, rel: string) {
  return {
    read: async (): Promise<string | null> => {
      try {
        return (await fsReadFile(root, rel)).content;
      } catch {
        return null;
      }
    },
    write: async (text: string): Promise<void> => {
      await fsCreateFile(root, rel).catch(() => {});
      await fsWriteFile(root, rel, text, null);
    },
  };
}

export function ingestSettings(): GateSettings {
  const p = getPrefs();
  return {
    enabled: p.ingest,
    ttlDays: p.ingestTtlDays,
    dailyCallCap: p.ingestDailyCalls,
    dailyTokenCap: p.ingestDailyTokens,
    loopWindowMin: p.ingestLoopMin,
  };
}

function resultLine(result: ToolResult): string {
  if (result.status === "error") return `[ingest] ${result.metadata.tool} failed: ${result.error ?? "error"}`;
  const values = Object.entries(result.values)
    .map(([k, v]) => `${k}=${typeof v === "number" ? Math.round(v * 1e6) / 1e6 : v}`)
    .join(" ");
  return `[ingest] ${result.metadata.tool}: ${values}`;
}

/** Something the gate answered with code, kept for the panel until the app closes. */
export interface RecentResult {
  at: number;
  source: Request["source"];
  from?: string;
  to: string;
  toName?: string;
  text: string;
  gist?: string;
  result: ToolResult;
  cached: boolean;
}

const RECENT_MAX = 50;

const gates = new Map<string, Gate>();
const logs = new Map<string, IngestLog>();
const stores = new Map<string, BlockedStore>();
const recents = new Map<string, RecentResult[]>();
const listeners = new Set<() => void>();

const changed = (): void => {
  for (const fn of listeners) {
    try {
      fn();
    } catch {
      /* a broken view must not break a send */
    }
  }
};

function logFor(dir: string): IngestLog {
  let log = logs.get(dir);
  if (!log) {
    log = new IngestLog(fileIO(dir, LOG));
    void log.load();
    logs.set(dir, log);
  }
  return log;
}

function storeFor(dir: string): BlockedStore {
  let store = stores.get(dir);
  if (!store) {
    store = new BlockedStore(fileIO(dir, BLOCKED));
    stores.set(dir, store);
  }
  return store;
}

// Only a changed ingest setting rebuilds a gate; a font or tray change must not
// disturb a log write that is still on its debounce.
let lastSettings = JSON.stringify(ingestSettings());
onPrefs(() => {
  const next = JSON.stringify(ingestSettings());
  if (next === lastSettings) return;
  lastSettings = next;
  gates.clear();
});

function gateFor(dir: string): Gate {
  const hit = gates.get(dir);
  if (hit) return hit;
  const tools = defaultTools();
  const log = logFor(dir);
  const cache = new ResultCache(fileIO(dir, CACHE), {
    ttlMs: ingestSettings().ttlDays * 86_400_000,
    version: `${pricesFingerprint()}|${tools.map((t) => `${t.name}:${t.version}`).join(",")}|1`,
  });
  const gate = new Gate({
    send: (to, text) => sendMessage(to, text),
    sendResult: (req, result) => {
      if (req.fromPane) {
        void sendMessage(req.fromPane, resultLine(result)).catch(() => {});
        return true;
      }
      // A person reads it in the toast and the panel; anyone else gets the agent.
      return req.source === "kanban" || req.source === "phone";
    },
    classify: classifyWithProvider,
    isAwaiting: (to) => !!taskOf(to)?.status.ask,
    settings: ingestSettings,
    cache,
    tools,
    log: (entry) => void log.add(entry),
    block: (entry) => storeFor(dir).add(entry),
    canCall: () => {
      const stats = log.stats();
      const settings = ingestSettings();
      return stats.calls < settings.dailyCallCap && stats.tokens < settings.dailyTokenCap;
    },
    hash: sha256,
    id: () => crypto.randomUUID(),
  });
  gates.set(dir, gate);
  return gate;
}

/** Send one programmatic message through the gate for the project at `dir`. */
export async function sendGated(dir: string, req: Request): Promise<Outcome> {
  const out = await gateFor(dir).send(req);
  if (out.kind === "answered" || out.kind === "cache-hit") {
    const list = recents.get(dir) ?? [];
    list.push({ ...req, at: Date.now(), result: out.result, cached: out.kind === "cache-hit" });
    recents.set(dir, list.slice(-RECENT_MAX));
    if (!req.fromPane) showDelegationToast("Ingest", [req.toName ?? "agent"], resultLine(out.result).replace("[ingest] ", ""));
  } else if (out.kind === "blocked") {
    if (!req.fromPane) showDelegationToast("Ingest", [req.toName ?? "agent"], "Held back. Open Ingest to send it anyway.");
  } else if (out.kind === "failed") {
    if (!req.fromPane) showDelegationToast("Ingest", [req.toName ?? "agent"], "Couldn't deliver it.");
  }
  changed();
  return out;
}

/** Tell the panel when something it shows has changed. */
export function onIngest(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export const blockedList = (dir: string): Promise<BlockedList> => storeFor(dir).list();

export const blockedRemove = (dir: string, id: string): Promise<BlockedEntry | null> => storeFor(dir).remove(id);

export const blockedRestore = (dir: string, entry: BlockedEntry): Promise<void> => storeFor(dir).add(entry);

/** Deliver a held message as it was written, past the gate. It stays on the
 *  list unless it really reached the agent. */
export async function blockedResend(dir: string, entry: BlockedEntry): Promise<boolean> {
  const ws = [...workspaces.values()].find((w) => w.dir === dir);
  const panes = ws ? [...ws.panes.values()].map((p) => ({ id: p.id, name: p.spec.name, running: p.running })) : [];
  const to = liveTarget(panes, entry);
  if (!to) return false;
  const sent = await sendMessage(to, entry.text).catch(() => false);
  if (!sent) return false;
  await storeFor(dir).remove(entry.id);
  void logFor(dir).add({ ts: Date.now(), hash: entry.hash, source: "retry", from: entry.from, to, path: "pass", ms: { total: 0 } });
  if (entry.card) cardDelivered(dir, entry.card, panes.find((p) => p.id === to)?.name ?? entry.toName ?? "");
  return true;
}

export async function ingestStats(dir: string): Promise<LogStats> {
  const log = logFor(dir);
  await log.load();
  return log.stats();
}

export const ingestRecent = (dir: string): readonly RecentResult[] => recents.get(dir) ?? [];
