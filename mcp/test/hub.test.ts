// @vitest-environment node
/* The standalone hub over real local sockets: a fake extension (role browser)
 * and agents, both the real HubClient and raw sockets. */
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { WebSocket } from "ws";
import { HubClient, type AskUser } from "../src/browser.js";
import { Hub } from "../src/hub/hub.js";
import { TOOK_OVER, locateHub, readHubFile, startOwn, takeLock } from "../src/hub/claim.js";

type Call = { type: string; id: number; agent: string; tool: string; args: Record<string, unknown> };
const text = (r: { content: Array<{ type: string; text?: string }> }) => r.content.map((c) => c.text ?? "").join(" ");

const hubs: Hub[] = [];
afterEach(async () => {
  for (const h of hubs.splice(0)) await h.stop("test over");
});

async function newHub(grace = 300) {
  const h = new Hub(grace);
  await h.start();
  hubs.push(h);
  return h;
}

/** A socket that said hello; resolves once the hub answered hello_ok. */
function peer(h: Hub, role: "browser" | "agent", agent = "Raw"): Promise<{ ws: WebSocket; got: Record<string, unknown>[] }> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${h.port}/`);
    const got: Record<string, unknown>[] = [];
    ws.on("message", (d) => {
      const m = JSON.parse(d.toString());
      if (m.type === "hello_ok") return resolve({ ws, got });
      got.push(m);
    });
    ws.on("error", reject);
    ws.on("open", () => {
      ws.send(JSON.stringify({ type: "hello", role, token: h.token, agent }));
      if (role === "browser") ws.send(JSON.stringify({ type: "browser_info", browser: "Google Chrome", email: "me@x.test", version: "1.0" }));
    });
  });
}

/** A browser that answers each call with answer(call). */
async function browser(h: Hub, answer: (c: Call) => Record<string, unknown>) {
  const b = await peer(h, "browser");
  const calls: Call[] = [];
  b.ws.on("message", (d) => {
    const m = JSON.parse(d.toString());
    if (m.type !== "call") return;
    calls.push(m);
    b.ws.send(JSON.stringify({ type: "result", id: m.id, result: answer(m) }));
  });
  return calls;
}

const agent = (h: Hub, ask?: AskUser) => {
  const c = new HubClient("Ana", async () => ({ port: h.port, token: h.token }));
  if (ask) c.setAsk(ask);
  return c;
};
const until = async (f: () => boolean, ms = 3000) => {
  const end = Date.now() + ms;
  while (!f() && Date.now() < end) await new Promise((r) => setTimeout(r, 20));
};

describe("standalone hub routing", () => {
  it("sends a call to the browser, always asking for risky clicks, and brings the answer back", async () => {
    const h = await newHub();
    const calls = await browser(h, () => ({ content: [{ type: "text", text: "went there" }] }));
    const r = await agent(h).call("navigate", { url: "https://example.com", confirmed: true, _ask: false });
    expect(text(r)).toBe("went there");
    expect(calls[0]).toMatchObject({ agent: "Ana", tool: "navigate", args: { url: "https://example.com", _ask: true } });
    expect(calls[0].args.confirmed).toBeUndefined(); // an agent can't say it was confirmed
  });

  it("asks the agent's user about a risky click and clicks only after a yes", async () => {
    const h = await newHub();
    const calls = await browser(h, (c) =>
      c.args.confirmed ? { content: [{ type: "text", text: "clicked" }] } : { content: [{ type: "text", text: "waiting" }], needsConfirm: { what: 'Click "Send"', url: "https://mail.test" } },
    );
    const asked: string[] = [];
    const r = await agent(h, async (what, url) => (asked.push(`${what} @ ${url}`), true)).call("computer", { action: "left_click", ref: "r1" });
    expect(asked).toEqual(['Click "Send" @ https://mail.test']);
    expect(text(r)).toBe("clicked");
    expect(calls.map((c) => c.args.confirmed)).toEqual([undefined, true]);
  });

  it("does not click when the user says no, or when nobody could be asked", async () => {
    const h = await newHub();
    const calls = await browser(h, () => ({ content: [], needsConfirm: { what: 'Click "Pay"', url: "" } }));
    const no = await agent(h, async () => false).call("computer", { action: "left_click", ref: "r1" });
    expect(no.isError).toBe(true);
    expect(text(no)).toMatch(/said no to: Click "Pay"/);
    const cannot = await agent(h).call("computer", { action: "left_click", ref: "r1" });
    expect(text(cannot)).toMatch(/could not ask the user/);
    expect(calls.every((c) => !c.args.confirmed)).toBe(true);
  });

  it("takes the answer to a hold only from the agent that owns it", async () => {
    const h = await newHub();
    const calls = await browser(h, () => ({ content: [], needsConfirm: { what: 'Click "Delete"', url: "" } }));
    const a = await peer(h, "agent", "A");
    const b = await peer(h, "agent", "B");
    a.ws.send(JSON.stringify({ type: "call", id: 7, tool: "computer", args: { action: "left_click" } }));
    await until(() => a.got.length > 0);
    const confirm = a.got[0];
    expect(confirm).toMatchObject({ type: "confirm", id: 7, what: 'Click "Delete"' });
    b.ws.send(JSON.stringify({ type: "answer", ask: confirm.ask, ok: true }));
    await new Promise((r) => setTimeout(r, 150));
    expect(calls).toHaveLength(1); // B's yes did nothing
    a.ws.send(JSON.stringify({ type: "answer", ask: confirm.ask, ok: false }));
    await until(() => a.got.length > 1);
    expect(a.got[1]).toMatchObject({ type: "error", id: 7 });
  });

  it("asks before letting an agent the user stopped go on", async () => {
    const h = await newHub();
    const b = await peer(h, "browser");
    b.ws.send(JSON.stringify({ type: "pause", agent: "Ana" }));
    await until(() => h.paused.has("Ana"));
    const r = await agent(h, async () => false).call("navigate", { url: "x" });
    expect(text(r)).toMatch(/stopped Ana's browser use/);
    expect(h.paused.has("Ana")).toBe(true);
  });

  it("lets a call made before the browser arrived see who the browser is", async () => {
    const h = await newHub(3000);
    const listing = agent(h).call("list_browsers", {});
    await new Promise((r) => setTimeout(r, 100));
    await peer(h, "browser");
    expect(text(await listing)).toMatch(/Google Chrome · me@x\.test/);
  });

  it("says no browser is connected once the grace is over", async () => {
    const h = await newHub(100);
    const r = await agent(h).call("tabs", {});
    expect(text(r)).toMatch(/No browser is connected/);
  });
});

describe("claiming the hub file", () => {
  const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), "hub-claim-"));

  it("starts a hub when the file names nobody, and refuses when this agent may not", async () => {
    const dir = tmp();
    const file = path.join(dir, "browser-hub.json");
    fs.writeFileSync(file, JSON.stringify({ port: 1, token: "dead" }));
    await expect(locateHub(file, { mayStart: false, file })).rejects.toThrow(/closed/);
    const addr = await locateHub(file, { mayStart: true, file, hooks: false });
    expect(readHubFile(file)).toEqual(addr);
    expect(addr.port).not.toBe(1);
    const again = await locateHub(file, { mayStart: true, file, hooks: false });
    expect(again).toEqual(addr); // the second agent dials it, no second hub
  });

  it("steps down when ByteHive publishes its own hub", async () => {
    const file = path.join(tmp(), "browser-hub.json");
    const hub = await startOwn(file, { hooks: false, grace: 100 });
    hubs.push(hub);
    const app = net.createServer().listen(0, "127.0.0.1");
    await new Promise((r) => app.once("listening", r));
    const port = (app.address() as net.AddressInfo).port;
    const a = await peer(hub, "agent", "A");
    await browser(hub, () => ({ content: [], needsConfirm: { what: "Click \"Post\"", url: "" } }));
    a.ws.send(JSON.stringify({ type: "call", id: 3, tool: "computer", args: {} }));
    await until(() => a.got.length > 0);
    fs.writeFileSync(file, JSON.stringify({ port, token: "app", pid: 1 }));
    await until(() => hub.stopped, 5000);
    expect(hub.stopped).toBe(true);
    await until(() => a.got.length > 1);
    expect(a.got[1]).toMatchObject({ type: "error", id: 3, error: TOOK_OVER });
    expect(readHubFile(file)?.port).toBe(port); // the app's file is left alone
    app.close();
  }, 10_000);

  it("clears a start lock left by a process that is gone", () => {
    const lock = path.join(tmp(), "browser-hub.lock");
    fs.writeFileSync(lock, "999999");
    expect(takeLock(lock)).toBe(true);
    expect(takeLock(lock)).toBe(false); // ours now, and we are alive
  });
});
