/** Wire the gate to the app: one instance per project, files under
 *  `.bytehive/`, settings from prefs, and the classifier from Rust. */
import { fsCreateFile, fsReadFile, fsWriteFile, sendMessage } from "../ipc";
import { getPrefs } from "../prefs";
import { pricesFingerprint } from "../pricing";
import { taskOf } from "../tasks";
import { ResultCache } from "./cache";
import { Gate } from "./gate";
import { sha256 } from "./hash";
import { IngestLog } from "./log";
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

/** shortcut: two blocks landing together can drop one entry, fine while blocks
 *  are rare; make the file one writer if the panel starts missing some. */
async function addBlocked(root: string, entry: BlockedEntry): Promise<void> {
  let entries: BlockedEntry[] = [];
  try {
    const text = (await fsReadFile(root, BLOCKED)).content;
    const file = JSON.parse(text) as { entries?: BlockedEntry[] };
    if (Array.isArray(file.entries)) entries = file.entries;
  } catch {
    /* first block, or a file we cannot read */
  }
  entries.push(entry);
  await fsCreateFile(root, BLOCKED).catch(() => {});
  await fsWriteFile(root, BLOCKED, JSON.stringify({ version: 1, entries: entries.slice(-100) }), null);
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

const gates = new Map<string, Gate>();

function gateFor(dir: string): Gate {
  const hit = gates.get(dir);
  if (hit) return hit;
  const tools = defaultTools();
  const log = new IngestLog(fileIO(dir, LOG));
  void log.load();
  const cache = new ResultCache(fileIO(dir, CACHE), {
    ttlMs: ingestSettings().ttlDays * 86_400_000,
    version: `${pricesFingerprint()}|${tools.map((t) => `${t.name}:${t.version}`).join(",")}|1`,
  });
  const gate = new Gate({
    send: (to, text) => sendMessage(to, text),
    sendResult: (req, result) => {
      if (req.fromPane) void sendMessage(req.fromPane, resultLine(result)).catch(() => {});
    },
    classify: classifyWithProvider,
    isAwaiting: (to) => !!taskOf(to)?.status.ask,
    settings: ingestSettings,
    cache,
    tools,
    log: (entry) => void log.add(entry),
    block: (entry) => addBlocked(dir, entry),
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
  return gateFor(dir).send(req);
}
