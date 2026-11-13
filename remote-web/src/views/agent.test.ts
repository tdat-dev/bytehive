import { afterEach, describe, expect, it, vi } from "vitest";
import type { RemoteAgent } from "../../../src/remote/protocol";
import type { App, Fleet } from "../main";
import type { MachineSession } from "../session";
import { leaveAgent, showAgent } from "./agent";

const opened: RemoteAgent[] = [];
vi.mock("./terminal", () => ({
  openTerminal: (_el: HTMLElement, _s: unknown, _peer: string, a: RemoteAgent) => {
    opened.push(a);
    return () => {};
  },
}));

const agent = (cols: number, rows: number): RemoteAgent => ({
  id: "a1", name: "claude", project: "p", projectId: "p1", status: "idle", title: null, hasChat: false, cols, rows,
});
const fleet = (a: RemoteAgent): Fleet => ({ type: "fleet.state", agents: [a], projects: [], presets: [] });

afterEach(() => {
  leaveAgent();
  opened.length = 0;
});

describe("the agent page", () => {
  it("opens the terminal at the size the agent has now, not the size of the page build", () => {
    const session = { open: true } as unknown as MachineSession;
    const app = {
      sessions: new Map([["m:1", session]]),
      fleet: new Map([["m:1", fleet(agent(80, 24))]]),
    } as unknown as App;
    const root = document.createElement("div");
    showAgent(root, app, "1", "a1");
    expect(opened.map((a) => [a.cols, a.rows])).toEqual([[80, 24]]);

    // The computer resized the terminal; the page is not rebuilt for a fleet update.
    app.fleet.set("m:1", fleet(agent(93, 30)));
    showAgent(root, app, "1", "a1");
    expect(opened).toHaveLength(1);

    // Reopening the Terminal tab (the only tab here) uses the new size.
    root.querySelectorAll("button")[0].click();
    expect(opened.map((a) => [a.cols, a.rows])).toEqual([[80, 24], [93, 30]]);
  });

  it("says so when the agent is gone by the time the tab opens", () => {
    const session = { open: true } as unknown as MachineSession;
    const app = {
      sessions: new Map([["m:1", session]]),
      fleet: new Map([["m:1", fleet(agent(80, 24))]]),
    } as unknown as App;
    const root = document.createElement("div");
    showAgent(root, app, "1", "a1");
    app.fleet.set("m:1", { ...fleet(agent(80, 24)), agents: [] });
    root.querySelectorAll("button")[0].click();
    expect(opened).toHaveLength(1);
    expect(root.textContent).toContain("Not found.");
  });
});
