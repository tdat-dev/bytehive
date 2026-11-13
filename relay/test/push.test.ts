import { env, exports } from "cloudflare:workers";
import { afterEach, describe, expect, it, vi } from "vitest";
import { allowedEndpoint, vapidHeader } from "../src/push";

const get = (p: string, init?: RequestInit) => exports.default.fetch(`https://relay.test${p}`, { redirect: "manual", ...init });
const local = (p: string, init?: RequestInit) => exports.default.fetch(`https://127.0.0.1${p}`, { redirect: "manual", ...init });
const FCM = "https://fcm.googleapis.com/fcm/send/abc";
const settle = (ms = 100) => new Promise((r) => setTimeout(r, ms));

async function login(account: string, peer: string): Promise<string> {
  const r = await local(`/auth/start?account=${account}&peer=${peer}&state=s&port=17641`);
  const to = new URL(r.headers.get("Location")!);
  return to.searchParams.get("token") ?? decodeURIComponent(to.hash.replace(/^#token=/, ""));
}

const post = (path: string, token: string, body?: unknown) =>
  get(path, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

async function connect(account: string, peer: string) {
  const token = await login(account, peer);
  const { ticket } = (await (await post("/ticket", token, { name: peer })).json()) as { ticket: string };
  const r = await get(`/ws?ticket=${encodeURIComponent(ticket)}`, { headers: { Upgrade: "websocket" } });
  const ws = r.webSocket!;
  ws.accept();
  return ws;
}

const pending = async (token: string) => (await post("/push/pending", token)).json();

/** The calls the Room made to a push service. */
function pushService(status = 201) {
  const calls: { url: string; headers: Headers; body: BodyInit | null | undefined }[] = [];
  vi.stubGlobal("fetch", async (url: string, init: RequestInit) => {
    calls.push({ url, headers: new Headers(init.headers), body: init.body });
    return new Response(null, { status });
  });
  return calls;
}

afterEach(() => vi.unstubAllGlobals());

describe("vapid", () => {
  it("signs a header the push service can check, for its own origin", async () => {
    const h = await vapidHeader("https://push.example/send/1", env.VAPID_PRIVATE_JWK, env.VAPID_PUBLIC, "mailto:a@b.c");
    const m = h.match(/^vapid t=([\w-]+)\.([\w-]+)\.([\w-]+), k=(.+)$/)!;
    expect(m[4]).toBe(env.VAPID_PUBLIC);
    const unb64 = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));
    const claims = JSON.parse(new TextDecoder().decode(unb64(m[2])));
    expect(claims.aud).toBe("https://push.example");
    expect(claims.sub).toBe("mailto:a@b.c");
    expect(claims.exp).toBeGreaterThan(Date.now() / 1000);
    expect(claims.exp).toBeLessThanOrEqual(Date.now() / 1000 + 24 * 3600);
    const pub = await crypto.subtle.importKey("raw", unb64(env.VAPID_PUBLIC), { name: "ECDSA", namedCurve: "P-256" }, false, ["verify"]);
    const ok = await crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, pub, unb64(m[3]), new TextEncoder().encode(`${m[1]}.${m[2]}`));
    expect(ok).toBe(true);
  });

  it("only sends to the push services of browsers", () => {
    for (const ok of [FCM, "https://updates.push.services.mozilla.com/wpush/v2/x", "https://web.push.apple.com/x", "https://wns2-par02p.notify.windows.com/w/?token=x"]) {
      expect(allowedEndpoint(ok, false), ok).toBe(true);
    }
    for (const no of ["https://example.com/x", "https://fcm.googleapis.com.evil.test/x", "https://evilgoogleapis.com/x", "http://fcm.googleapis.com/x", "https://user@fcm.googleapis.com/x", "not a url"]) {
      expect(allowedEndpoint(no, false), no).toBe(false);
    }
  });

  it("lets a local run send to a loopback port, and nothing else plain", () => {
    expect(allowedEndpoint("http://127.0.0.1:18790/push", true)).toBe(true);
    expect(allowedEndpoint("http://127.0.0.1:18790/push", false)).toBe(false);
    expect(allowedEndpoint("http://example.com/push", true)).toBe(false);
  });
});

describe("a held note", () => {
  it("keeps the latest note for a device and gives it out once", async () => {
    const room = env.ROOM.getByName("p1");
    await room.holdPush("d:x", "m1", "first");
    await room.holdPush("d:x", "m1", "second");
    expect(await room.takePush("d:x")).toEqual({ sealed: "second", machine: "m1" });
    expect(await room.takePush("d:x")).toBeNull();
  });
});

describe("routes", () => {
  it("tells a page the public key", async () => {
    expect(await (await get("/push/key")).json()).toEqual({ key: env.VAPID_PUBLIC });
    expect((await get("/push/key", { method: "OPTIONS" })).status).toBe(204);
  });

  it("is for a signed-in device only", async () => {
    for (const path of ["/push/subscribe", "/push/pending"]) {
      expect((await post(path, "")).status, path).toBe(401);
      expect((await post(path, await login("p2", "m:one"), { endpoint: FCM })).status, `${path} by a machine`).toBe(401);
      expect((await get(path)).status, `${path} by GET`).toBe(405);
    }
  });

  it("takes an endpoint of a push service and refuses any other address", async () => {
    const t = await login("p3", "d:one");
    expect((await post("/push/subscribe", t, { endpoint: FCM })).status).toBe(200);
    for (const endpoint of ["https://example.com/x", "http://fcm.googleapis.com/x", 7, undefined]) {
      expect((await post("/push/subscribe", t, { endpoint })).status, String(endpoint)).toBe(400);
    }
  });

  it("gives a device its own note, once, and never another device's", async () => {
    const room = env.ROOM.getByName("p4");
    await room.holdPush("d:one", "m1", "for one");
    await room.holdPush("d:two", "m1", "for two");
    const one = await login("p4", "d:one");
    expect(await pending(one)).toEqual({ sealed: "for one", machine: "m1" });
    expect(await pending(one)).toBeNull();
    expect(await pending(await login("p4", "d:two"))).toEqual({ sealed: "for two", machine: "m1" });
    expect(await pending(await login("other", "d:one"))).toBeNull();
  });

  it("turns a removed device away", async () => {
    const machine = await connect("p5", "m:one");
    const t = await login("p5", "d:gone");
    await connect("p5", "d:gone");
    machine.send(JSON.stringify({ t: "revoke", peer: "d:gone" }));
    await settle();
    expect((await post("/push/subscribe", t, { endpoint: FCM })).status).toBe(403);
    expect((await post("/push/pending", t)).status).toBe(403);
  });
});

describe("telling a device that is not here", () => {
  async function subscribed(account: string, device: string) {
    const t = await login(account, device);
    await post("/push/subscribe", t, { endpoint: FCM });
    return t;
  }

  it("holds the sealed note and wakes the browser with an empty push", async () => {
    const calls = pushService();
    const t = await subscribed("p6", "d:phone");
    const machine = await connect("p6", "m:desk");
    machine.send(JSON.stringify({ t: "push", peer: "d:phone", sealed: "SEALED" }));
    await settle();
    expect(calls).toHaveLength(1);
    expect(calls[0].url).toBe(FCM);
    expect(calls[0].body ?? null).toBeNull();
    expect(calls[0].headers.get("Authorization")).toMatch(/^vapid t=.+, k=/);
    expect(await pending(t)).toEqual({ sealed: "SEALED", machine: "desk" });
  });

  it("stays quiet when the device is connected: it already hears the machine", async () => {
    const calls = pushService();
    const t = await subscribed("p7", "d:phone");
    await connect("p7", "d:phone");
    const machine = await connect("p7", "m:desk");
    machine.send(JSON.stringify({ t: "push", peer: "d:phone", sealed: "SEALED" }));
    await settle();
    expect(calls).toHaveLength(0);
    expect(await pending(t)).toBeNull();
  });

  it("holds nothing for a device that never asked, or for a peer it does not know", async () => {
    const calls = pushService();
    await login("p8", "d:quiet");
    const machine = await connect("p8", "m:desk");
    machine.send(JSON.stringify({ t: "push", peer: "d:quiet", sealed: "SEALED" }));
    machine.send(JSON.stringify({ t: "push", peer: "d:nobody", sealed: "SEALED" }));
    machine.send(JSON.stringify({ t: "push", peer: "m:desk", sealed: "SEALED" }));
    await settle();
    expect(calls).toHaveLength(0);
    expect(await pending(await login("p8", "d:quiet"))).toBeNull();
  });

  it("is for a machine to say, and a note too big for the line is dropped, not cut", async () => {
    const calls = pushService();
    const t = await subscribed("p9", "d:phone");
    const device = await connect("p9", "d:other");
    device.send(JSON.stringify({ t: "push", peer: "d:phone", sealed: "FORGED" }));
    const machine = await connect("p9", "m:desk");
    machine.send(JSON.stringify({ t: "push", peer: "d:phone", sealed: "x".repeat(5000) }));
    await settle();
    expect(calls).toHaveLength(0);
    expect(await pending(t)).toBeNull();
  });

  it("forgets a subscription the push service says is gone", async () => {
    const calls = pushService(410);
    const t = await subscribed("p10", "d:phone");
    const machine = await connect("p10", "m:desk");
    for (let i = 0; i < 2; i++) {
      machine.send(JSON.stringify({ t: "push", peer: "d:phone", sealed: `S${i}` }));
      await settle();
    }
    expect(calls).toHaveLength(1);
    expect(await pending(t)).toEqual({ sealed: "S0", machine: "desk" });
  });

  it("still holds the note when the push service can't be reached", async () => {
    vi.stubGlobal("fetch", async () => { throw new Error("offline"); });
    const t = await subscribed("p11", "d:phone");
    const machine = await connect("p11", "m:desk");
    machine.send(JSON.stringify({ t: "push", peer: "d:phone", sealed: "SEALED" }));
    await settle();
    expect(await pending(t)).toEqual({ sealed: "SEALED", machine: "desk" });
  });

  it("drops what waits for a device once it is removed, and its subscription", async () => {
    const calls = pushService();
    await subscribed("p12", "d:phone");
    await connect("p12", "d:phone");
    const machine = await connect("p12", "m:desk");
    await env.ROOM.getByName("p12").holdPush("d:phone", "desk", "SEALED");
    machine.send(JSON.stringify({ t: "revoke", peer: "d:phone" }));
    await settle();
    expect(await env.ROOM.getByName("p12").takePush("d:phone")).toBeNull();
    machine.send(JSON.stringify({ t: "push", peer: "d:phone", sealed: "AGAIN" }));
    await settle();
    expect(calls).toHaveLength(0);
    expect(await env.ROOM.getByName("p12").takePush("d:phone")).toBeNull();
  });
});
