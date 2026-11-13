import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enablePush, notifyControl, pushState, restorePush } from "./push";

vi.mock("./keys", () => ({ loadToken: async () => "TOKEN" }));

// 65 bytes, as a real key: base64url of 0..64
const KEY = btoa(String.fromCharCode(...Array.from({ length: 65 }, (_, i) => i))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

interface World { permission: NotificationPermission; subscription: { endpoint: string; options: { applicationServerKey: ArrayBuffer | null }; unsubscribe(): Promise<boolean> } | null }

function browser(w: World) {
  const subscribe = vi.fn(async (o: { applicationServerKey: Uint8Array }) => {
    w.subscription = { endpoint: "https://fcm.googleapis.com/fcm/send/x", options: { applicationServerKey: o.applicationServerKey.slice().buffer }, unsubscribe: async () => true };
    return w.subscription;
  });
  const reg = { pushManager: { getSubscription: async () => w.subscription, subscribe } };
  const register = vi.fn(async () => reg);
  vi.stubGlobal("PushManager", class {});
  const notification = function () {};
  Object.defineProperty(notification, "permission", { get: () => w.permission });
  Object.defineProperty(notification, "requestPermission", {
    value: vi.fn(async () => { w.permission = w.permission === "default" ? "granted" : w.permission; return w.permission; }),
  });
  vi.stubGlobal("Notification", notification);
  Object.defineProperty(navigator, "serviceWorker", {
    configurable: true,
    value: { register, ready: Promise.resolve(reg), getRegistration: async () => reg },
  });
  const posts: { url: string; auth: string | null; body: unknown }[] = [];
  vi.stubGlobal("fetch", async (url: string, init?: RequestInit) => {
    if (url.endsWith("/push/key")) return Response.json({ key: KEY });
    posts.push({ url, auth: new Headers(init?.headers).get("Authorization"), body: JSON.parse(String(init?.body)) });
    return new Response("{}", { status: 200 });
  });
  return { register, subscribe, posts };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

afterEach(() => {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "serviceWorker");
});
beforeEach(() => { document.body.replaceChildren(); });

describe("notifications on this device", () => {
  it("offers nothing where the browser can't", async () => {
    expect(await pushState()).toBe("unsupported");
    const box = notifyControl();
    await flush();
    expect(box.children).toHaveLength(0);
  });

  it("turns on from the person's click: asks, subscribes under the relay's key, tells the relay where", async () => {
    const w: World = { permission: "default", subscription: null };
    const b = browser(w);
    const box = notifyControl();
    await flush();
    const button = box.querySelector("button")!;
    expect(button.textContent).toBe("Turn on notifications");
    button.click();
    await vi.waitFor(() => expect(box.textContent).toBe("Notifications on"));
    expect(b.register).toHaveBeenCalledWith("/sw.js", { type: "module", scope: "/" });
    expect([...(b.subscribe.mock.calls[0][0].applicationServerKey)]).toEqual(Array.from({ length: 65 }, (_, i) => i));
    expect(b.posts).toEqual([{ url: expect.stringMatching(/\/push\/subscribe$/), auth: "Bearer TOKEN", body: { endpoint: "https://fcm.googleapis.com/fcm/send/x" } }]);
  });

  it("subscribes nothing when the person says no", async () => {
    const w: World = { permission: "default", subscription: null };
    const b = browser(w);
    (Notification.requestPermission as ReturnType<typeof vi.fn>).mockImplementation(async () => { w.permission = "denied"; return "denied"; });
    expect(await enablePush()).toBe("blocked");
    expect(b.subscribe).not.toHaveBeenCalled();
    expect(b.posts).toEqual([]);
  });

  it("says so when the browser has it blocked, with no button", async () => {
    browser({ permission: "denied", subscription: null });
    const box = notifyControl();
    await flush();
    expect(box.textContent).toBe("Notifications are blocked in this browser");
    expect(box.querySelector("button")).toBeNull();
  });

  it("lets the person try again when it did not work", async () => {
    const w: World = { permission: "default", subscription: null };
    browser(w);
    vi.stubGlobal("fetch", async () => new Response("no", { status: 404 }));
    const box = notifyControl();
    await flush();
    box.querySelector("button")!.click();
    await vi.waitFor(() => expect(box.querySelector("button")?.textContent).toBe("Try notifications again"));
  });

  it("keeps a subscription it already has, and renews one made under another key", async () => {
    const same = new Uint8Array(Array.from({ length: 65 }, (_, i) => i)).buffer;
    const w: World = { permission: "granted", subscription: { endpoint: "https://fcm.googleapis.com/fcm/send/old", options: { applicationServerKey: same }, unsubscribe: async () => true } };
    const b = browser(w);
    await restorePush();
    expect(b.subscribe).not.toHaveBeenCalled();
    expect(b.posts[0].body).toEqual({ endpoint: "https://fcm.googleapis.com/fcm/send/old" });
    const unsub = vi.fn(async () => true);
    w.subscription = { endpoint: "https://fcm.googleapis.com/fcm/send/older", options: { applicationServerKey: new Uint8Array([1, 2]).buffer }, unsubscribe: unsub };
    await restorePush();
    expect(unsub).toHaveBeenCalled();
    expect(b.subscribe).toHaveBeenCalledTimes(1);
  });

  it("does not wake the browser's prompt by itself when it was never allowed", async () => {
    const b = browser({ permission: "default", subscription: null });
    await restorePush();
    expect(b.register).not.toHaveBeenCalled();
    expect(Notification.requestPermission).not.toHaveBeenCalled();
  });
});
