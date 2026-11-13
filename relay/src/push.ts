// Wakes a device's browser with an empty push. What it should say waits sealed
// in the Room, so the push service carries nothing and the relay reads nothing.

const b64 = (b: ArrayBuffer | Uint8Array) => {
  const u = b instanceof Uint8Array ? b : new Uint8Array(b);
  return btoa(String.fromCharCode(...u)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
};
const text = (s: string) => b64(new TextEncoder().encode(s));

/** The push services of the browsers: an endpoint is an address the relay will send to, so only these. */
const SERVICES = ["fcm.googleapis.com", "push.services.mozilla.com", "push.apple.com", "notify.windows.com"];
const LOOPBACK = new Set(["127.0.0.1", "localhost"]);

/**
 * Whether the relay may send to an endpoint a device gave it. `dev` is a local
 * run (DEV_LOGIN), where a loopback address stands in for a push service.
 */
export function allowedEndpoint(endpoint: string, dev: boolean): boolean {
  let u: URL;
  try { u = new URL(endpoint); } catch { return false; }
  if (u.username || u.password) return false;
  if (dev && u.protocol === "http:" && LOOPBACK.has(u.hostname)) return true;
  return u.protocol === "https:" && !u.port && SERVICES.some((s) => u.hostname === s || u.hostname.endsWith(`.${s}`));
}

export async function vapidHeader(endpoint: string, privateJwk: string, publicKey: string, subject: string): Promise<string> {
  const key = await crypto.subtle.importKey("jwk", JSON.parse(privateJwk), { name: "ECDSA", namedCurve: "P-256" }, false, ["sign"]);
  const aud = new URL(endpoint).origin;
  const body = `${text(JSON.stringify({ typ: "JWT", alg: "ES256" }))}.${text(JSON.stringify({ aud, exp: Math.floor(Date.now() / 1000) + 12 * 3600, sub: subject }))}`;
  const sig = await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, key, new TextEncoder().encode(body));
  return `vapid t=${body}.${b64(sig)}, k=${publicKey}`;
}

/** The push service's answer: 404 or 410 means the browser dropped this subscription. */
export async function wake(endpoint: string, env: Env): Promise<number> {
  const r = await fetch(endpoint, {
    method: "POST",
    headers: {
      Authorization: await vapidHeader(endpoint, env.VAPID_PRIVATE_JWK, env.VAPID_PUBLIC, env.VAPID_SUBJECT),
      TTL: "3600",
      Urgency: "high",
    },
  });
  void r.body?.cancel();
  return r.status;
}
