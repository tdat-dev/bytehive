/* Two ByteHive apps running at once, the installed one and a dev build say, must
 * not both run the company: each would read the requests and start everyone. The
 * one that runs it keeps engine.lock fresh, and the other stays out while it does.
 * A silent lock is not a free one: an app that went quiet still has its panes open,
 * and a new owner would start everyone a second time. So a stale lock is taken only
 * from a dead process. A live one gets ten minutes, and then the owner is told. */
import { parseOwned } from "./org";

export interface EngineLock { pid: number; at: number } // `at` is the last heartbeat, in ms

/** What a read of the lock gave. Unknown is a read that failed, which says nothing. */
export type LockRead = EngineLock | null | "unknown";

/** What this app does on a beat. Take-over is a run the owner has to hear about. */
export type LockMove = "run" | "take-over" | "stay-out" | "skip";

/** No heartbeat for this long: whoever held the lock has stopped, or is dead. */
export const LOCK_STALE_MS = 15_000;

/** No heartbeat from a living app for this long: it is stuck, and the company won't wait. */
export const LOCK_ABANDONED_MS = 600_000;

export function parseLock(text: string): EngineLock | null {
  try {
    const l = parseOwned<EngineLock>(text);
    return Number.isFinite(l?.pid) && Number.isFinite(l?.at) ? l : null;
  } catch {
    return null;
  }
}

/** Another app's lock has gone quiet, so it matters whether that app is alive. A
 *  heartbeat from far ahead of the clock counts too: it would never age. */
export const lockSilent = (lock: EngineLock, me: number, now: number): boolean =>
  lock.pid !== me && Math.abs(now - lock.at) >= LOCK_STALE_MS;

/** What this app does about the lock. Its own pid is a window that reloaded.
 *  `awakeSince` is when this app last woke or saw its clock jump: silence only counts
 *  from there, or an hour's sleep or a moved clock would look like a stuck app. */
export function lockMove(read: LockRead, me: number, now: number, holderAlive: boolean, awakeSince = 0): LockMove {
  if (read === "unknown") return "skip";
  if (!read || read.pid === me) return "run";
  if (!lockSilent(read, me, now)) return "stay-out";
  if (!holderAlive) return "run";
  return Math.min(Math.abs(now - read.at), now - awakeSince) >= LOCK_ABANDONED_MS ? "take-over" : "stay-out";
}
