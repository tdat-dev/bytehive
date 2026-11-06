// src/company/queue.ts
/* A hung operation must not stop the company for good: git worktree add has no
 * timeout of its own, and everything else waits behind it. */

/** Starting a plan with five hires and a slow git can take minutes. */
export const OP_LIMIT_MS = 300_000;

export class OpTimeout extends Error {
  constructor(label: string, ms: number) {
    super(`${label} took over ${Math.round(ms / 1000)} s, so ByteHive moved on. It may still finish.`);
  }
}

/** The operation goes on running after a timeout: there is no cancelling a backend
 *  call. Only the queue stops waiting for it. */
export function withLimit<T>(op: () => Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new OpTimeout(label, ms)), ms);
    void Promise.resolve().then(op).then(resolve, reject).finally(() => clearTimeout(timer));
  });
}
