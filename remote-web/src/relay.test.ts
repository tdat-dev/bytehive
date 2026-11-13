import { afterEach, describe, expect, it, vi } from "vitest";
import { Relay } from "./session";

class FakeSocket {
  static all: FakeSocket[] = [];
  binaryType = "";
  onopen: () => void = () => {};
  onmessage: () => void = () => {};
  onclose: (e: { code: number }) => void = () => {};
  constructor(public url: string) { FakeSocket.all.push(this); }
  close() {}
  send() {}
}

async function connected() {
  vi.stubGlobal("WebSocket", FakeSocket);
  vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, status: 200, json: async () => ({ ticket: "t" }) })));
  const relay = new Relay({ token: "x", deviceName: "Phone" });
  await relay.connect();
  FakeSocket.all[0].onopen();
  return relay;
}

describe("relay", () => {
  afterEach(() => {
    FakeSocket.all = [];
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("does not retry when another tab took the socket, and takes it back on resume", async () => {
    vi.useFakeTimers();
    const relay = await connected();
    const replaced = vi.fn();
    relay.onReplaced = replaced;
    FakeSocket.all[0].onclose({ code: 4000 });
    expect(replaced).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(FakeSocket.all).toHaveLength(1);
    relay.resume();
    await vi.advanceTimersByTimeAsync(0);
    expect(FakeSocket.all).toHaveLength(2);
  });

  it("still retries after any other close", async () => {
    vi.useFakeTimers();
    const relay = await connected();
    const replaced = vi.fn();
    relay.onReplaced = replaced;
    FakeSocket.all[0].onclose({ code: 1006 });
    await vi.advanceTimersByTimeAsync(1100);
    expect(replaced).not.toHaveBeenCalled();
    expect(FakeSocket.all).toHaveLength(2);
  });
});
