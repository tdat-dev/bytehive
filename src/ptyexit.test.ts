import { describe, it, expect, beforeEach } from "vitest";
import { exitVerdict, forgetHeld, holdExit, replayEarlyExit, setExitHandler, takeEarlyExit, type ExitSubject } from "./ptyexit";

/** The race as it happens: a pane booting, its CLI dying before spawnPty resolves. */
function simulate(pane: ExitSubject & { running: boolean }, id: string, newRun: number, exitFirst: boolean) {
  const exited: number[] = [];
  setExitHandler((_id, code, run) => {
    // what bridges.ts does with an exit event
    if (exitVerdict(pane, run) !== "apply") return;
    pane.running = false;
    exited.push(code);
  });
  const onExitEvent = (run: number, code: number) => {
    const v = exitVerdict(pane, run);
    if (v === "early") holdExit(id, run, code);
    else if (v === "apply") {
      pane.running = false;
      exited.push(code);
    }
  };
  // boot(): mark spawning, await spawnPty...
  pane.spawning = true;
  if (exitFirst) onExitEvent(newRun, 1);
  // ...spawnPty resolves
  pane.run = newRun;
  pane.spawning = false;
  pane.running = true;
  replayEarlyExit(id, pane.run);
  if (!exitFirst) onExitEvent(newRun, 1);
  return exited;
}

describe("pty exits racing the spawn", () => {
  beforeEach(() => {
    takeEarlyExit("a", undefined);
  });

  it("a first run that dies before spawnPty resolves ends up exited, not running", () => {
    const pane = { running: false } as ExitSubject & { running: boolean };
    expect(simulate(pane, "a", 7, true)).toEqual([1]);
    expect(pane.running).toBe(false);
  });

  it("a restarted run that dies before spawnPty resolves ends up exited too", () => {
    const pane = { running: false, run: 3 } as ExitSubject & { running: boolean };
    expect(simulate(pane, "a", 8, true)).toEqual([1]);
    expect(pane.running).toBe(false);
  });

  it("the normal order still works", () => {
    const pane = { running: false } as ExitSubject & { running: boolean };
    expect(simulate(pane, "a", 9, false)).toEqual([1]);
    expect(pane.running).toBe(false);
  });

  it("an older run's late exit never ends the new run", () => {
    expect(exitVerdict({ run: 5 }, 4)).toBe("stale");
    expect(exitVerdict({ run: 5, spawning: true }, 4)).toBe("stale");
  });

  it("a detached window's pane (no run number, not spawning) takes the exit", () => {
    expect(exitVerdict({}, 12)).toBe("apply");
  });

  it("an exit held for another run is not played back", () => {
    holdExit("a", 20, 1);
    expect(replayEarlyExit("a", 21)).toBe(false);
    expect(takeEarlyExit("a", 20)).toBeUndefined(); // cleared with the replay
  });
it("forgetting a removed pane drops its held exits", () => {
    holdExit("gone", 3, 1);
    forgetHeld("gone");
    expect(takeEarlyExit("gone", 3)).toBeUndefined();
  });
});