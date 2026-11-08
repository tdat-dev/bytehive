// Tells the backend how far a terminal has parsed its output, so it can hold
// the program back while the window is behind (src-tauri/src/core/flow.rs).
// What goes out is a running total, not a delta: an ack that is lost (a failed
// invoke) or a chunk whose callback never comes is made good by the next one.
// At most one message per `minMs`, and none later than `maxMs` after the parse
// it reports, so a busy terminal sends a few acks a second, not one per chunk.

export interface Acker {
  /** The terminal has parsed everything up to `total` bytes of the stream. */
  parsedTo(total: number): void;
  /** Send what is owed now. */
  flush(): void;
  dispose(): void;
}

/** Bytes parsed after which an ack goes out as soon as `minMs` allows. The
 *  backend holds a program at 512 KB unparsed, so 32 KB keeps its count within 6%. */
export const ACK_STEP = 32 * 1024;
/** Shortest time between two acks of one terminal. */
export const ACK_MIN_MS = 50;
/** Longest an ack waits after a parse. */
export const ACK_MS = 250;

export function createAcker(send: (total: number) => void, step = ACK_STEP, minMs = ACK_MIN_MS, maxMs = ACK_MS): Acker {
  let best = 0; // parsed so far
  let told = 0; // what the backend was last told
  let toldAt = -Infinity;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const flush = () => {
    if (timer !== null) { clearTimeout(timer); timer = null; }
    if (best <= told) return;
    told = best;
    toldAt = Date.now();
    send(best);
  };
  return {
    parsedTo(total) {
      if (total <= best) return;
      best = total;
      const since = Date.now() - toldAt;
      const due = best - told >= step ? minMs : maxMs;
      if (since >= due) { flush(); return; }
      const wait = due - since;
      if (timer !== null) clearTimeout(timer);
      timer = setTimeout(flush, wait);
    },
    flush,
    dispose() {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    },
  };
}
