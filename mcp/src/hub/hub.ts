/* The standalone browser hub: what ByteHive's own hub (crates/browser-hub) does,
 * run inside bytehive-mcp while ByteHive is closed. Same wire, so the native host
 * the extension talks through and every other bytehive-mcp dial it unchanged.
 *
 * Peers say hello with the token from the hub file. Browsers are the extension
 * (through the native host); agents are bytehive-mcp processes. Routing lives in
 * route.ts, the hub file and stepping down for the app in claim.ts.
 * See docs/superpowers/specs/2026-11-25-mcp-standalone-browser-hub.md. */

import { randomBytes } from "node:crypto";
import type { AddressInfo } from "node:net";
import { WebSocketServer, type WebSocket } from "ws";
import { onAgentMessage, onBrowserMessage } from "./route.js";

export type BrowserInfo = { id: number; browser: string; email: string; version: string };

export type Role = { kind: "unknown" } | { kind: "browser"; info: BrowserInfo } | { kind: "agent"; name: string; pick: number | null };

export type Peer = { ws: WebSocket; role: Role };

/** A call sent to a browser, waiting for its answer. */
export type Pending = { agentConn: number; cid: unknown; browser: number; agent: string; call: Record<string, unknown> };

/** A step held for the user's OK: a risky click, or going on after Stop. */
export type Held = Pending & { kind: "click" | "resume"; what: string; url: string; at: number };

/** How long a held step waits; bytehive-mcp's HOLD_TIMEOUT_MS is the same. */
export const HOLD_MS = 10 * 60_000;

export function label(b: BrowserInfo): string {
  return `${b.browser} · ${b.email || "unnamed profile"}`;
}

export const fail = (id: unknown, error: string) => JSON.stringify({ type: "error", id, error });
export const reply = (id: unknown, text: string) => JSON.stringify({ type: "result", id, result: { content: [{ type: "text", text }] } });

export class Hub {
  readonly token = randomBytes(16).toString("hex");
  port = 0;
  readonly peers = new Map<number, Peer>();
  readonly pending = new Map<number, Pending>();
  readonly held = new Map<number, Held>();
  /** Agents the user stopped with the pill on the page. */
  readonly paused = new Set<string>();
  private next = 0;
  private server: WebSocketServer | null = null;
  private startedAt = Date.now();
  private browserWaiters: Array<() => void> = [];
  stopped = false;

  /** grace: how long a call on a fresh hub waits for a browser to say hello. */
  constructor(readonly grace = 5_000) {}

  nextId(): number {
    return ++this.next;
  }

  start(): Promise<number> {
    return new Promise((resolve, reject) => {
      const wss = new WebSocketServer({ host: "127.0.0.1", port: 0, maxPayload: 256 * 1024 * 1024 });
      wss.once("error", reject);
      wss.once("listening", () => {
        this.server = wss;
        this.port = (wss.address() as AddressInfo).port;
        this.startedAt = Date.now();
        resolve(this.port);
      });
      wss.on("connection", (ws) => this.accept(ws));
    });
  }

  private accept(ws: WebSocket) {
    if (this.stopped) return ws.close();
    const id = this.nextId();
    this.peers.set(id, { ws, role: { kind: "unknown" } });
    ws.on("message", (data) => {
      if (!this.onMessage(id, data.toString())) ws.close();
    });
    ws.on("close", () => this.remove(id));
    ws.on("error", () => ws.close());
  }

  send(peer: number, text: string): boolean {
    const p = this.peers.get(peer);
    if (!p || p.ws.readyState !== p.ws.OPEN) return false;
    p.ws.send(text);
    return true;
  }

  browsers(): BrowserInfo[] {
    const l: BrowserInfo[] = [];
    for (const p of this.peers.values()) if (p.role.kind === "browser") l.push(p.role.info);
    return l.sort((a, b) => a.id - b.id);
  }

  /** On a fresh hub, wait (up to the grace) for the native host to find it. */
  async browserReady(): Promise<void> {
    const left = this.grace - (Date.now() - this.startedAt);
    if (this.browsers().length || left <= 0) return;
    await new Promise<void>((resolve) => {
      const t = setTimeout(done, left);
      function done() {
        clearTimeout(t);
        resolve();
      }
      this.browserWaiters.push(done);
    });
  }

  /** A browser is ready (it said who it is): calls waiting for one go on. */
  browserJoined() {
    for (const w of this.browserWaiters.splice(0)) w();
  }

  /** One text message from a peer. False means: close it. */
  onMessage(from: number, text: string): boolean {
    let v: Record<string, unknown>;
    try {
      v = JSON.parse(text);
    } catch {
      return true;
    }
    const peer = this.peers.get(from);
    if (!peer) return false;
    if (peer.role.kind === "unknown") {
      if (v.type !== "hello" || v.token !== this.token) return false;
      if (v.role === "browser") {
        peer.role = { kind: "browser", info: { id: from, browser: "Browser", email: "", version: "" } };
        // Its browser_info comes next and wakes waiting calls; this is in case it doesn't.
        setTimeout(() => this.browserJoined(), 300);
      } else if (v.role === "agent") {
        const name = typeof v.agent === "string" && v.agent.trim() ? v.agent.trim() : "Agent";
        peer.role = { kind: "agent", name, pick: null };
      } else return false;
      this.send(from, JSON.stringify({ type: "hello_ok" }));
      return true;
    }
    if (peer.role.kind === "browser") onBrowserMessage(this, from, peer.role.info, v);
    else void onAgentMessage(this, from, v);
    return true;
  }

  /** A peer left: fail what it was part of. */
  remove(id: number) {
    const peer = this.peers.get(id);
    if (!peer) return;
    this.peers.delete(id);
    const gone = peer.role.kind === "browser" ? `${label(peer.role.info)} disconnected before it answered.` : "";
    for (const [k, p] of this.pending) {
      if (p.browser !== id && p.agentConn !== id) continue;
      this.pending.delete(k);
      if (gone) this.send(p.agentConn, fail(p.cid, gone));
    }
    for (const [k, h] of this.held) {
      if (h.browser !== id && h.agentConn !== id) continue;
      this.held.delete(k);
      if (gone) this.send(h.agentConn, fail(h.cid, "The browser disconnected while waiting for the user."));
    }
  }

  /** Step down: tell every waiting agent why, close every peer and the server. */
  stop(why: string): Promise<void> {
    if (this.stopped) return Promise.resolve();
    this.stopped = true;
    for (const p of this.pending.values()) this.send(p.agentConn, fail(p.cid, why));
    for (const h of this.held.values()) this.send(h.agentConn, fail(h.cid, why));
    this.pending.clear();
    this.held.clear();
    for (const p of this.peers.values()) p.ws.close();
    this.peers.clear();
    return new Promise((resolve) => (this.server ? this.server.close(() => resolve()) : resolve()));
  }
}
