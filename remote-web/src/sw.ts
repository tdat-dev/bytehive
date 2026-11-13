// Wakes on an empty push, fetches the note the machine left sealed with the
// relay, opens it with the vault this browser keeps, and shows it. Whatever
// goes wrong, something is shown: a push that shows nothing is a rule broken.
// No lib "webworker" here: this project's types are the page's, so the few
// parts of a service worker used below are named by hand.

import init, { open } from "../../remote/noise-wasm/pkg/bytehive_noise_wasm.js";
import type { PendingNote } from "../../src/remote/protocol";
import { loadToken, machines, storedDeviceId } from "./keys";

interface Waiting { waitUntil(work: Promise<unknown>): void }
interface Scope {
  registration: { showNotification(title: string, options: { body: string; icon: string }): Promise<void> };
  clients: {
    matchAll(options: { type: "window" }): Promise<{ focus(): Promise<unknown> }[]>;
    openWindow(url: string): Promise<unknown>;
  };
  addEventListener(type: "push", listener: (e: Waiting) => void): void;
  addEventListener(type: "notificationclick", listener: (e: Waiting & { notification: { close(): void } }) => void): void;
}
const sw = self as unknown as Scope;
const RELAY = import.meta.env.VITE_RELAY as string;

const PLAIN = { title: "ByteHive", body: "There's an update on your agents." };

async function note(): Promise<{ title: string; body: string }> {
  try {
    const token = await loadToken();
    const device = await storedDeviceId();
    if (!token || !device) return PLAIN;
    const r = await fetch(`${RELAY}/push/pending`, { method: "POST", headers: { Authorization: `Bearer ${token}` } });
    const held = (await r.json()) as PendingNote | null;
    if (!held) return PLAIN;
    const vault = (await machines()).find((m) => m.id === held.machine)?.vault;
    if (!vault) return PLAIN;
    await init();
    // The machine bound the note to this device: one swapped in from another does not open.
    const aad = new TextEncoder().encode(`note:d:${device}`);
    const sealed = Uint8Array.from(atob(held.sealed), (c) => c.charCodeAt(0));
    const said = JSON.parse(new TextDecoder().decode(open(vault, aad, sealed))) as { title?: unknown; body?: unknown };
    return typeof said.title === "string" && typeof said.body === "string" ? { title: said.title, body: said.body } : PLAIN;
  } catch {
    return PLAIN;
  }
}

sw.addEventListener("push", (e) => {
  e.waitUntil(note().then((n) => sw.registration.showNotification(n.title, { body: n.body, icon: "/icons/icon-192.png" })));
});

sw.addEventListener("notificationclick", (e) => {
  e.notification.close();
  // The app already open is the one to show: a second tab would take the connection from it.
  e.waitUntil(sw.clients.matchAll({ type: "window" }).then((open) => (open[0] ? open[0].focus() : sw.clients.openWindow("/"))));
});
