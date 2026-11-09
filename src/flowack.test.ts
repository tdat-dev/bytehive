import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createAcker, ACK_MS, ACK_MIN_MS, ACK_STEP } from "./flowack";

describe("createAcker", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
  });
  afterEach(() => vi.useRealTimers());

  it("tells the first parse after a quiet spell at once", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsedTo(100);
    expect(send).toHaveBeenCalledExactlyOnceWith(100);
  });

  it("sends running totals, one per interval while output keeps coming", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsedTo(100); // sent now
    vi.advanceTimersByTime(10);
    a.parsedTo(300);
    vi.advanceTimersByTime(10);
    a.parsedTo(600);
    expect(send).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(ACK_MS);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenLastCalledWith(600);
  });

  it("once a step is owed, goes out as soon as the shortest gap allows, not sooner", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsedTo(1); // sent now
    a.parsedTo(1 + ACK_STEP);
    expect(send).toHaveBeenCalledTimes(1); // not twice in the same moment
    vi.advanceTimersByTime(ACK_MIN_MS);
    expect(send).toHaveBeenLastCalledWith(1 + ACK_STEP);
  });

  it("never sends more than one ack per shortest gap, however fast the parses come", () => {
    const send = vi.fn();
    const a = createAcker(send);
    let total = 0;
    for (let ms = 0; ms < 1000; ms++) {
      total += ACK_STEP; // a huge stream: a step owed every millisecond
      a.parsedTo(total);
      vi.advanceTimersByTime(1);
    }
    expect(send.mock.calls.length).toBeLessThanOrEqual(1000 / ACK_MIN_MS + 1);
    a.flush();
    expect(send).toHaveBeenLastCalledWith(total);
  });

  it("a late or repeated total changes nothing", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsedTo(500);
    a.parsedTo(200);
    a.parsedTo(500);
    vi.advanceTimersByTime(1000);
    expect(send).toHaveBeenCalledExactlyOnceWith(500);
  });

  it("does nothing for nothing, and stops cleanly", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsedTo(0);
    a.flush();
    expect(send).not.toHaveBeenCalled();
    a.parsedTo(10);
    a.parsedTo(20);
    a.dispose();
    vi.advanceTimersByTime(1000);
    expect(send).toHaveBeenCalledTimes(1); // the first, at once; the second went with the pane
  });
});
