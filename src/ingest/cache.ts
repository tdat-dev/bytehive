/** Read and write the tool-result cache. A file in `.bytehive/` is the real
 *  store; a Map is always the fallback, so a broken file loses no messages. */
import type { ToolResult } from "./types";

export interface CacheIO {
  read(): Promise<string | null>;
  write(text: string): Promise<void>;
}

interface Entry {
  at: number;
  toolVersion: string;
  result: ToolResult;
}

interface File {
  version: 1;
  entries: Record<string, Entry>;
}

export interface CacheOptions {
  ttlMs: number;
  /** Changes when prices or tools change, so old answers stop matching. */
  version: string;
  cap?: number;
  now?: () => number;
  warn?: (message: string) => void;
}

const FILE_VERSION = 1;
const DEFAULT_CAP = 500;

export class ResultCache {
  private entries = new Map<string, Entry>();
  private loaded = false;
  private failures = 0;
  private memoryUntil = 0;
  private warned = false;
  private readonly cap: number;
  private readonly now: () => number;

  constructor(
    private readonly io: CacheIO,
    private readonly opts: CacheOptions,
  ) {
    this.cap = opts.cap ?? DEFAULT_CAP;
    this.now = opts.now ?? Date.now;
  }

  async get(key: string): Promise<ToolResult | null> {
    await this.load();
    const e = this.entries.get(key);
    if (!e) return null;
    if (e.toolVersion !== this.opts.version || this.now() - e.at > this.opts.ttlMs) {
      this.entries.delete(key);
      return null;
    }
    return e.result;
  }

  async set(key: string, result: ToolResult): Promise<void> {
    await this.load();
    this.entries.set(key, { at: this.now(), toolVersion: this.opts.version, result });
    if (this.entries.size > this.cap) {
      const oldest = [...this.entries.entries()].sort((a, b) => a[1].at - b[1].at)[0];
      if (oldest) this.entries.delete(oldest[0]);
    }
    await this.flush();
  }

  /** Entries currently held, for tests and the panel's count. */
  size(): number {
    return this.entries.size;
  }

  private async load(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;
    try {
      const text = await this.io.read();
      if (!text) return;
      const file = JSON.parse(text) as File;
      if (file?.version !== FILE_VERSION || !file.entries) return;
      for (const [k, e] of Object.entries(file.entries)) {
        if (e && typeof e.at === "number" && e.result) this.entries.set(k, e);
      }
    } catch {
      this.warn("cache file unreadable, starting empty");
    }
  }

  private async flush(): Promise<void> {
    if (this.memoryUntil > this.now()) return;
    const file: File = { version: FILE_VERSION, entries: Object.fromEntries(this.entries) };
    try {
      await this.io.write(JSON.stringify(file));
      this.failures = 0;
    } catch {
      this.failures += 1;
      this.warn("cache write failed, keeping results in memory");
      if (this.failures >= 2) this.memoryUntil = this.now() + 60_000;
    }
  }

  private warn(message: string): void {
    if (this.warned) return;
    this.warned = true;
    this.opts.warn?.(message);
  }
}
