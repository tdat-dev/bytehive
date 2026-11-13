import { DurableObject } from "cloudflare:workers";
import { FLUSH_BYTES, LIMITS } from "./limits";
import { wake } from "./push";
import * as store from "./store";

// One per account. It routes frames between the account's machines and devices
// and reads nothing inside them. Who may do what is settled end to end: a
// device that is merely logged in fails the Noise handshake with the machine.

interface Attachment { peer: string; name: string }
const peerOf = (ws: WebSocket) => (ws.deserializeAttachment() as Attachment).peer;
const isMachine = (peer: string) => peer.startsWith("m:");
/** A sealed note is a title and a line, sealed: anything longer is not one. */
const NOTE_BYTES = 4096;
/** A peer id: `m:` for a machine, `d:` for a device. */
export const PEER = /^[md]:[\w-]{1,64}$/;

export class Room extends DurableObject<Env> {
  private bytes = 0;

  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS peers (id TEXT PRIMARY KEY, name TEXT, last_seen INTEGER, revoked INTEGER DEFAULT 0);
      CREATE TABLE IF NOT EXISTS usage (day TEXT PRIMARY KEY, bytes INTEGER);
      CREATE TABLE IF NOT EXISTS subs (peer TEXT PRIMARY KEY, endpoint TEXT);
      CREATE TABLE IF NOT EXISTS notes (peer TEXT PRIMARY KEY, machine TEXT, sealed TEXT);
    `);
    store.initStore(ctx.storage.sql);
  }

  // The session store (store.ts), called by the Worker after it checked the login.
  async getVault(): Promise<store.Vault | null> { return store.getVault(this.ctx.storage.sql); }
  async setVault(v: unknown, replace: boolean) { return store.setVault(this.ctx.storage.sql, v, replace); }
  async putSession(machine: string, p: unknown) { return store.put(this.ctx.storage.sql, machine, p); }
  async dropSession(machine: string, sid: unknown) { store.drop(this.ctx.storage.sql, machine, sid); }
  async listSessions() { return store.list(this.ctx.storage.sql); }
  async sessionSegments(sid: string, from: number) { return store.segments(this.ctx.storage.sql, sid, from); }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);
    const peer = url.searchParams.get("peer") ?? "";
    const name = (url.searchParams.get("name") ?? "").slice(0, 80);
    if (req.headers.get("Upgrade") !== "websocket" || !PEER.test(peer)) return new Response("bad request", { status: 400 });

    if (this.isRevoked(peer)) return new Response("revoked", { status: 403 });

    const sockets = this.ctx.getWebSockets();
    const others = sockets.filter((w) => peerOf(w) !== peer);
    const same = (m: boolean) => new Set(others.map(peerOf).filter((p) => isMachine(p) === m)).size;
    const room = isMachine(peer) ? same(true) < LIMITS.machines : same(false) < LIMITS.devices;
    if (!room || others.length >= LIMITS.connections) return new Response("limit", { status: 429 });

    for (const old of this.ctx.getWebSockets(peer)) old.close(4000, "replaced");

    const [client, server] = Object.values(new WebSocketPair());
    this.ctx.acceptWebSocket(server, [peer]);
    server.serializeAttachment({ peer, name } satisfies Attachment);
    this.ctx.storage.sql.exec(
      "INSERT INTO peers (id, name, last_seen) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, last_seen = excluded.last_seen",
      peer, name, Date.now(),
    );
    this.announce();
    return new Response(null, { status: 101, webSocket: client });
  }

  /** Called by the Worker before it hands out a ticket. */
  isRevoked(peer: string): boolean {
    return !!this.ctx.storage.sql.exec<{ revoked: number }>("SELECT revoked FROM peers WHERE id = ?", peer).toArray()[0]?.revoked;
  }

  /** A device's browser can be woken at `endpoint`. */
  async subscribe(peer: string, endpoint: string): Promise<void> {
    this.ctx.storage.sql.exec("INSERT OR REPLACE INTO subs (peer, endpoint) VALUES (?, ?)", peer, endpoint);
  }

  /** What a device's browser reads when it wakes: only the latest, sealed by the machine. */
  async holdPush(peer: string, machine: string, sealed: string): Promise<void> {
    this.ctx.storage.sql.exec("INSERT OR REPLACE INTO notes (peer, machine, sealed) VALUES (?, ?, ?)", peer, machine, sealed);
  }

  /** The held note, once: it is gone after it is given out. */
  async takePush(peer: string): Promise<{ sealed: string; machine: string } | null> {
    const row = this.ctx.storage.sql.exec<{ sealed: string; machine: string }>("SELECT sealed, machine FROM notes WHERE peer = ?", peer).toArray()[0];
    if (!row) return null;
    this.ctx.storage.sql.exec("DELETE FROM notes WHERE peer = ?", peer);
    return { sealed: row.sealed, machine: row.machine };
  }

  webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): void {
    if (typeof message === "string") return this.control(ws, message);
    if (this.overQuota(message.byteLength)) {
      ws.send(JSON.stringify({ t: "quota", which: "bytes" }));
      return;
    }
    const b = new Uint8Array(message);
    if (b.length < 3 || b.length < 3 + b[2]) return;
    const to = new TextDecoder().decode(b.subarray(3, 3 + b[2]));
    const target = this.open(to)[0];
    if (!target) return;
    const from = new TextEncoder().encode(peerOf(ws));
    const out = new Uint8Array(3 + from.length + b.length - 3 - b[2]);
    out.set([b[0], b[1], from.length]);
    out.set(from, 3);
    out.set(b.subarray(3 + b[2]), 3 + from.length);
    target.send(out);
  }

  webSocketClose(ws: WebSocket): void {
    this.flush();
    this.ctx.storage.sql.exec("UPDATE peers SET last_seen = ? WHERE id = ?", Date.now(), peerOf(ws));
    this.announce(ws);
  }

  private control(ws: WebSocket, text: string): void {
    let m: { t?: string; peer?: string; sealed?: string };
    try { m = JSON.parse(text); } catch { return; }
    if (m.t === "revoke" && m.peer && isMachine(peerOf(ws))) {
      this.ctx.storage.sql.exec("UPDATE peers SET revoked = 1 WHERE id = ?", m.peer);
      this.ctx.storage.sql.exec("DELETE FROM subs WHERE peer = ?", m.peer);
      this.ctx.storage.sql.exec("DELETE FROM notes WHERE peer = ?", m.peer);
      if (isMachine(m.peer)) store.dropMachine(this.ctx.storage.sql, m.peer);
      for (const w of this.ctx.getWebSockets(m.peer)) w.close(4001, "revoked");
    }
    if (m.t === "push" && isMachine(peerOf(ws))) this.push(peerOf(ws), m.peer, m.sealed);
  }

  /** A machine has something to say to a device that is not here: keep it sealed, and wake the browser. */
  private push(machine: string, peer: unknown, sealed: unknown): void {
    if (typeof peer !== "string" || !PEER.test(peer) || isMachine(peer)) return;
    if (typeof sealed !== "string" || !sealed || sealed.length > NOTE_BYTES) return;
    // A device that is connected hears the machine itself; one that never asked has no browser to wake.
    if (this.open(peer).length || this.isRevoked(peer)) return;
    const sub = this.ctx.storage.sql.exec<{ endpoint: string }>("SELECT endpoint FROM subs WHERE peer = ?", peer).toArray()[0];
    if (!sub) return;
    void this.holdPush(peer, machine.slice(2), sealed);
    this.ctx.waitUntil(
      wake(sub.endpoint, this.env)
        .then((status) => {
          if (status === 404 || status === 410) this.ctx.storage.sql.exec("DELETE FROM subs WHERE peer = ?", peer);
        })
        .catch(() => {}),
    );
  }

  private announce(gone?: WebSocket): void {
    const here = this.open().filter((w) => w !== gone);
    const live = new Set(here.map(peerOf));
    const rows = this.ctx.storage.sql.exec<{ id: string; name: string; last_seen: number }>("SELECT id, name, last_seen FROM peers WHERE revoked = 0").toArray();
    const text = JSON.stringify({
      t: "peers",
      peers: rows.map((r) => ({ id: r.id, name: r.name, online: live.has(r.id), lastSeen: r.last_seen })),
    });
    for (const w of here) w.send(text);
  }

  /** Sockets that can still be written to: a replaced one stays listed until its close lands. */
  private open(tag?: string): WebSocket[] {
    return this.ctx.getWebSockets(tag).filter((w) => w.readyState === WebSocket.OPEN);
  }

  private today(): string { return new Date().toISOString().slice(0, 10); }

  private overQuota(n: number): boolean {
    this.bytes += n;
    if (this.bytes >= FLUSH_BYTES) this.flush();
    const row = this.ctx.storage.sql.exec<{ bytes: number }>("SELECT bytes FROM usage WHERE day = ?", this.today()).toArray()[0];
    return (row?.bytes ?? 0) + this.bytes > LIMITS.bytesPerDay;
  }

  private flush(): void {
    if (!this.bytes) return;
    this.ctx.storage.sql.exec(
      "INSERT INTO usage (day, bytes) VALUES (?, ?) ON CONFLICT(day) DO UPDATE SET bytes = bytes + excluded.bytes",
      this.today(), this.bytes,
    );
    this.bytes = 0;
  }
}
