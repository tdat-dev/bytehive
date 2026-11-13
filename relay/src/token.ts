// Signed, expiring strings. Each carries a `typ` and only verifies as that type,
// so a sign-in state can't pass for a login, nor a ticket for either:
//   state  the OAuth round trip (start to callback)
//   login  what a machine or device holds after signing in
//   ticket the short pass that opens one socket (a browser can't set headers on one)

const b64 = (b: Uint8Array) => btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const unb64 = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));

const hmacKey = (secret: string) =>
  crypto.subtle.importKey("raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);

export async function sign(claims: Record<string, unknown>, seconds: number, secret: string): Promise<string> {
  const body = b64(new TextEncoder().encode(JSON.stringify({ ...claims, exp: Math.floor(Date.now() / 1000) + seconds })));
  const mac = await crypto.subtle.sign("HMAC", await hmacKey(secret), new TextEncoder().encode(body));
  return `${body}.${b64(new Uint8Array(mac))}`;
}

export async function verify(token: string, secret: string, typ: string): Promise<Record<string, unknown> | null> {
  const [body, mac] = token.split(".");
  if (!body || !mac) return null;
  try {
    const ok = await crypto.subtle.verify("HMAC", await hmacKey(secret), unb64(mac), new TextEncoder().encode(body));
    if (!ok) return null;
    const claims = JSON.parse(new TextDecoder().decode(unb64(body)));
    return claims.typ === typ && typeof claims.exp === "number" && claims.exp > Date.now() / 1000 ? claims : null;
  } catch {
    return null;
  }
}
