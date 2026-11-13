import { env } from "cloudflare:workers";
import { describe, expect, it } from "vitest";
import { LIMITS } from "../src/limits";

async function join(room: string, peer: string, name = peer) {
  const stub = env.ROOM.getByName(room);
  const r = await stub.fetch(`https://r/ws?peer=${peer}&name=${name}`, { headers: { Upgrade: "websocket" } });
  const ws = r.webSocket;
  if (!ws) return { status: r.status, ws: null as never, next: async () => null as never };
  ws.accept();
  ws.binaryType = "arraybuffer";
  const queue: (string | ArrayBuffer)[] = [];
  const waiters: ((v: string | ArrayBuffer) => void)[] = [];
  ws.addEventListener("message", (e) => {
    const w = waiters.shift();
    if (w) w(e.data); else queue.push(e.data);
  });
  const next = () => new Promise<string | ArrayBuffer>((res) => {
    const m = queue.shift();
    if (m !== undefined) res(m); else waiters.push(res);
  });
  return { status: 101, ws, next };
}

const frame = (to: string, ...payload: number[]) => {
  const p = new TextEncoder().encode(to);
  return new Uint8Array([1, 3, p.length, ...p, ...payload]);
};
const nextBinary = async (c: { next(): Promise<string | ArrayBuffer> }) => {
  for (;;) { const m = await c.next(); if (typeof m !== "string") return new Uint8Array(m); }
};

describe("room", () => {
  it("routes a frame and names the sender, not the receiver", async () => {
    const a = await join("r1", "m:one");
    const b = await join("r1", "d:two");
    b.ws.send(frame("m:one", 5, 6));
    const got = await nextBinary(a);
    expect([...got]).toEqual([...frame("d:two", 5, 6)]);
  });
  it("tells everyone who is here", async () => {
    const a = await join("r2", "m:one", "Desk");
    await a.next();
    await join("r2", "d:two", "Phone");
    const m = JSON.parse((await a.next()) as string);
    expect(m.t).toBe("peers");
    expect(m.peers.map((p: { name: string }) => p.name).sort()).toEqual(["Desk", "Phone"]);
  });
  it("never lets two accounts see each other", async () => {
    const a = await join("r3a", "m:one");
    const b = await join("r3b", "d:two");
    b.ws.send(frame("m:one", 1));
    const raced = await Promise.race([nextBinary(a), new Promise((r) => setTimeout(() => r("quiet"), 100))]);
    expect(raced).toBe("quiet");
  });
  it("a revoked device is hung up and stays out", async () => {
    const m = await join("r4", "m:one");
    const d = await join("r4", "d:two");
    const closed = new Promise((r) => d.ws.addEventListener("close", r));
    m.ws.send(JSON.stringify({ t: "revoke", peer: "d:two" }));
    await closed;
    expect((await join("r4", "d:two")).status).toBe(403);
  });
  it("a device can't revoke anyone", async () => {
    await join("r5", "m:one");
    const d = await join("r5", "d:two");
    d.ws.send(JSON.stringify({ t: "revoke", peer: "m:one" }));
    await new Promise((r) => setTimeout(r, 50));
    expect((await join("r5", "m:one")).status).toBe(101);
  });
  it("turns away a device past the limit", async () => {
    for (let i = 0; i < LIMITS.devices; i++) await join("r6", `d:${i}`);
    expect((await join("r6", "d:extra")).status).toBe(429);
  });
});
