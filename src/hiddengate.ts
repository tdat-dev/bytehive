// Timers that keep running while the window is hidden (minimized, in the tray,
// another app in front) mostly redraw things nobody can see. A gate lets such
// a timer keep its interval but act only on every nth tick while hidden, and on
// every tick again as soon as the window is back. Work that must go on in the
// background (limit guard, scheduler, fleet delivery) is only slowed a little.

/** Every tick while the window shows; every `n`th while it is hidden. */
export function hiddenGate(n: number): () => boolean {
  let k = 0;
  return () => {
    if (typeof document === "undefined" || !document.hidden) {
      k = 0;
      return true;
    }
    k += 1;
    return k % n === 0;
  };
}
