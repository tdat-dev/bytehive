/* Two ByteHive apps running at once, the installed one and a dev build say, must
 * not both run the company: each would read the requests and start everyone. The
 * one that runs it keeps engine.lock fresh, and the other stays out while it does. */
import { parseOwned } from "./org";

export interface EngineLock { pid: number; at: number } // `at` is the last heartbeat, in ms

/** No heartbeat for this long: whoever held the lock is gone. */
export const LOCK_STALE_MS = 15_000;

export function parseLock(text: string): EngineLock | null {
  try {
    const l = parseOwned<EngineLock>(text);
    return Number.isFinite(l?.pid) && Number.isFinite(l?.at) ? l : null;
  } catch {
    return null;
  }
}

/** Whether this app may run the company. Its own pid is a window that reloaded. A
 *  heartbeat from far ahead of the clock counts as stale: it would never age. */
export function mayRun(lock: EngineLock | null, me: number, now: number): boolean {
  return !lock || lock.pid === me || Math.abs(now - lock.at) >= LOCK_STALE_MS;
}
