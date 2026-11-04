import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./appstate", () => ({ workspaces: new Map() }));
vi.mock("./ipc", () => ({ claudeLiveSessions: async () => [] }));
vi.mock("./session", () => ({ saveSession: () => {} }));

import { follow, stopSessionFollow, START_SLACK_MS } from "./sessionfollow";
import type { Pane } from "./panetypes";

const T = 1_792_395_000_000;
const A = "aaaaaaaa-0000-0000-0000-000000000001";
const A2 = "aaaaaaaa-0000-0000-0000-000000000002";
const B = "bbbbbbbb-0000-0000-0000-000000000001";
const pane = (id: string, sessionId: string | undefined, over: Partial<Pane> = {}) =>
  ({ id, running: true, run: 1, spawnedAt: T, spec: { badge: "claude", sessionId, ranIn: "D:\\app" }, ...over }) as unknown as Pane;
const live = (pid: number, session_id: string, started_ms = T + 1500, cwd = "D:\\app") => ({ pid, session_id, cwd, started_ms });

describe("following an agent's conversation", () => {
  beforeEach(() => stopSessionFollow());

  it("moves on with a /clear, after finding its process by the session it started on", () => {
    const p = pane("p1", A);
    expect(follow([p], [live(10, A)])).toEqual([]);
    // the same process is in a new conversation now
    expect(follow([p], [live(10, A2)])).toEqual([p]);
    expect(p.spec.sessionId).toBe(A2);
  });

  it("tells two agents in one folder apart by the session each started on", () => {
    const p = pane("p1", A), q = pane("p2", B);
    follow([p, q], [live(10, A), live(11, B)]);
    follow([p, q], [live(10, A), live(11, A2)]);
    expect([p.spec.sessionId, q.spec.sessionId]).toEqual([A, A2]);
  });

  it("finds a process by its folder and start time when its session is not the one asked for", () => {
    const p = pane("p1", A);
    expect(follow([p], [live(10, A2, T + 900, "d:/app/")])).toEqual([p]);
    expect(p.spec.sessionId).toBe(A2);
  });

  it("guesses nothing when two processes could be it, or one started long before", () => {
    const p = pane("p1", A);
    follow([p], [live(10, A2), live(11, B)]);
    follow([p], [live(12, B, T - START_SLACK_MS - 1000)]);
    expect(p.spec.sessionId).toBe(A);
  });

  it("leaves stopped agents and other CLIs alone", () => {
    const stopped = pane("p1", A, { running: false });
    const codex = pane("p2", A, { spec: { badge: "codex", sessionId: A, ranIn: "D:\\app" } as Pane["spec"] });
    expect(follow([stopped, codex], [live(10, A2)])).toEqual([]);
  });

  it("starts looking again for a new run of the same pane", () => {
    const p = pane("p1", A);
    follow([p], [live(10, A)]);
    p.run = 2;
    p.spec.sessionId = B;
    expect(follow([p], [live(10, A), live(20, B)])).toEqual([]);
    expect(p.spec.sessionId).toBe(B);
  });
});
