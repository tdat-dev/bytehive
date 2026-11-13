// The web side of a machine: the relay socket, and one Noise session on top of it.

import {
  FrameKind, Inner, Joiner, PROTOCOL, decodeFrame, decodeInner, encodeFrame, encodeTerm, splitJson,
  type Frame, type Peer, type RelayControl, type ToMachine, type ToWeb,
} from "../../src/remote/protocol";

export interface NoiseSession {
  write(payload: Uint8Array): Uint8Array;
  read(msg: Uint8Array): Uint8Array;
  encrypt(p: Uint8Array): Uint8Array;
  decrypt(c: Uint8Array): Uint8Array;
  readonly ready: boolean;
  readonly remoteKey: Uint8Array | undefined;
}

export type CloseReason = "wrong-key" | "update" | "broken";

const same = (a: Uint8Array, b: Uint8Array) => a.length === b.length && a.every((x, i) => x === b[i]);

export class MachineSession {
  open = false;
  machine: { id: string; name: string } | null = null;
  onOpen: () => void = () => {};
  onMessage: (m: ToWeb) => void = () => {};
  onTerm: (agent: string, bytes: Uint8Array, reset: boolean) => void = () => {};
  onClosed: (why: CloseReason) => void = () => {};
  private join = new Joiner();
  private kind: number;

  constructor(private o: {
    peer: string;
    noise: NoiseSession;
    pairing: boolean;
    /** The key this machine must have. Unset while pairing: the code proves it. */
    expectKey?: Uint8Array;
    deviceName?: string;
    sendFrame(b: Uint8Array): void;
  }) {
    this.kind = o.pairing ? FrameKind.HandshakePair : FrameKind.Handshake;
  }

  get remoteKey(): Uint8Array | undefined { return this.o.noise.remoteKey; }

  start(): void {
    this.out(this.kind, this.o.noise.write(new Uint8Array()));
  }

  private out(kind: number, payload: Uint8Array): void {
    this.o.sendFrame(encodeFrame({ kind, peer: this.o.peer, payload }));
  }

  private close(why: CloseReason): void {
    this.open = false;
    this.onClosed(why);
  }

  frame(f: Frame): void {
    try {
      if (f.kind === this.kind && !this.open) return this.finish(f.payload);
      if (f.kind === FrameKind.Data && this.open) return this.data(this.o.noise.decrypt(f.payload));
    } catch {
      this.close("broken");
    }
  }

  private finish(reply: Uint8Array): void {
    const hello = JSON.parse(new TextDecoder().decode(this.o.noise.read(reply))) as { machine: string; name: string };
    // Before message 3: that one carries this device's identity.
    const key = this.o.noise.remoteKey;
    if (this.o.expectKey && (!key || !same(key, this.o.expectKey))) return this.close("wrong-key");
    // The id is claimed inside the handshake, so a machine with a code of its
    // own could name another machine's id and have its key pinned in its place.
    if (hello.machine !== this.o.peer.slice(2)) return this.close("wrong-key");
    const me = new TextEncoder().encode(JSON.stringify({ name: this.o.deviceName ?? "Device" }));
    this.out(this.kind, this.o.noise.write(me));
    this.machine = { id: hello.machine, name: hello.name };
    this.open = true;
    this.onOpen();
  }

  private data(m: Uint8Array): void {
    const d = decodeInner(m);
    if (d.kind === Inner.Json || d.kind === Inner.JsonPart) {
      const text = this.join.add(m);
      if (text === null) return;
      const msg = JSON.parse(text) as ToWeb;
      if (msg.type === "hello" && msg.protocol !== PROTOCOL) return this.close("update");
      this.onMessage(msg);
    } else if (d.kind === Inner.TermData || d.kind === Inner.TermReset) {
      this.onTerm(d.agent, d.bytes, d.kind === Inner.TermReset);
    }
  }

  private seal(inner: Uint8Array): void {
    this.out(FrameKind.Data, this.o.noise.encrypt(inner));
  }

  send(m: ToMachine): void {
    for (const p of splitJson(JSON.stringify(m))) this.seal(p);
  }

  sendKeys(agent: string, bytes: Uint8Array): void {
    this.seal(encodeTerm(Inner.TermInput, agent, bytes));
  }
}

const RELAY = import.meta.env.VITE_RELAY as string;
const RELAY_WS = import.meta.env.VITE_RELAY_WS as string;
const WAITS = [1000, 2000, 4000, 8000, 15000, 30000];

export class Relay {
  peers: Peer[] = [];
  onPeers: (p: Peer[]) => void = () => {};
  onFrame: (f: Frame) => void = () => {};
  onUp: () => void = () => {};
  onDown: () => void = () => {};
  onAuthLost: () => void = () => {};
  /** The machine removed this device: nothing here will ever be let in again. */
  onRevoked: () => void = () => {};
  onQuota: (which: string) => void = () => {};
  /** Another tab of this browser took the connection (one device id, one socket). */
  onReplaced: () => void = () => {};
  private ws: WebSocket | null = null;
  private tries = 0;
  private stopped = false;

  constructor(private o: { token: string; deviceName: string }) {}

  async connect(): Promise<void> {
    const r = await fetch(`${RELAY}/ticket`, {
      method: "POST",
      headers: { Authorization: `Bearer ${this.o.token}`, "Content-Type": "application/json" },
      body: JSON.stringify({ name: this.o.deviceName }),
    }).catch(() => null);
    if (r?.status === 401) { this.stopped = true; return this.onAuthLost(); }
    if (r?.status === 403) { this.stopped = true; return this.onRevoked(); }
    if (!r?.ok) return this.retry();
    const { ticket } = (await r.json()) as { ticket: string };
    const ws = new WebSocket(`${RELAY_WS}/ws?ticket=${encodeURIComponent(ticket)}`);
    ws.binaryType = "arraybuffer";
    ws.onopen = () => { this.tries = 0; this.ws = ws; this.onUp(); };
    ws.onmessage = (e) => this.message(e.data);
    ws.onclose = (e) => {
      this.ws = null;
      // 4001 is the relay saying this device was revoked while it was connected.
      if (e.code === 4001) { this.stopped = true; this.onRevoked(); }
      // 4000 is another tab taking the socket. Retrying would take it back, and
      // the two tabs would swap it about once a second: wait for a click instead.
      if (e.code === 4000) { this.stopped = true; this.onReplaced(); }
      this.onDown();
      if (!this.stopped) this.retry();
    };
  }

  private retry(): void {
    const wait = WAITS[Math.min(this.tries++, WAITS.length - 1)];
    window.setTimeout(() => void this.connect(), wait);
  }

  private message(data: string | ArrayBuffer): void {
    if (typeof data === "string") {
      const m = JSON.parse(data) as RelayControl;
      if (m.t === "peers") { this.peers = m.peers; this.onPeers(m.peers); }
      if (m.t === "quota") this.onQuota(m.which);
      return;
    }
    const f = decodeFrame(new Uint8Array(data));
    if (f) this.onFrame(f);
  }

  /** After `onReplaced`: take the connection back from the other tab. */
  resume(): void {
    this.stopped = false;
    this.tries = 0;
    void this.connect();
  }

  send(b: Uint8Array): void { this.ws?.send(b as BufferSource); }
  stop(): void { this.stopped = true; this.ws?.close(); }
}
