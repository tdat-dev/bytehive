import { env, exports } from "cloudflare:workers";
import { afterEach, describe, expect, it, vi } from "vitest";
import worker from "../src/index";
import { sign, verify } from "../src/token";

const secret = "test-secret-test-secret-test-secret";
// Redirects are the thing under test, so never follow them.
const get = (p: string, init?: RequestInit) => exports.default.fetch(`https://relay.test${p}`, { redirect: "manual", ...init });
const local = (p: string, init?: RequestInit) => exports.default.fetch(`https://127.0.0.1${p}`, { redirect: "manual", ...init });
const enc = encodeURIComponent;

/** Sign in through the dev shortcut and return the login token from the redirect. */
async function login(account: string, peer: string) {
  const r = await local(`/auth/start?account=${account}&peer=${peer}&state=s&port=17641`);
  expect(r.status).toBe(302);
  const to = new URL(r.headers.get("Location")!);
  return to.searchParams.get("token") ?? decodeURIComponent(to.hash.replace(/^#token=/, ""));
}

const ticketFor = (token: string, body: unknown = { name: "x" }) =>
  get("/ticket", {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

async function connect(account: string, peer: string, name = peer) {
  const { ticket } = (await (await ticketFor(await login(account, peer), { name })).json()) as { ticket: string };
  const r = await get(`/ws?ticket=${enc(ticket)}`, { headers: { Upgrade: "websocket" } });
  const ws = r.webSocket!;
  ws.accept();
  ws.binaryType = "arraybuffer";
  const frames: Uint8Array[] = [];
  ws.addEventListener("message", (e) => { if (typeof e.data !== "string") frames.push(new Uint8Array(e.data)); });
  return { status: r.status, ws, frames };
}

const frame = (to: string, ...payload: number[]) => {
  const p = new TextEncoder().encode(to);
  return new Uint8Array([1, 3, p.length, ...p, ...payload]);
};
const settle = () => new Promise((r) => setTimeout(r, 100));

afterEach(() => vi.unstubAllGlobals());

describe("sign-in", () => {
  it("refuses a bad peer, and a machine without a usable port", async () => {
    expect((await get("/auth/start?peer=x:1&state=s")).status).toBe(400);
    expect((await get("/auth/start?peer=m:a&state=s")).status).toBe(400);
    expect((await get("/auth/start?peer=m:a&state=s&port=80")).status).toBe(400);
    expect((await get("/auth/start?peer=m:a&state=s&port=70000")).status).toBe(400);
    expect((await get("/auth/start?peer=m:a&state=s&port=abc")).status).toBe(400);
    expect((await get("/auth/start?peer=d:a&state=s")).status).toBe(302);
  });

  it("sends the login only to the machine's own loopback port, with its state", async () => {
    const start = await get(`/auth/start?peer=m:desk&state=${enc("my state")}&port=17641`);
    const state = new URL(start.headers.get("Location")!).searchParams.get("state")!;
    vi.stubGlobal("fetch", async (u: string) =>
      u.includes("access_token") ? Response.json({ access_token: "gh" }) : Response.json({ id: 99 }));
    const r = await worker.fetch(new Request(`https://relay.test/auth/callback?code=c&state=${enc(state)}`), env);
    const to = new URL(r.headers.get("Location")!);
    expect(r.status).toBe(302);
    expect(to.origin).toBe("http://127.0.0.1:17641");
    expect(to.pathname).toBe("/callback");
    expect(to.searchParams.get("state")).toBe("my state");
    expect(await verify(to.searchParams.get("token")!, secret, "login")).toMatchObject({ account: "99", peer: "m:desk" });
  });

  it("puts a device's login in the fragment of the app page", async () => {
    const start = await get("/auth/start?peer=d:phone&state=s");
    const state = new URL(start.headers.get("Location")!).searchParams.get("state")!;
    vi.stubGlobal("fetch", async (u: string) =>
      u.includes("access_token") ? Response.json({ access_token: "gh" }) : Response.json({ id: 5 }));
    const r = await worker.fetch(new Request(`https://relay.test/auth/callback?code=c&state=${enc(state)}`), env);
    const loc = r.headers.get("Location")!;
    expect(loc.startsWith(`${env.APP_ORIGIN}/#token=`)).toBe(true);
    const token = decodeURIComponent(loc.split("#token=")[1]);
    expect(await verify(token, secret, "login")).toMatchObject({ account: "5", peer: "d:phone" });
  });

  it("refuses a callback with a state it didn't sign, or a login passed as one", async () => {
    vi.stubGlobal("fetch", async () => Response.json({ id: 1, access_token: "gh" }));
    const bad = await worker.fetch(new Request("https://relay.test/auth/callback?code=c&state=nonsense"), env);
    expect(bad.status).toBe(400);
    const asLogin = await sign({ typ: "login", account: "1", peer: "m:a" }, 60, secret);
    const r = await worker.fetch(new Request(`https://relay.test/auth/callback?code=c&state=${enc(asLogin)}`), env);
    expect(r.status).toBe(400);
  });

  it("goes to GitHub unless DEV_LOGIN is 1 and the host is loopback", async () => {
    const github = (r: Response) => (r.headers.get("Location") ?? "").startsWith("https://github.com/login/oauth/authorize");
    const q = "/auth/start?peer=d:a&state=s&account=evil";
    const req = (host: string) => new Request(`https://${host}${q}`);
    expect(github(await worker.fetch(req("127.0.0.1"), { ...env, DEV_LOGIN: undefined } as unknown as Env))).toBe(true);
    expect(github(await worker.fetch(req("127.0.0.1"), { ...env, DEV_LOGIN: "0" } as unknown as Env))).toBe(true);
    expect(github(await worker.fetch(req("relay.test"), env))).toBe(true);
    expect(github(await worker.fetch(req("127.0.0.1"), env))).toBe(false);
  });

  it("has no claim or dev routes", async () => {
    expect((await get("/auth/claim?state=x")).status).toBe(404);
    expect((await get("/auth/dev?account=1&kind=machine&state=x")).status).toBe(404);
  });
});

describe("ticket", () => {
  it("takes a login and nothing else as the Bearer", async () => {
    const login_ = await login("7", "d:abc");
    const ok = await ticketFor(login_);
    expect(ok.status).toBe(200);
    const { ticket } = (await ok.json()) as { ticket: string };
    expect(await verify(ticket, secret, "ticket")).toMatchObject({ account: "7", peer: "d:abc", name: "x" });

    expect((await ticketFor("nonsense")).status).toBe(401);
    expect((await ticketFor(ticket)).status).toBe(401);
    const state = new URL((await get("/auth/start?peer=d:a&state=s")).headers.get("Location")!).searchParams.get("state")!;
    expect((await ticketFor(state)).status).toBe(401);
    expect((await get("/ticket")).status).toBe(405);
  });

  it("takes the peer from the token, not the body, and survives a bad body", async () => {
    const t = await login("8", "d:mine");
    const { ticket } = (await (await ticketFor(t, { peer: "m:other", name: "y".repeat(200) })).json()) as { ticket: string };
    const claims = await verify(ticket, secret, "ticket");
    expect(claims).toMatchObject({ account: "8", peer: "d:mine" });
    expect((claims!.name as string).length).toBe(80);
    const raw = await get("/ticket", { method: "POST", headers: { Authorization: `Bearer ${t}` }, body: "{not json" });
    expect(raw.status).toBe(200);
  });

  it("refuses a revoked peer, whatever id the body asks for", async () => {
    const m = await connect("9", "m:desk");
    const d = await connect("9", "d:phone");
    const hungUp = new Promise((r) => d.ws.addEventListener("close", r));
    const phone = await login("9", "d:phone");
    m.ws.send(JSON.stringify({ t: "revoke", peer: "d:phone" }));
    await hungUp;
    expect((await ticketFor(phone)).status).toBe(403);
    expect((await ticketFor(phone, { peer: "d:fresh" })).status).toBe(403);
  });
});

describe("socket", () => {
  it("opens only on a ticket, not on a login or a state", async () => {
    const ws = (t: string) => get(`/ws?ticket=${enc(t)}`, { headers: { Upgrade: "websocket" } });
    expect((await ws("bad")).status).toBe(401);
    expect((await ws(await login("1", "d:a"))).status).toBe(401);
    expect((await ws(await sign({ typ: "state", peer: "d:a" }, 60, secret))).status).toBe(401);
    expect((await ws(await sign({ typ: "ticket", peer: "d:a" }, 60, secret))).status).toBe(401);
  });

  it("carries frames inside an account and never across two", async () => {
    const mine = await connect("A", "m:desk");
    const phone = await connect("A", "d:phone");
    const stranger = await connect("B", "d:other");

    stranger.ws.send(frame("m:desk", 1));
    await settle();
    expect(mine.frames).toEqual([]);

    phone.ws.send(frame("m:desk", 2));
    await settle();
    expect(mine.frames.map((f) => [...f])).toEqual([[...frame("d:phone", 2)]]);
  });
});
