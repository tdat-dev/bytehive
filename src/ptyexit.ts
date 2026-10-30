// What a `pty-exit` event means for the pane it names, including the one that
// arrives too early: a CLI that fails at once (bad flag, missing binary behind
// cmd.exe) can exit before `spawnPty` has handed back its run number. Taken
// then, the exit either marked a pane "exited" that boot then set running, or
// was dropped as an older run's; either way the pane showed "running" forever.
// Such an exit is held here and played back the moment the spawn resolves.
// No DOM, no IPC: unit-tested on its own.

/** The part of a pane an exit decision needs. */
export interface ExitSubject {
  /** Run number of the current process, once `spawnPty` resolved (none in a
   *  detached window, which attached to a process it didn't start). */
  run?: number;
  /** A `spawnPty` call for this pane is in flight. */
  spawning?: boolean;
}

/** "apply": this is the pane's process ending. "stale": an older run's late
 *  exit (restart in place), ignore it. "early": a run still being spawned
 *  ended before we learned its number, hold it for `replayEarlyExit`. */
export function exitVerdict(p: ExitSubject, run: number): "apply" | "stale" | "early" {
  if (p.run === run) return "apply";
  // Run numbers only grow: one newer than what the pane knows is the run in flight.
  if (p.spawning && (p.run === undefined || run > p.run)) return "early";
  if (p.run !== undefined) return "stale";
  return "apply";
}

const held = new Map<string, { run: number; code: number }[]>();

/** Keep an early exit until the spawn it belongs to resolves. */
export function holdExit(id: string, run: number, code: number): void {
  const list = held.get(id) ?? [];
  list.push({ run, code });
  held.set(id, list);
}

/** The exit code held for this run of the pane, if its process already ended.
 *  Clears everything held for the pane: other runs' exits are moot now. */
export function takeEarlyExit(id: string, run: number | undefined): number | undefined {
  const list = held.get(id);
  held.delete(id);
  return list?.find((x) => x.run === run)?.code;
}
/** Drop everything held for a removed pane (its spawn never resolves). */
export function forgetHeld(id: string): void {
  held.delete(id);
}


let applyExit: ((id: string, code: number, run: number) => void) | null = null;

/** Who marks a pane exited (bridges.ts, where pty-exit is listened to). */
export function setExitHandler(fn: (id: string, code: number, run: number) => void): void {
  applyExit = fn;
}

/** Right after a spawn resolved: if its process already ended, end the pane now. */
export function replayEarlyExit(id: string, run: number | undefined): boolean {
  const code = takeEarlyExit(id, run);
  if (code === undefined || run === undefined) return false;
  applyExit?.(id, code, run);
  return true;
}
