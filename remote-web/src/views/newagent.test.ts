import { afterEach, describe, expect, it } from "vitest";
import type { ToMachine, ToWeb } from "../../../src/remote/protocol";
import type { App, Fleet } from "../main";
import type { MachineSession } from "../session";
import { leaveNewAgent, showNewAgent } from "./newagent";

const fleet: Fleet = {
  type: "fleet.state", agents: [],
  projects: [{ id: "p1", name: "demo" }, { id: "p2", name: "other" }],
  presets: [{ id: "claude", label: "Claude" }, { id: "codex", label: "Codex" }],
};

afterEach(() => leaveNewAgent());

describe("the new agent form", () => {
  it("sends the chosen project, CLI and task, and keeps what was typed through a fleet update", () => {
    const sent: ToMachine[] = [];
    const session = { open: true, send: (m: ToMachine) => sent.push(m) } as unknown as MachineSession;
    const app = { sessions: new Map([["m:1", session]]), fleet: new Map([["m:1", fleet]]) } as unknown as App;
    const root = document.createElement("div");
    showNewAgent(root, app, "1");

    const [project, preset] = root.querySelectorAll("select");
    project.value = "p2";
    preset.value = "codex";
    root.querySelector("textarea")!.value = "  fix the build ";
    showNewAgent(root, app, "1");
    expect(root.querySelector("textarea")!.value).toBe("  fix the build ");

    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    expect(sent).toEqual([{ type: "agent.new", project: "p2", preset: "codex", task: "fix the build" }]);
  });

  it("starts one agent for two presses, and takes a press again once the computer refuses", () => {
    const sent: ToMachine[] = [];
    const session = { open: true, send: (m: ToMachine) => sent.push(m) } as unknown as MachineSession;
    const app = { sessions: new Map([["m:1", session]]), fleet: new Map([["m:1", fleet]]) } as unknown as App;
    const root = document.createElement("div");
    showNewAgent(root, app, "1");
    const submit = () => root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    const start = root.querySelector<HTMLButtonElement>("button[type=submit]")!;
    submit();
    submit();
    expect(sent).toHaveLength(1);
    expect(start.disabled).toBe(true);

    const refused: ToWeb = { type: "error", code: "no-project", message: "That project isn't open on this computer." };
    window.dispatchEvent(new CustomEvent("remote-message", { detail: { peer: "m:1", msg: refused } }));
    expect(root.textContent).toContain("That project isn't open");
    expect(start.disabled).toBe(false);
    submit();
    expect(sent).toHaveLength(2);
  });

  it("takes a press again when the new agent shows up", () => {
    const session = { open: true, send: () => {} } as unknown as MachineSession;
    const app = { sessions: new Map([["m:1", session]]), fleet: new Map([["m:1", fleet]]) } as unknown as App;
    const root = document.createElement("div");
    showNewAgent(root, app, "1");
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    const up = { id: "n", name: "claude", project: "demo", projectId: "p1", status: "working", title: null, hasChat: true, cols: 80, rows: 24 };
    app.fleet.set("m:1", { ...fleet, agents: [up] });
    showNewAgent(root, app, "1");
    expect(root.querySelector<HTMLButtonElement>("button[type=submit]")!.disabled).toBe(false);
    expect(root.textContent).toContain("claude is up");
  });

  it("says so when no project is open on the computer", () => {
    const session = { open: true, send: () => {} } as unknown as MachineSession;
    const app = { sessions: new Map([["m:1", session]]), fleet: new Map([["m:1", { ...fleet, projects: [] }]]) } as unknown as App;
    const root = document.createElement("div");
    showNewAgent(root, app, "1");
    expect(root.querySelector("form")).toBeNull();
    expect(root.textContent).toContain("no project open");
  });
});
