// What the relay's session store answers (relay/src/storeroutes.ts). All of it is sealed.

const RELAY = import.meta.env.VITE_RELAY as string;

export interface Vault { salt: string; rounds: number; check: string }
export interface Listed { sid: string; machine: string; meta: string; updated: number; segments: number }
export interface Segment { n: number; sealed: string }

export class StoreError extends Error {
  constructor(readonly status: number) {
    super(status === 401 ? "Sign in again." : status === 403 ? "This device was removed." : `The relay answered ${status}.`);
  }
}

async function get<T>(token: string, path: string): Promise<T | null> {
  const r = await fetch(`${RELAY}${path}`, { headers: { Authorization: `Bearer ${token}` } });
  if (r.status === 404) return null;
  if (!r.ok) throw new StoreError(r.status);
  return (await r.json()) as T;
}

export const vault = (token: string) => get<Vault>(token, "/vault");
export const list = async (token: string) => (await get<Listed[]>(token, "/store/list")) ?? [];

/** Every segment of a session, page after page. */
export async function segments(token: string, sid: string): Promise<Segment[]> {
  const out: Segment[] = [];
  let from = 0;
  for (;;) {
    const page = await get<{ segments: Segment[]; more: boolean }>(token, `/store/segments?sid=${encodeURIComponent(sid)}&from=${from}`);
    if (!page) return out;
    out.push(...page.segments);
    if (!page.more || !page.segments.length) return out;
    from = page.segments[page.segments.length - 1].n + 1;
  }
}
