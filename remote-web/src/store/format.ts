// What a machine seals into the store (native/src/cloud/items.rs writes the same).

import type { ChatItem } from "../../../src/chatmodel";

/** A session's header, sealed in slot `<sid>/meta`. */
export interface Meta {
  title: string;
  cli: string;
  project: string;
  machine: string;
  agent: string;
  started: number;
  updated: number;
  items: number;
}

/** One item as sealed; a segment (slot `<sid>/<n>`) is an array of up to 200. */
export type Wire =
  | { kind: "user"; id: string; text: string }
  | { kind: "text"; id: string; text: string }
  | { kind: "step"; id: string; verb: string; target: string; code?: boolean; error?: boolean; done?: boolean };

/** A stored item as the chat log draws it; anything not understood is left out. */
export function toChat(w: Wire): ChatItem | null {
  if (!w || typeof w !== "object" || typeof w.id !== "string") return null;
  if ((w.kind === "user" || w.kind === "text") && typeof w.text === "string") {
    return w.kind === "user" ? { kind: "user", id: w.id, text: w.text, images: 0, at: 0 } : { kind: "text", id: w.id, text: w.text, at: 0 };
  }
  if (w.kind === "step" && typeof w.verb === "string" && typeof w.target === "string") {
    return { kind: "step", id: w.id, tool: "", verb: w.verb, target: w.target, code: !!w.code, error: !!w.error, done: w.done !== false, at: 0 };
  }
  return null;
}
