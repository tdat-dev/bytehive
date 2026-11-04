import { afterEach, describe, expect, it, vi } from "vitest";
import { createPacer, setCalmForTest } from "./calm";

describe("calm pacer", () => {
  afterEach(() => { setCalmForTest(false); vi.useRealTimers(); });

  it("passes output straight through while Maestro is in use", () => {
    const out: string[] = [];
    const p = createPacer<string>((c) => out.push(c));
    p.push("a");
    p.push("b");
    expect(out).toEqual(["a", "b"]);
    p.dispose();
  });

  it("holds output in the background and hands it over a few times a second, in order", () => {
    vi.useFakeTimers();
    const out: string[] = [];
    const p = createPacer<string>((c) => out.push(c), 250);
    setCalmForTest(true);
    p.push("a");
    p.push("b");
    expect(out).toEqual([]);
    vi.advanceTimersByTime(250);
    expect(out).toEqual(["a", "b"]);
    p.push("c");
    expect(out).toEqual(["a", "b"]);
    // coming back draws what was held at once
    setCalmForTest(false);
    expect(out).toEqual(["a", "b", "c"]);
    p.push("d");
    expect(out).toEqual(["a", "b", "c", "d"]);
    p.dispose();
  });
});
