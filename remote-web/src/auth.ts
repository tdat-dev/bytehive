import { loadToken, saveToken } from "./keys";

const RELAY = import.meta.env.VITE_RELAY as string;

/** Set by `signIn` in this tab only: a login in the address is taken only if it was asked for. */
export const NONCE_KEY = "bytehive.signin";

/** Whose login this is, from the unsigned body of the token; the relay is the one that verifies it. */
function peerOf(token: string): string | null {
  try {
    const body = token.split(".")[0].replace(/-/g, "+").replace(/_/g, "/");
    const claims = JSON.parse(atob(body)) as { peer?: unknown };
    return typeof claims.peer === "string" ? claims.peer : null;
  } catch {
    return null;
  }
}

/** The relay hands a login back in the fragment; it leaves the address at once. */
export async function takeTokenFromUrl(deviceId: string): Promise<void> {
  const m = location.hash.match(/^#token=(.+)$/);
  if (!m) return;
  const asked = sessionStorage.getItem(NONCE_KEY) !== null;
  sessionStorage.removeItem(NONCE_KEY);
  history.replaceState(null, "", location.pathname);
  // A link someone else made must not swap this device's login for theirs.
  if (!asked) return;
  const token = decodeURIComponent(m[1]);
  if (peerOf(token) !== `d:${deviceId}`) return;
  await saveToken(token);
}

export const token = () => loadToken();

/** A login token works for this device only, so it carries the device's id. */
export function signIn(deviceId: string): void {
  sessionStorage.setItem(NONCE_KEY, crypto.randomUUID());
  location.href = `${RELAY}/auth/start?peer=d:${deviceId}`;
}
