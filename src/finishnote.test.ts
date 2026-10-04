import { describe, expect, it } from "vitest";
import {
  Burst, FinishWatch, GROUP_MAX_MS, GROUP_MS, MIN_RUN_MS, SETTLE_MS,
  finishNote, finishedLabel, ranFor, worthTelling, type Finish, type FinishSnap,
} from "./finishnote";
import type { TaskState } from "./taskstate";

const snap = (state: TaskState, over: Partial<FinishSnap> = {}): FinishSnap => ({
  paneId: "p1", name: "Ana", project: "shop", state, limited: false, files: null, exitCode: null, stopAsked: false, ...over,
});

/** Feed the watch one state per second from t0 and collect what it reports. */
function play(w: FinishWatch, states: Array<FinishSnap | TaskState>, t0 = 0): Finish[] {
  const out: Finish[] = [];
  states.forEach((s, i) => out.push(...w.step([typeof s === "string" ? snap(s) : s], t0 + i * 1000)));
  return out;
}
const times = (n: number, s: TaskState): TaskState[] => Array(n).fill(s);

describe("FinishWatch", () => {
  it("says done once a long run has settled idle", () => {
    const w = new FinishWatch();
    const got = play(w, [...times(30, "working"), ...times(SETTLE_MS / 1000 + 1, "idle")]);
    expect(got).toHaveLength(1);
    expect(got[0]).toMatchObject({ paneId: "p1", kind: "done", ranMs: 30_000, at: 30_000 });
    expect(w.lastFinish("p1")).toEqual({ kind: "done", at: 30_000 });
  });

  it("says nothing for a short run", () => {
    const w = new FinishWatch();
    expect(play(w, [...times(MIN_RUN_MS / 1000 - 5, "working"), ...times(10, "idle")])).toEqual([]);
    expect(w.lastFinish("p1")).toBeNull();
  });

  it("says nothing for an agent that never worked", () => {
    expect(play(new FinishWatch(), times(60, "idle"))).toEqual([]);
  });

  it("treats a short pause as part of the run", () => {
    const w = new FinishWatch();
    const got = play(w, [...times(15, "working"), ...times(3, "idle"), ...times(15, "working"), ...times(8, "idle")]);
    expect(got).toHaveLength(1);
    expect(got[0].ranMs).toBe(33_000);
  });

  it("keeps the run open through a question", () => {
    const w = new FinishWatch();
    const got = play(w, [...times(10, "working"), ...times(20, "needs"), ...times(10, "working"), ...times(8, "idle")]);
    expect(got).toHaveLength(1);
    expect(got[0].ranMs).toBe(40_000);
  });

  it("says ready to review when it ends with changes", () => {
    const w = new FinishWatch();
    const got = play(w, [...times(25, "working"), ...times(8, "idle").map(() => snap("review", { files: 3 }))]);
    expect(got).toHaveLength(1);
    expect(got[0]).toMatchObject({ kind: "review", files: 3 });
  });

  it("reads the state at the end of the settle, not the first quiet tick", () => {
    const w = new FinishWatch();
    const got = play(w, [...times(25, "working"), "idle", "idle", ...times(6, "idle").map(() => snap("review", { files: 2 }))]);
    expect(got[0].kind).toBe("review");
  });

  it("says nothing when it is out of usage (Limit Guard does)", () => {
    const w = new FinishWatch();
    expect(play(w, [...times(25, "working"), ...times(8, "idle").map(() => snap("idle", { limited: true }))])).toEqual([]);
  });

  it("reports a crash at once, but not a stop you asked for or a clean exit", () => {
    const crash = play(new FinishWatch(), [...times(25, "working"), snap("stopped", { exitCode: 1 })]);
    expect(crash).toHaveLength(1);
    expect(crash[0]).toMatchObject({ kind: "crash", exitCode: 1, ranMs: 25_000 });
    expect(play(new FinishWatch(), [...times(25, "working"), snap("stopped", { exitCode: 1, stopAsked: true })])).toEqual([]);
    expect(play(new FinishWatch(), [...times(25, "working"), snap("stopped", { exitCode: 0 })])).toEqual([]);
    expect(play(new FinishWatch(), [...times(25, "working"), snap("merged")])).toEqual([]);
  });

  it("forgets the last finish when the agent starts again, or is gone", () => {
    const w = new FinishWatch();
    play(w, [...times(25, "working"), ...times(8, "idle")]);
    expect(w.lastFinish("p1")).not.toBeNull();
    w.step([snap("working")], 100_000);
    expect(w.lastFinish("p1")).toBeNull();
    play(w, [...times(25, "working"), ...times(8, "idle")], 101_000);
    expect(w.lastFinish("p1")).not.toBeNull();
    w.step([], 200_000);
    expect(w.lastFinish("p1")).toBeNull();
  });

  it("tracks agents apart", () => {
    const w = new FinishWatch();
    const out: Finish[] = [];
    for (let t = 0; t < 40; t++) {
      out.push(...w.step([
        snap(t < 25 ? "working" : "idle"),
        snap(t < 10 ? "working" : "idle", { paneId: "p2", name: "Bo" }),
      ], t * 1000));
    }
    expect(out.map((f) => f.name)).toEqual(["Ana"]); // Bo ran 10 s
  });
});

const fin = (over: Partial<Finish> = {}): Finish => ({
  paneId: "p1", name: "Ana", project: "shop", kind: "done", files: null, exitCode: null, ranMs: 240_000, at: 0, ...over,
});

describe("Burst", () => {
  it("waits for a quiet gap, then hands over everything at once", () => {
    const b = new Burst();
    b.add([fin()], 0);
    expect(b.take(GROUP_MS - 1)).toBeNull();
    b.add([fin({ paneId: "p2", name: "Bo" })], 2000);
    expect(b.take(2000 + GROUP_MS - 1)).toBeNull();
    expect(b.take(2000 + GROUP_MS)?.map((f) => f.name)).toEqual(["Ana", "Bo"]);
    expect(b.take(99_000)).toBeNull();
  });

  it("doesn't wait forever on a steady trickle", () => {
    const b = new Burst();
    for (let t = 0; t < GROUP_MAX_MS; t += 2000) b.add([fin({ paneId: `p${t}` })], t);
    expect(b.take(GROUP_MAX_MS)).not.toBeNull();
  });

  it("keeps one entry per agent", () => {
    const b = new Burst();
    b.add([fin({ kind: "done" })], 0);
    b.add([fin({ kind: "review", files: 2 })], 1000);
    expect(b.take(10_000)).toEqual([fin({ kind: "review", files: 2 })]);
  });

  it("ignores empty adds", () => {
    const b = new Burst();
    b.add([], 0);
    expect(b.take(50_000)).toBeNull();
  });
});

describe("worthTelling", () => {
  it("stays quiet only about the agent you are looking at", () => {
    expect(worthTelling(fin(), "p1", true)).toBe(false);
    expect(worthTelling(fin(), "p1", false)).toBe(true);
    expect(worthTelling(fin(), "p2", true)).toBe(true);
    expect(worthTelling(fin(), null, true)).toBe(true);
  });
});

describe("finishNote", () => {
  it("words a single finish", () => {
    expect(finishNote([fin()])).toEqual({ title: "Ana is done", body: "Worked 4 min in shop.", open: "p1" });
    expect(finishNote([fin({ kind: "review", files: 1 })]).title).toBe("Ana is ready to review");
    expect(finishNote([fin({ kind: "review", files: 1 })]).body).toBe("1 file changed in shop, after 4 min.");
    expect(finishNote([fin({ kind: "crash", exitCode: 3 })])).toMatchObject({
      title: "Ana stopped with an error", body: "It exited with code 3 after 4 min, in shop.",
    });
  });

  it("groups a burst and opens the one that most wants you", () => {
    const n = finishNote([
      fin({ paneId: "a", name: "Ana", at: 1 }),
      fin({ paneId: "b", name: "Bo", kind: "review", files: 3, at: 2 }),
      fin({ paneId: "c", name: "Cy", kind: "crash", exitCode: 1, at: 3 }),
      fin({ paneId: "d", name: "Di", at: 4 }),
    ]);
    expect(n.title).toBe("4 agents finished");
    expect(n.body).toBe("Cy: stopped with an error\nBo: ready to review, 3 files changed\nAna: done\nand 1 more");
    expect(n.open).toBe("c");
  });

  it("never uses an em dash", () => {
    const all = [finishNote([fin()]), finishNote([fin({ kind: "review", files: 2 }), fin({ paneId: "x", kind: "crash", exitCode: 1 })])];
    for (const n of all) expect(`${n.title}${n.body}`).not.toContain(String.fromCharCode(0x2014));
  });
});

describe("ranFor", () => {
  it("reads like a person would say it", () => {
    expect(ranFor(45_000)).toBe("45 s");
    expect(ranFor(4 * 60_000)).toBe("4 min");
    expect(ranFor(80 * 60_000)).toBe("1 h 20 min");
    expect(ranFor(120 * 60_000)).toBe("2 h");
  });
});

describe("finishedLabel", () => {
  it("marks review, and done only after work", () => {
    expect(finishedLabel("review", true, false)).toBe("review");
    expect(finishedLabel("idle", true, false)).toBe("done");
    expect(finishedLabel("idle", false, false)).toBeNull();
    expect(finishedLabel("idle", true, true)).toBeNull();
    expect(finishedLabel("working", true, false)).toBeNull();
  });
});
