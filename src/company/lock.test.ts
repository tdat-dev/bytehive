import { describe, it, expect } from "vitest";
import { LOCK_ABANDONED_MS, LOCK_STALE_MS, lockMove, lockSilent, parseLock } from "./lock";

const NOW = 1_000_000;

describe("lockMove", () => {
  const MIN = 60_000;
  const held = (silent: number) => ({ pid: 9, at: NOW - silent });
  it("runs when nobody holds the lock", () => {
    expect(lockMove(null, 7, NOW, false)).toBe("run");
  });
  it("stays out while another app's heartbeat is fresh, alive or not", () => {
    expect(lockMove({ pid: 9, at: NOW }, 7, NOW, true)).toBe("stay-out");
    expect(lockMove(held(LOCK_STALE_MS - 1), 7, NOW, true)).toBe("stay-out");
    expect(lockMove(held(LOCK_STALE_MS - 1), 7, NOW, false)).toBe("stay-out");
  });
  it("takes the lock of a dead holder after 15 seconds with no heartbeat", () => {
    expect(LOCK_STALE_MS).toBe(15_000);
    expect(lockMove(held(LOCK_STALE_MS), 7, NOW, false)).toBe("run");
  });
  it("stays out for a live holder silent 1 minute: its panes are still running", () => {
    expect(lockMove(held(MIN), 7, NOW, true)).toBe("stay-out");
    expect(lockMove(held(LOCK_ABANDONED_MS - 1), 7, NOW, true)).toBe("stay-out");
  });
  it("takes over from a live holder silent 11 minutes, and says it did", () => {
    expect(LOCK_ABANDONED_MS).toBe(10 * MIN);
    expect(lockMove(held(LOCK_ABANDONED_MS), 7, NOW, true)).toBe("take-over");
    expect(lockMove(held(11 * MIN), 7, NOW, true)).toBe("take-over");
  });
  it("does not count a sleep as silence: the holder gets to wake up too", () => {
    expect(lockMove(held(480 * MIN), 7, NOW, true, NOW - 1500)).toBe("stay-out");
    expect(lockMove(held(480 * MIN), 7, NOW, true, NOW - LOCK_ABANDONED_MS)).toBe("take-over");
    expect(lockMove(held(480 * MIN), 7, NOW, false, NOW - 1500)).toBe("run");
    expect(lockMove({ pid: 9, at: NOW + LOCK_ABANDONED_MS }, 7, NOW, true, NOW - 1500)).toBe("stay-out");
  });
  it("skips the beat on a lock it could not read", () => {
    expect(lockMove("unknown", 7, NOW, false)).toBe("skip");
    expect(lockMove("unknown", 7, NOW, true)).toBe("skip");
  });
  it("knows its own lock, however fresh: that is a window that reloaded", () => {
    expect(lockMove({ pid: 7, at: NOW }, 7, NOW, true)).toBe("run");
    expect(lockMove({ pid: 7, at: NOW - 11 * MIN }, 7, NOW, true)).toBe("run");
  });
  it("does not wait forever on a heartbeat from far ahead of the clock", () => {
    expect(lockMove({ pid: 9, at: NOW + LOCK_STALE_MS }, 7, NOW, false)).toBe("run");
    expect(lockMove({ pid: 9, at: NOW + LOCK_ABANDONED_MS }, 7, NOW, true)).toBe("take-over");
    expect(lockMove({ pid: 9, at: NOW + 1000 }, 7, NOW, false)).toBe("stay-out");
  });
});

describe("lockSilent", () => {
  it("is another app's lock with no heartbeat for 15 seconds: only then is the process looked up", () => {
    expect(lockSilent({ pid: 9, at: NOW - LOCK_STALE_MS }, 7, NOW)).toBe(true);
    expect(lockSilent({ pid: 9, at: NOW - 1 }, 7, NOW)).toBe(false);
    expect(lockSilent({ pid: 7, at: NOW - LOCK_STALE_MS }, 7, NOW)).toBe(false);
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
