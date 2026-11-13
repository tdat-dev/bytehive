import { describe, expect, it } from "vitest";
import { FrameKind, Inner, decodeFrame, encodeTerm, splitJson } from "../../src/remote/protocol";
import { MachineSession, type NoiseSession } from "./session";

class FakeNoise implements NoiseSession {
  step = 0; ready = false; remoteKey: Uint8Array | undefined = new Uint8Array([9]);
  machine = "1";
  write() { this.step++; if (this.step >= 2) this.ready = true; return new Uint8Array([this.step]); }
  read() { return new TextEncoder().encode(JSON.stringify({ machine: this.machine, name: "Desk" })); }
  encrypt(p: Uint8Array) { return p; }
  decrypt(c: Uint8Array) { return c; }
}

function setup(known = new Uint8Array([9]), machine = "1") {
  const sent: Uint8Array[] = [];
  const noise = new FakeNoise();
  noise.machine = machine;
  const s = new MachineSession({
    peer: "m:1",
    noise,
    pairing: false,
    expectKey: known,
    sendFrame: (b) => { sent.push(b); },
  });
  return { s, sent };
}

describe("machine session", () => {
  it("starts with a handshake frame and finishes on the reply", () => {
    const { s, sent } = setup();
    s.start();
    expect(decodeFrame(sent[0])!.kind).toBe(FrameKind.Handshake);
    s.frame({ kind: FrameKind.Handshake, peer: "m:1", payload: new Uint8Array([1]) });
    expect(sent).toHaveLength(2);
    expect(s.open).toBe(true);
  });
  it("hangs up on a machine with another key", () => {
    const { s } = setup(new Uint8Array([8]));
    let closed = "";
    s.onClosed = (why) => { closed = why; };
    s.start();
    s.frame({ kind: FrameKind.Handshake, peer: "m:1", payload: new Uint8Array([1]) });
    expect(closed).toBe("wrong-key");
    expect(s.open).toBe(false);
  });
  it("hangs up on a machine that names another id than the one dialled, before message 3", () => {
    const { s, sent } = setup(undefined, "victim");
    let closed = "";
    s.onClosed = (why) => { closed = why; };
    s.start();
    s.frame({ kind: FrameKind.Handshake, peer: "m:1", payload: new Uint8Array([1]) });
    expect(closed).toBe("wrong-key");
    expect(s.open).toBe(false);
    expect(s.machine).toBeNull();
    expect(sent).toHaveLength(1);
  });
  it("hangs up on that too while pairing, where no key is expected yet", () => {
    const sent: Uint8Array[] = [];
    const noise = new FakeNoise();
    noise.machine = "victim";
    const s = new MachineSession({ peer: "m:1", noise, pairing: true, sendFrame: (b) => { sent.push(b); } });
    let closed = "";
    s.onClosed = (why) => { closed = why; };
    s.start();
    s.frame({ kind: FrameKind.HandshakePair, peer: "m:1", payload: new Uint8Array([1]) });
    expect(closed).toBe("wrong-key");
    expect(sent).toHaveLength(1);
  });
  it("hands up whole JSON messages and terminal bytes", () => {
    const { s } = setup();
    s.start();
    s.frame({ kind: FrameKind.Handshake, peer: "m:1", payload: new Uint8Array([1]) });
    const got: unknown[] = [];
    s.onMessage = (m) => got.push(m);
    s.onTerm = (agent, bytes, reset) => got.push([agent, [...bytes], reset]);
    for (const p of splitJson('{"type":"hello","protocol":1,"machine":"Desk"}')) s.frame({ kind: FrameKind.Data, peer: "m:1", payload: p });
    s.frame({ kind: FrameKind.Data, peer: "m:1", payload: encodeTerm(Inner.TermData, "a1", new Uint8Array([5])) });
    expect(got).toEqual([{ type: "hello", protocol: 1, machine: "Desk" }, ["a1", [5], false]]);
  });
  it("closes on a machine speaking a newer protocol", () => {
    const { s } = setup();
    let closed = "";
    s.onClosed = (why) => { closed = why; };
    s.start();
    s.frame({ kind: FrameKind.Handshake, peer: "m:1", payload: new Uint8Array([1]) });
    for (const p of splitJson('{"type":"hello","protocol":99,"machine":"Desk"}')) s.frame({ kind: FrameKind.Data, peer: "m:1", payload: p });
    expect(closed).toBe("update");
  });
});
