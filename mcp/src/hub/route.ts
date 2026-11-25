/* Routing for the standalone hub: an agent's call to its browser and back, and
 * the steps held for the user's OK. Without ByteHive's window, the agent's own
 * bytehive-mcp asks its user (MCP elicitation) and answers the hold; only the
 * connection that owns a hold can answer it. */

import { HOLD_MS, type BrowserInfo, type Hub, fail, label, reply } from "./hub.js";

const NONE_CONNECTED =
  'No browser is connected. Open Chrome (or Edge/Brave) in a profile that has the "ByteHive for Chrome" extension; ByteHive Settings → Browser shows how to install it.';
const NO_DESKTOP =
  "Chrome keeps this page from extensions, and only the ByteHive app can drive it from Windows. ByteHive is closed: ask the user to do this step, or to open ByteHive and try again.";

const listing = (bs: BrowserInfo[]) => `Connected: ${bs.map((b) => `${label(b)} (id ${b.id})`).join("; ")}.`;

type Msg = Record<string, unknown>;
const obj = (v: unknown): Msg | null => (v && typeof v === "object" && !Array.isArray(v) ? (v as Msg) : null);

export function onBrowserMessage(hub: Hub, from: number, info: BrowserInfo, v: Msg) {
  if (v.type === "browser_info") {
    info.browser = typeof v.browser === "string" ? v.browser : "Browser";
    info.email = typeof v.email === "string" ? v.email : "";
    info.version = typeof v.version === "string" ? v.version : "";
    hub.browserJoined();
    return;
  }
  if (v.type === "pause") {
    if (typeof v.agent === "string" && v.agent) hub.paused.add(v.agent);
    return;
  }
  if (v.type !== "result" && v.type !== "error") return;
  const hid = typeof v.id === "number" ? v.id : -1;
  const p = hub.pending.get(hid);
  if (!p || p.browser !== from) return;
  hub.pending.delete(hid);
  const result = obj(v.result);
  const confirm = v.type === "result" ? obj(result?.needsConfirm) : null;
  if (confirm) {
    const ask = hub.nextId();
    const what = typeof confirm.what === "string" ? confirm.what : "a click";
    const url = typeof confirm.url === "string" ? confirm.url : "";
    hub.held.set(ask, { ...p, kind: "click", what, url, at: Date.now() });
    hub.send(p.agentConn, JSON.stringify({ type: "confirm", id: p.cid, ask, what, url }));
    return;
  }
  if (v.type === "result" && obj(result?.desktop)) {
    hub.send(p.agentConn, fail(p.cid, NO_DESKTOP));
    return;
  }
  hub.send(p.agentConn, JSON.stringify({ ...v, id: p.cid }));
}

export async function onAgentMessage(hub: Hub, from: number, v: Msg) {
  if (v.type === "answer") return answer(hub, from, v);
  if (v.type !== "call") return;
  const peer = hub.peers.get(from);
  if (peer?.role.kind !== "agent") return;
  const { name } = peer.role;
  const id = v.id;
  const tool = typeof v.tool === "string" ? v.tool : "";
  const args = obj(v.args) ?? {};
  if (!hub.browsers().length) await hub.browserReady();
  const browsers = hub.browsers();
  const pick = peer.role.pick !== null && browsers.some((b) => b.id === (peer.role as { pick: number }).pick) ? peer.role.pick : null;

  if (tool === "list_browsers") {
    const text = browsers.length
      ? browsers.map((b) => `${b.id === pick ? "*" : "-"} ${label(b)} (id ${b.id})${b.email ? `, ${b.email}` : ""}`).join("\n")
      : NONE_CONNECTED;
    hub.send(from, reply(id, text));
    return;
  }
  if (tool === "select_browser") {
    const want = String(args.browser ?? "").toLowerCase();
    const hit = browsers.filter((b) => String(b.id) === want || label(b).toLowerCase().includes(want) || (b.email && b.email.toLowerCase().includes(want)));
    if (hit.length === 1) {
      peer.role.pick = hit[0].id;
      hub.send(from, reply(id, `${name} now uses ${label(hit[0])}.`));
    } else if (!hit.length) hub.send(from, fail(id, `No connected browser matches "${want}". ${listing(browsers)}`));
    else hub.send(from, fail(id, `"${want}" matches more than one browser; be more specific. ${listing(browsers)}`));
    return;
  }
  if (hub.paused.has(name)) {
    const ask = hub.nextId();
    const what = `Let ${name} use the browser again (you pressed Stop on the page)`;
    hub.held.set(ask, { agentConn: from, cid: id, browser: -1, agent: name, call: { tool, args }, kind: "resume", what, url: "", at: Date.now() });
    hub.send(from, JSON.stringify({ type: "confirm", id, ask, what, url: "" }));
    return;
  }
  dispatch(hub, from, id, name, pick, tool, args);
}

/** Send one call to the agent's browser. The hub always asks for risky clicks,
 *  and an agent never gets to say a click was confirmed. */
function dispatch(hub: Hub, from: number, cid: unknown, name: string, pick: number | null, tool: string, args: Msg) {
  const browsers = hub.browsers();
  const target = pick ?? (browsers.length === 1 ? browsers[0].id : null);
  if (target === null) {
    hub.send(from, fail(cid, browsers.length ? `More than one browser is connected; pick one with browser_select first. ${listing(browsers)}` : NONE_CONNECTED));
    return;
  }
  const clean: Msg = { ...args };
  delete clean.confirmed;
  send(hub, from, cid, name, target, { tool, args: { ...clean, _ask: true } });
}

function send(hub: Hub, from: number, cid: unknown, name: string, browser: number, call: Msg) {
  const hid = hub.nextId();
  const msg = { type: "call", id: hid, agent: name, tool: call.tool, args: call.args };
  hub.pending.set(hid, { agentConn: from, cid, browser, agent: name, call });
  if (!hub.send(browser, JSON.stringify(msg))) {
    hub.pending.delete(hid);
    hub.send(from, fail(cid, "That browser just disconnected."));
  }
}

/** The agent's process asked its user. `why: "cannot-ask"` means it had no way
 *  to ask, which counts as no. */
function answer(hub: Hub, from: number, v: Msg) {
  const ask = typeof v.ask === "number" ? v.ask : -1;
  const h = hub.held.get(ask);
  if (!h || h.agentConn !== from) return;
  hub.held.delete(ask);
  if (Date.now() - h.at >= HOLD_MS) return; // the agent's call already gave up
  if (v.ok === true) {
    if (h.kind === "resume") {
      hub.paused.delete(h.agent);
      const peer = hub.peers.get(from);
      const pick = peer?.role.kind === "agent" ? peer.role.pick : null;
      return dispatch(hub, from, h.cid, h.agent, pick, String(h.call.tool), obj(h.call.args) ?? {});
    }
    return send(hub, from, h.cid, h.agent, h.browser, { tool: h.call.tool, args: { ...(obj(h.call.args) ?? {}), confirmed: true } });
  }
  const text =
    h.kind === "resume"
      ? `The user stopped ${h.agent}'s browser use and did not let it go on. Don't use the browser; tell the user where you are and wait until they say to go on.`
      : v.why === "cannot-ask"
        ? `ByteHive is closed and this app could not ask the user, so this was not done: ${h.what}. Ask the user to do it themselves, or to open ByteHive and try again.`
        : `The user said no to: ${h.what}. Don't do it; tell them what you were about to do and why.`;
  hub.send(from, fail(h.cid, text));
}
