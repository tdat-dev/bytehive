// Notifications on this device. The browser is woken by an empty push; what to
// say comes sealed from the computer and is opened by the service worker (sw.ts).

import { loadToken } from "./keys";
import { el, mark, say } from "./views/ui";

const RELAY = import.meta.env.VITE_RELAY as string;

/** `unsupported`: this browser can't, so nothing is offered. `blocked`: the person said no in the browser's own settings. */
export type PushState = "unsupported" | "blocked" | "off" | "on";

const supported = () => "serviceWorker" in navigator && "PushManager" in window && "Notification" in window;

const bytes = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));
const same = (a: ArrayBuffer | null, b: Uint8Array) => !!a && new Uint8Array(a).length === b.length && new Uint8Array(a).every((v, i) => v === b[i]);

export async function pushState(): Promise<PushState> {
  if (!supported()) return "unsupported";
  if (Notification.permission === "denied") return "blocked";
  const sub = await (await navigator.serviceWorker.getRegistration("/"))?.pushManager.getSubscription();
  return sub && Notification.permission === "granted" ? "on" : "off";
}

/** Registers the worker, subscribes this browser, and tells the relay where to wake it. Asks for no permission. */
async function subscribe(): Promise<void> {
  const token = await loadToken();
  if (!token) throw new Error("not signed in");
  await navigator.serviceWorker.register("/sw.js", { type: "module", scope: "/" });
  const reg = await navigator.serviceWorker.ready;
  const asked = await fetch(`${RELAY}/push/key`);
  if (!asked.ok) throw new Error("the relay has no push key");
  const key = bytes(((await asked.json()) as { key: string }).key);
  let sub = await reg.pushManager.getSubscription();
  // A subscription made under another key can't be reused.
  if (sub && !same(sub.options.applicationServerKey, key)) {
    await sub.unsubscribe();
    sub = null;
  }
  sub ??= await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key as BufferSource });
  const sent = await fetch(`${RELAY}/push/subscribe`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ endpoint: sub.endpoint }),
  });
  if (!sent.ok) throw new Error(`the relay said ${sent.status}`);
}

/** Called from the person's click: the permission prompt must answer one. */
export async function enablePush(): Promise<PushState> {
  if (!supported()) return "unsupported";
  if ((await Notification.requestPermission()) === "granted") await subscribe();
  return pushState();
}

/** On each visit, once allowed: a newer worker, and the relay told again where this browser is now. Quiet when it can't. */
export async function restorePush(): Promise<void> {
  if (!supported() || Notification.permission !== "granted") return;
  await subscribe().catch(() => {});
}

let last: PushState | null = null;

/** The control for the home page: a button to turn notifications on, or a word on how they stand. */
export function notifyControl(): HTMLElement {
  const box = el("div", undefined, "notify");
  const paint = (state: PushState, failed = false) => {
    last = state;
    if (state === "unsupported") return box.replaceChildren();
    if (state === "on" || state === "blocked") {
      const line = el("p", undefined, "machine-state");
      line.append(mark(state === "on" ? "up" : "stopped"), el("span", state === "on" ? "Notifications on" : "Notifications are blocked in this browser"));
      return box.replaceChildren(line);
    }
    const b = el("button", failed ? "Try notifications again" : "Turn on notifications", "btn quiet");
    b.id = "notify";
    b.type = "button";
    b.onclick = async () => {
      b.disabled = true;
      b.textContent = "Turning on";
      try {
        const now = await enablePush();
        paint(now);
        say(now === "on" ? "Notifications are on." : "Notifications were not turned on.");
      } catch {
        paint("off", true);
        say("Could not turn on notifications.");
      }
    };
    box.replaceChildren(b);
  };
  // What is known now is drawn at once, so a redraw does not blink the button away.
  if (last) paint(last);
  void pushState().then(paint, () => {});
  return box;
}
