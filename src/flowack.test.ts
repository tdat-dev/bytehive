import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createAcker, ACK_MS, ACK_STEP } from "./flowack";

describe("createAcker", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
  });
  afterEach(() => vi.useRealTimers());

  it("acks the first chunk after a quiet spell at once", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsed(100);
    expect(send).toHaveBeenCalledExactlyOnceWith(100);
  });

  it("joins what is parsed while output keeps coming into one ack per interval", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsed(100); // sent now
    vi.advanceTimersByTime(10);
    a.parsed(200);
    vi.advanceTimersByTime(10);
    a.parsed(300);
    expect(send).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(ACK_MS);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenLastCalledWith(500);
  });

  it("does not wait for the clock once a step is owed", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsed(1);
    a.parsed(ACK_STEP);
    expect(send).toHaveBeenLastCalledWith(ACK_STEP);
  });

  it("never loses a byte: everything parsed is acked in the end", () => {
    const send = vi.fn();
    const a = createAcker(send);
    let total = 0;
    for (let i = 1; i <= 50; i++) { a.parsed(i * 7); total += i * 7; vi.advanceTimersByTime(3); }
    a.flush();
    expect(send.mock.calls.reduce((n, c) => n + c[0], 0)).toBe(total);
  });

  it("does nothing for nothing, and stops cleanly", () => {
    const send = vi.fn();
    const a = createAcker(send);
    a.parsed(0);
    a.flush();
    expect(send).not.toHaveBeenCalled();
    a.parsed(10);
    a.parsed(10);
    a.dispose();
    vi.advanceTimersByTime(1000);
    expect(send).toHaveBeenCalledTimes(1); // the first, at once; the second was dropped with the pane
  });
});
