/** The append-only record of what the gate did, kept as JSONL in
 *  `.bytehive/`. The file is rewritten on a short debounce because the
 *  backend has no append command. Contents are never written, only the hash. */
import type { LogEntry } from "./types";

export interface LogIO {
  read(): Promise<string | null>;
  write(text: string): Promise<void>;
}

export interface LogStats {
  messages: number;
  cacheHits: number;
  blocked: number;
  calls: number;
  tokens: number;
  saved: number;
}

const DEFAULT_CAP = 2000;
const DEFAULT_DELAY = 2000;

export class IngestLog {
  private entries: LogEntry[] = [];
  private loaded = false;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private warned = false;
  private readonly cap: number;
  private readonly delay: number;
  private readonly now: () => number;
  private readonly warnFn: (message: string) => void;

  constructor(
    private readonly io: LogIO,
    opts: { cap?: number; delayMs?: number; now?: () => number; warn?: (message: string) => void } = {},
  ) {
    this.cap = opts.cap ?? DEFAULT_CAP;
    this.delay = opts.delayMs ?? DEFAULT_DELAY;
    this.now = opts.now ?? Date.now;
    this.warnFn = opts.warn ?? ((message) => this.warnOnce(message));
  }

  async add(entry: LogEntry): Promise<void> {
    await this.load();
    this.entries.push(entry);
    if (this.entries.length > this.cap) this.entries.splice(0, this.entries.length - this.cap);
    if (this.delay <= 0) {
      await this.flush();
      return;
    }
    if (this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.flush();
    }, this.delay);
  }

  async flush(): Promise<void> {
    if (!this.loaded) return;
    try {
      await this.io.write(this.entries.map((e) => JSON.stringify(e)).join("\n") + (this.entries.length ? "\n" : ""));
    } catch (e) {
      this.warnFn("ingest log write failed: " + String(e));
    }
  }

  /** Entries since local midnight, for the panel and the daily caps. */
  today(): LogEntry[] {
    const d = new Date(this.now());
    d.setHours(0, 0, 0, 0);
    return this.entries.filter((e) => e.ts >= d.getTime());
  }

  stats(): LogStats {
    const today = this.today();
    const out: LogStats = { messages: today.length, cacheHits: 0, blocked: 0, calls: 0, tokens: 0, saved: 0 };
    for (const e of today) {
      if (e.path === "cache-hit") out.cacheHits += 1;
      if (e.path === "blocked") out.blocked += 1;
      if (e.llm?.called) {
        out.calls += 1;
        out.tokens += e.llm.in + e.llm.out;
      }
      out.saved += e.tokensSavedEstimate ?? 0;
    }
    return out;
  }

  async load(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;
    try {
      const text = await this.io.read();
      if (!text) return;
      for (const line of text.split(/\r?\n/)) {
        if (!line.trim()) continue;
        try {
          const e = JSON.parse(line) as LogEntry;
          if (e && typeof e.ts === "number" && typeof e.path === "string") this.entries.push(e);
        } catch {
          /* a half written last line is not worth keeping */
        }
      }
      if (this.entries.length > this.cap) this.entries.splice(0, this.entries.length - this.cap);
    } catch {
      this.warnFn("ingest log unreadable, starting empty");
    }
  }

  private warnOnce(message: string): void {
    if (this.warned) return;
    this.warned = true;
    console.warn(message);
  }
}
