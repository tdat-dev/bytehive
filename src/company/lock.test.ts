import { describe, it, expect } from "vitest";
import { LOCK_STALE_MS, mayRun, parseLock } from "./lock";

const NOW = 1_000_000;

describe("mayRun", () => {
  it("runs when nobody holds the lock", () => {
    expect(mayRun(null, 7, NOW)).toBe(true);
  });
  it("stays out while another app's heartbeat is fresh", () => {
    expect(mayRun({ pid: 9, at: NOW }, 7, NOW)).toBe(false);
    expect(mayRun({ pid: 9, at: NOW - LOCK_STALE_MS + 1 }, 7, NOW)).toBe(false);
  });
  it("takes over after 15 seconds with no heartbeat", () => {
    expect(LOCK_STALE_MS).toBe(15_000);
    expect(mayRun({ pid: 9, at: NOW - LOCK_STALE_MS }, 7, NOW)).toBe(true);
  });
  it("knows its own lock, however fresh: that is a window that reloaded", () => {
    expect(mayRun({ pid: 7, at: NOW }, 7, NOW)).toBe(true);
  });
  it("does not wait forever on a heartbeat from far ahead of the clock", () => {
    expect(mayRun({ pid: 9, at: NOW + LOCK_STALE_MS }, 7, NOW)).toBe(true);
    expect(mayRun({ pid: 9, at: NOW + 1000 }, 7, NOW)).toBe(false);
  });
});

describe("parseLock", () => {
  it("reads a lock", () => {
    expect(parseLock(JSON.stringify({ pid: 9, at: 5 }))).toEqual({ pid: 9, at: 5 });
  });
  it.each(["", "{ half", "null", "{}", '{"pid":"9","at":5}', '{"pid":9}'])("takes %j for no lock", (text) => {
    expect(parseLock(text)).toBeNull();
  });
});
