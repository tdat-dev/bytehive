/** Calm: Maestro's window is not the one you are using (you are watching a
 *  video, or working in another app). Terminals then draw a few times a second
 *  instead of on every byte, and CSS animations hold still, so Maestro stops
 *  competing for the GPU that also draws your screen. */

/** How often a terminal draws while Maestro is in the background. */
export const CALM_MS = 250;

let calm = false;
const listeners = new Set<(calm: boolean) => void>();

export const isCalm = (): boolean => calm;

/** Runs `cb` whenever calm turns on or off. Returns the unsubscribe. */
export function onCalmChange(cb: (calm: boolean) => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function update(): void {
  const next = !document.hasFocus();
  if (next === calm) return;
  calm = next;
  document.body?.classList.toggle("calm", calm);
  for (const cb of listeners) cb(calm);
}

/** Starts watching the window. Safe to call more than once. */
let started = false;
export function initCalm(): void {
  if (started) return;
  started = true;
  window.addEventListener("focus", update);
  window.addEventListener("blur", update);
  document.addEventListener("visibilitychange", update);
  update();
}

/** Holds a terminal's output while calm and hands it over every CALM_MS; out
 *  of calm it passes each chunk straight through. The order of chunks is kept. */
export function createPacer<T>(write: (chunk: T) => void, ms: number = CALM_MS): { push(chunk: T): void; flush(): void; dispose(): void } {
  let held: T[] = [];
  let timer: ReturnType<typeof setTimeout> | null = null;
  const flush = () => {
    if (timer !== null) { clearTimeout(timer); timer = null; }
    const out = held;
    held = [];
    for (const c of out) write(c);
  };
  const off = onCalmChange((c) => { if (!c) flush(); });
  return {
    push(chunk) {
      if (!calm && !held.length) { write(chunk); return; }
      held.push(chunk);
      if (timer === null) timer = setTimeout(flush, ms);
    },
    flush,
    dispose() { if (timer !== null) clearTimeout(timer); timer = null; held = []; off(); },
  };
}

/** For tests: set calm directly. */
export function setCalmForTest(v: boolean): void {
  if (v === calm) return;
  calm = v;
  for (const cb of listeners) cb(calm);
}
