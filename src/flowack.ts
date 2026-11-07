// Tells the backend how much of a terminal's output has been parsed, so it can
// hold the program back while the window is behind (src-tauri/src/core/flow.rs).
// One message per `step` bytes, or per `ms` while output keeps coming, with the
// first one after a quiet spell going out at once: a few ack messages a second
// for a busy terminal, not one per chunk.

export interface Acker {
  /** `n` more bytes have been parsed by the terminal. */
  parsed(n: number): void;
  /** Send what is owed now. */
  flush(): void;
  dispose(): void;
}

/** Bytes parsed before an ack goes out whatever the clock says. The backend
 *  holds a program at 512 KB unparsed, so 32 KB keeps its count within 6%. */
export const ACK_STEP = 32 * 1024;
/** Longest an ack waits while output keeps coming. */
export const ACK_MS = 250;

export function createAcker(send: (n: number) => void, step = ACK_STEP, ms = ACK_MS): Acker {
  let owed = 0;
  let sentAt = -Infinity;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const flush = () => {
    if (timer !== null) { clearTimeout(timer); timer = null; }
    if (owed <= 0) return;
    const n = owed;
    owed = 0;
    sentAt = Date.now();
    send(n);
  };
  return {
    parsed(n) {
      if (n <= 0) return;
      owed += n;
      const wait = ms - (Date.now() - sentAt);
      if (owed >= step || wait <= 0) flush();
      else if (timer === null) timer = setTimeout(flush, wait);
    },
    flush,
    dispose() {
      if (timer !== null) clearTimeout(timer);
      timer = null;
      owed = 0;
    },
  };
}
