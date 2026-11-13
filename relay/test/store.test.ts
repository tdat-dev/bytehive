import { exports } from "cloudflare:workers";
import { describe, expect, it } from "vitest";
import { STORE } from "../src/limits";

const local = (p: string, init?: RequestInit) => exports.default.fetch(`https://127.0.0.1${p}`, { redirect: "manual", ...init });

/** Sign in through the dev shortcut and return the login token. */
async function login(account: string, peer: string): Promise<string> {
  const r = await local(`/auth/start?account=${account}&peer=${peer}&state=s&port=17641`);
  const to = new URL(r.headers.get("Location")!);
  return to.searchParams.get("token") ?? decodeURIComponent(to.hash.replace(/^#token=/, ""));
}

const call = (token: string, method: string, path: string, body?: unknown) =>
  local(path, {
    method,
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

const sid = (c: string) => c.repeat(32);
const vault = { salt: "c2FsdHNhbHRzYWx0c2FsdA", rounds: 600_000, check: "Y2hlY2s" };
const session = (s: string, segs: [number, string][], meta = "bWV0YQ") => ({ sid: s, meta, updated: 1000, segments: segs.map(([n, sealed]) => ({ n, sealed })) });

describe("session store", () => {
  it("keeps one vault, and only a machine sets it", async () => {
    const m = await login("v1", "m:one");
    const d = await login("v1", "d:phone");
    expect((await call(m, "GET", "/vault")).status).toBe(404);
    expect((await call(d, "PUT", "/vault", { vault })).status).toBe(403);
    expect((await call(m, "PUT", "/vault", { vault })).status).toBe(200);
    expect(await (await call(d, "GET", "/vault")).json()).toEqual(vault);
    expect((await call(m, "PUT", "/vault", { vault })).status).toBe(409);
  });

  it("turns away a vault too weak to be one", async () => {
    const m = await login("v2", "m:one");
    expect((await call(m, "PUT", "/vault", { vault: { ...vault, rounds: 1000 } })).status).toBe(400);
  });

  it("stores sessions sealed and gives them to any peer of the account, newest first", async () => {
    const m = await login("s1", "m:one");
    const d = await login("s1", "d:phone");
    await call(m, "PUT", "/vault", { vault });
    expect((await call(m, "POST", "/store/put", session(sid("a"), [[0, "AAAA"], [1, "BBBB"]]))).status).toBe(200);
    expect((await call(m, "POST", "/store/put", { ...session(sid("b"), [[0, "CCCC"]]), updated: 2000 })).status).toBe(200);
    const listed = (await (await call(d, "GET", "/store/list")).json()) as { sid: string; segments: number; machine: string }[];
    expect(listed.map((s) => [s.sid, s.segments, s.machine])).toEqual([[sid("b"), 1, "m:one"], [sid("a"), 2, "m:one"]]);
    const got = (await (await call(d, "GET", `/store/segments?sid=${sid("a")}&from=1`)).json()) as { segments: unknown[] };
    expect(got.segments).toEqual([{ n: 1, sealed: "BBBB" }]);
  });

  it("replaces a changed segment in place", async () => {
    const m = await login("s2", "m:one");
    await call(m, "POST", "/store/put", session(sid("a"), [[0, "AAAA"], [1, "BBBB"]]));
    await call(m, "POST", "/store/put", session(sid("a"), [[1, "DDDDDD"]]));
    const got = (await (await call(m, "GET", `/store/segments?sid=${sid("a")}&from=0`)).json()) as { segments: unknown[] };
    expect(got.segments).toEqual([{ n: 0, sealed: "AAAA" }, { n: 1, sealed: "DDDDDD" }]);
  });

  it("lets only a machine write, and only the machine that owns a session", async () => {
    const m1 = await login("s3", "m:one");
    const m2 = await login("s3", "m:two");
    const d = await login("s3", "d:phone");
    expect((await call(d, "POST", "/store/put", session(sid("a"), []))).status).toBe(403);
    await call(m1, "POST", "/store/put", session(sid("a"), [[0, "AAAA"]]));
    expect((await call(m2, "POST", "/store/put", session(sid("a"), [[0, "EEEE"]]))).status).toBe(409);
    await call(m2, "POST", "/store/drop", { sid: sid("a") });
    expect(((await (await call(d, "GET", "/store/list")).json()) as unknown[]).length).toBe(1);
  });

  it("turns away what is not sealed or is too big", async () => {
    const m = await login("s4", "m:one");
    expect((await call(m, "POST", "/store/put", session("nothex", []))).status).toBe(400);
    expect((await call(m, "POST", "/store/put", session(sid("a"), [[0, "not base64!"]]))).status).toBe(400);
    expect((await call(m, "POST", "/store/put", session(sid("a"), [[0, "A".repeat(STORE.segmentBytes + 1)]]))).status).toBe(400);
  });

  it("needs a login", async () => {
    expect((await local("/store/list")).status).toBe(401);
  });

  it("drops every session when the vault is replaced, since none can be opened any more", async () => {
    const m = await login("s5", "m:one");
    await call(m, "PUT", "/vault", { vault });
    await call(m, "POST", "/store/put", session(sid("a"), [[0, "AAAA"]]));
    expect((await call(m, "PUT", "/vault", { vault: { ...vault, salt: "bmV3c2FsdG5ld3NhbHQ" }, replace: true })).status).toBe(200);
    expect(await (await call(m, "GET", "/store/list")).json()).toEqual([]);
  });

  it("gives long sessions in pages", async () => {
    const m = await login("s6", "m:one");
    const big = "A".repeat(STORE.segmentBytes);
    const segs: [number, string][] = Array.from({ length: 6 }, (_, n) => [n, big]);
    await call(m, "POST", "/store/put", session(sid("a"), segs));
    const page = (await (await call(m, "GET", `/store/segments?sid=${sid("a")}&from=0`)).json()) as { segments: { n: number }[]; more: boolean };
    expect(page.more).toBe(true);
    expect(page.segments.length).toBe(STORE.pageBytes / STORE.segmentBytes);
  });
});
