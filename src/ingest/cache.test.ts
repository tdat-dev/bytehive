import { describe, expect, it } from "vitest";
import { ResultCache } from "./cache";
import type { ToolResult } from "./types";

const result: ToolResult = {
  status: "ok",
  values: { usd: 1 },
  timestamp: 0,
  metadata: { tool: "estimate_cost", version: "1", cacheable: true },
};

function memoryIO() {
  let text: string | null = null;
  return {
    read: async () => text,
    write: async (next: string) => {
      text = next;
    },
    text: () => text,
  };
}

describe("ResultCache", () => {
  it("keeps a result and reads it back", async () => {
    const io = memoryIO();
    const cache = new ResultCache(io, { ttlMs: 1000, version: "v1" });
    await cache.set("k", result);
    expect(await cache.get("k")).toEqual(result);
    expect(io.text()).toContain("estimate_cost");
  });

  it("forgets an entry after its TTL", async () => {
    let clock = 1000;
    const cache = new ResultCache(memoryIO(), { ttlMs: 100, version: "v1", now: () => clock });
    await cache.set("k", result);
    clock += 101;
    expect(await cache.get("k")).toBeNull();
  });

  it("forgets an entry when the tool version changes", async () => {
    const io = memoryIO();
    const v2 = new ResultCache(io, { ttlMs: 1000, version: "v2" });
    await v2.set("k", result);
    const v1 = new ResultCache(io, { ttlMs: 1000, version: "v1" });
    expect(await v1.get("k")).toBeNull();
  });

  it("starts empty when the file is corrupt", async () => {
    const cache = new ResultCache({ read: async () => "{", write: async () => {} }, { ttlMs: 1000, version: "v1" });
    expect(await cache.get("k")).toBeNull();
    await cache.set("k", result);
    expect(await cache.get("k")).toEqual(result);
  });

  it("keeps working when the disk refuses the write", async () => {
    let writes = 0;
    const cache = new ResultCache(
      {
        read: async () => null,
        write: async () => {
          writes += 1;
          throw new Error("disk full");
        },
      },
      { ttlMs: 1000, version: "v1", warn: () => {} },
    );
    await cache.set("a", result);
    await cache.set("b", result);
    await cache.set("c", result);
    expect(writes).toBe(2);
    expect(await cache.get("c")).toEqual(result);
  });
});
