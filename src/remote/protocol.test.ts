import { describe, expect, it } from "vitest";
import { FrameKind, Inner, Joiner, MAX_PART, decodeFrame, decodeInner, encodeFrame, encodeTerm, splitJson } from "./protocol";

const hex = (b: Uint8Array) => [...b].map((x) => x.toString(16).padStart(2, "0")).join("");

describe("frames", () => {
  it("round trips and matches the bytes the Rust side tests against", () => {
    const f = encodeFrame({ kind: FrameKind.Data, peer: "d:a", payload: new Uint8Array([7, 8]) });
    expect(hex(f)).toBe("010303643a610708");
    expect(decodeFrame(f)).toEqual({ kind: FrameKind.Data, peer: "d:a", payload: new Uint8Array([7, 8]) });
  });
  it("refuses another version and a cut frame", () => {
    expect(decodeFrame(new Uint8Array([2, 3, 0]))).toBeNull();
    expect(decodeFrame(new Uint8Array([1, 3, 9, 1]))).toBeNull();
  });
});

describe("inner", () => {
  it("splits a long message and joins it back", () => {
    const text = "é".repeat(MAX_PART);
    const parts = splitJson(text);
    expect(parts.length).toBeGreaterThan(1);
    expect(parts.slice(0, -1).every((p) => p[0] === Inner.JsonPart)).toBe(true);
    expect(parts[parts.length - 1][0]).toBe(Inner.Json);
    const j = new Joiner();
    const got = parts.map((p) => j.add(p)).filter((x) => x !== null);
    expect(got).toEqual([text]);
  });
  it("carries terminal bytes with their agent", () => {
    const m = encodeTerm(Inner.TermData, "a1", new Uint8Array([1, 2]));
    expect(decodeInner(m)).toEqual({ kind: Inner.TermData, agent: "a1", bytes: new Uint8Array([1, 2]) });
  });
});
