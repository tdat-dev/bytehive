import { afterEach, describe, expect, it, vi } from "vitest";
import { OpTimeout, withLimit } from "./queue";

describe("withLimit", () => {
  afterEach(() => { vi.useRealTimers(); });
  it("passes a result through", async () => {
    await expect(withLimit(async () => 7, 1000, "x")).resolves.toBe(7);
  });
  it("passes a failure through", async () => {
    await expect(withLimit(async () => { throw new Error("no"); }, 1000, "x")).rejects.toThrow("no");
  });
  it("gives up at the limit and says what it gave up on", async () => {
    vi.useFakeTimers();
    const p = withLimit(() => new Promise<void>(() => {}), 2000, "Starting a plan");
    const seen = expect(p).rejects.toThrow(OpTimeout);
    await vi.advanceTimersByTimeAsync(2000);
    await seen;
    await expect(p).rejects.toThrow("Starting a plan took over 2 s, so ByteHive moved on. It may still finish.");
  });
});
