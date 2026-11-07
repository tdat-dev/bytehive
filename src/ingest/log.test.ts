import { describe, expect, it } from "vitest";
import { IngestLog } from "./log";
import type { LogEntry } from "./types";

const entry = (path: LogEntry["path"], extra: Partial<LogEntry> = {}): LogEntry => ({
  ts: Date.now(),
  hash: "h",
  source: "fleet",
  to: "a1",
  path,
  ms: { total: 1 },
  ...extra,
});

describe("IngestLog", () => {
  it("counts today's paths and tokens", async () => {
    let text: string | null = null;
    const log = new IngestLog(
      {
        read: async () => text,
        write: async (next) => {
          text = next;
        },
      },
      { delayMs: 0 },
    );
    await log.add(entry("cache-hit", { tokensSavedEstimate: 40 }));
    await log.add(entry("blocked", { tokensSavedEstimate: 5 }));
    await log.add(entry("pass", { llm: { called: true, in: 10, out: 4 } }));
    expect(log.stats()).toEqual({ messages: 3, cacheHits: 1, blocked: 1, calls: 1, tokens: 14, saved: 45 });
    expect(text).toContain('"cache-hit"');
  });

  it("keeps only the newest entries", async () => {
    const log = new IngestLog({ read: async () => null, write: async () => {} }, { cap: 2, delayMs: 0 });
    await log.add(entry("pass"));
    await log.add(entry("pass"));
    await log.add(entry("blocked"));
    expect(log.stats().messages).toBe(2);
    expect(log.stats().blocked).toBe(1);
  });

  it("does not count a message twice when it is sent again from the panel", async () => {
    const log = new IngestLog({ read: async () => null, write: async () => {} }, { delayMs: 0, now: () => 1_000 });
    await log.add({ ts: 1_000, hash: "h", source: "fleet", to: "a", path: "blocked", ms: { total: 0 } });
    await log.add({ ts: 1_000, hash: "h", source: "retry", to: "a", path: "pass", ms: { total: 0 } });
    expect(log.stats()).toMatchObject({ messages: 1, blocked: 1 });
  });
});
