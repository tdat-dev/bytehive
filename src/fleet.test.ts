import { describe, it, expect } from "vitest";
import { fleetStatus, fleetSummary, sortFleet, needsCount } from "./fleet";
import type { FleetPane } from "./agentbridge";

const mk = (over: Partial<FleetPane>): FleetPane => ({
  id: "p",
  name: "Agent",
  color: "#fff",
  wsId: "w",
  wsName: "ws",
  running: true,
  state: "idle",
  spawnedAt: 1,
  lastOutputAt: 2,
  ...over,
});

describe("fleetStatus", () => {
  it("says needs only for an agent whose task state is needs", () => {
    expect(fleetStatus("needs", true)).toBe("needs");
    for (const s of ["review", "idle", "working", "stopped", "merged"] as const) expect(fleetStatus(s, true)).not.toBe("needs");
  });
  it("calls a working agent active, and one that is quiet idle", () => {
    expect(fleetStatus("working", true)).toBe("active");
    expect(fleetStatus("idle", true)).toBe("idle");
    expect(fleetStatus("review", true)).toBe("idle");
  });
  it("stopped when not running, whatever the state last was", () => {
    expect(fleetStatus("needs", false)).toBe("stopped");
    expect(fleetStatus("merged", false)).toBe("stopped");
  });
  it("before the first tick a running agent is idle, never needs", () => {
    expect(fleetStatus(null, true)).toBe("idle");
    expect(fleetStatus(null, false)).toBe("stopped");
  });
});

describe("sortFleet", () => {
  it("orders needs → active → idle → stopped, then by name", () => {
    const rows = sortFleet([
      mk({ id: "1", name: "Zeta", running: false, state: "stopped" }),
      mk({ id: "2", name: "Beta", state: "needs" }),
      mk({ id: "3", name: "Alpha", state: "idle" }),
      mk({ id: "4", name: "Gamma", state: "working" }),
    ]);
    expect(rows.map((r) => r.name)).toEqual(["Beta", "Gamma", "Alpha", "Zeta"]);
    expect(rows.map((r) => r.status)).toEqual(["needs", "active", "idle", "stopped"]);
  });
});

describe("fleetSummary", () => {
  it("counts Company items as the headline does, and calls them that", () => {
    const rows = sortFleet([mk({ id: "1", state: "needs" }), mk({ id: "2", state: "working" })]);
    expect(fleetSummary(rows, 0).replace(/<[^>]+>/g, "")).toBe("2 agents · 1 needs you · 1 working");
    expect(fleetSummary(rows, 2).replace(/<[^>]+>/g, "")).toBe("2 agents · 1 needs you · 2 Company items need you · 1 working");
    expect(fleetSummary([], 1).replace(/<[^>]+>/g, "")).toBe("0 agents · 1 Company item needs you");
  });
});

describe("needsCount", () => {
  it("counts only running agents whose task needs you", () => {
    expect(needsCount([mk({ state: "needs" }), mk({ state: "needs", running: false }), mk({ state: "idle" })])).toBe(1);
  });
});
