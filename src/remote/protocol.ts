// What ByteHive and the web page say to each other through the relay. A frame
// is what the relay routes; what is inside a Data frame only the two ends read.
// Used by the app and by remote-web; src-tauri/src/remote/frame.rs mirrors the
// bytes, the test vector in protocol.test.ts keeps them equal.

import type { ChatItem } from "../chatmodel";

export const PROTOCOL = 1;
const FRAME_VERSION = 1;

export const FrameKind = { HandshakePair: 1, Handshake: 2, Data: 3 } as const;
export type FrameKindValue = (typeof FrameKind)[keyof typeof FrameKind];

export interface Frame { kind: number; peer: string; payload: Uint8Array }

/** [version][kind][peer length][peer][payload]. `peer` is the receiver on the
 *  way in; the relay swaps it for the sender on the way out. */
export function encodeFrame(f: Frame): Uint8Array {
  const peer = new TextEncoder().encode(f.peer);
  const out = new Uint8Array(3 + peer.length + f.payload.length);
  out.set([FRAME_VERSION, f.kind, peer.length]);
  out.set(peer, 3);
  out.set(f.payload, 3 + peer.length);
  return out;
}

export function decodeFrame(b: Uint8Array): Frame | null {
  if (b.length < 3 || b[0] !== FRAME_VERSION) return null;
  const end = 3 + b[2];
  if (b.length < end) return null;
  return { kind: b[1], peer: new TextDecoder().decode(b.subarray(3, end)), payload: b.slice(end) };
}

export const Inner = { Json: 0, JsonPart: 1, TermData: 2, TermInput: 3, TermReset: 4 } as const;

/** Bytes of JSON per Noise message; a Noise message stops at 65535. */
export const MAX_PART = 60_000;

export function splitJson(text: string): Uint8Array[] {
  const bytes = new TextEncoder().encode(text);
  const parts: Uint8Array[] = [];
  for (let at = 0; at === 0 || at < bytes.length; at += MAX_PART) {
    const piece = bytes.subarray(at, at + MAX_PART);
    const last = at + MAX_PART >= bytes.length;
    const m = new Uint8Array(piece.length + 1);
    m[0] = last ? Inner.Json : Inner.JsonPart;
    m.set(piece, 1);
    parts.push(m);
  }
  return parts;
}

/** Puts split JSON back together. `add` answers the text once it is whole. */
export class Joiner {
  private held: Uint8Array[] = [];
  add(m: Uint8Array): string | null {
    this.held.push(m.subarray(1));
    if (m[0] === Inner.JsonPart) return null;
    const size = this.held.reduce((n, p) => n + p.length, 0);
    const all = new Uint8Array(size);
    let at = 0;
    for (const p of this.held) { all.set(p, at); at += p.length; }
    this.held = [];
    return new TextDecoder().decode(all);
  }
}

export function encodeTerm(kind: number, agent: string, bytes: Uint8Array): Uint8Array {
  const id = new TextEncoder().encode(agent);
  const out = new Uint8Array(2 + id.length + bytes.length);
  out.set([kind, id.length]);
  out.set(id, 2);
  out.set(bytes, 2 + id.length);
  return out;
}

export type DecodedInner =
  | { kind: typeof Inner.Json | typeof Inner.JsonPart; raw: Uint8Array }
  | { kind: typeof Inner.TermData | typeof Inner.TermInput | typeof Inner.TermReset; agent: string; bytes: Uint8Array };

export function decodeInner(m: Uint8Array): DecodedInner {
  const kind = m[0];
  if (kind === Inner.Json || kind === Inner.JsonPart) return { kind, raw: m };
  const end = 2 + m[1];
  return {
    kind: kind as typeof Inner.TermData,
    agent: new TextDecoder().decode(m.subarray(2, end)),
    bytes: m.slice(end),
  };
}

export interface RemoteProject { id: string; name: string }
export interface RemoteAgent {
  id: string;
  name: string;
  project: string;
  projectId: string;
  /** "needs", "working", "idle", "done" or "stopped", as `remoteStatus` (wire-rules.ts) makes it from the app's own rows. */
  status: string;
  title: string | null;
  /** Chat is there for CLIs with a transcript; the rest are Terminal only. */
  hasChat: boolean;
  cols: number;
  rows: number;
}
export interface PromptOption { label: string; keys: string }

export type ToMachine =
  | { type: "fleet.get" }
  | { type: "chat.page"; agent: string; before: string | null; limit: number; whole?: boolean }
  | { type: "chat.close"; agent: string }
  | { type: "chat.send"; agent: string; id: string; text: string }
  | { type: "term.open"; agent: string }
  | { type: "term.close"; agent: string }
  | { type: "term.resize"; agent: string; cols: number; rows: number }
  | { type: "agent.stop"; agent: string }
  | { type: "agent.new"; project: string; preset: string; task: string }
  /** `id` is the `id` of the question the person answered: the app types nothing if the screen has moved on. */
  | { type: "prompt.answer"; agent: string; id: string; keys: string }
  | { type: "pair.invite" };

export type ToWeb =
  | { type: "hello"; protocol: number; machine: string }
  | { type: "fleet.state"; agents: RemoteAgent[]; projects: RemoteProject[]; presets: { id: string; label: string }[] }
  /** `live`: pushed as the conversation grew, not asked for. `reset`: the page throws away what it holds and starts from these (a whole load, a new conversation). `older`: items exist before the first of these. */
  | { type: "chat.items"; agent: string; items: ChatItem[]; older: boolean; partial: boolean; live: boolean; reset: boolean }
  | { type: "chat.ack"; id: string; state: "typed" | "seen" | "failed" }
  | { type: "prompt.ask"; agent: string; id: string; screen: string; options: PromptOption[] }
  /** The agent is no longer asking: the page drops its buttons. */
  | { type: "prompt.clear"; agent: string }
  | { type: "pair.code"; code: string; expires: number }
  | { type: "vault.set"; key: string }
  | { type: "error"; code: string; message: string };

/** What the relay hands a device's browser when it wakes: the machine's sealed note and which machine sealed it. */
export interface PendingNote { sealed: string; machine: string }

export interface Peer { id: string; name: string; online: boolean; lastSeen: number }
/** Text messages between a peer and the relay. Nothing private goes in here. */
export type RelayControl =
  | { t: "peers"; peers: Peer[] }
  | { t: "quota"; which: "connections" | "machines" | "devices" | "bytes" }
  | { t: "revoke"; peer: string }
  | { t: "push"; peer: string; sealed: string };
