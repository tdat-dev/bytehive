import { afterEach, describe, expect, it, vi } from "vitest";
import type { Peer, RemoteAgent } from "../../../src/remote/protocol";
import type { PairedMachine } from "../keys";
import type { App, Fleet, Invite } from "../main";
import type { MachineSession } from "../session";
import { ago, showHome, showMachine } from "./home";

const known = vi.hoisted(() => ({ list: [] as PairedMachine[] }));
vi.mock("../keys", () => ({ machines: async () => known.list }));
const qr = vi.hoisted(() => ({ toCanvas: vi.fn(async (_canvas: HTMLCanvasElement, _text: string) => {}) }));
vi.mock("qrcode", () => ({ default: qr }));

const machine = (id: string, name: string): PairedMachine => ({ id, name, key: new Uint8Array() });
const peer = (id: string, name: string, online: boolean, lastSeen = 0): Peer => ({ id: `m:${id}`, name, online, lastSeen });
const agent = (id: string, project: string, status: string, title: string | null = null): RemoteAgent => ({
  id, name: `agent-${id}`, project, projectId: project, status, title, hasChat: true, cols: 80, rows: 24,
});
const fleet = (...agents: RemoteAgent[]): Fleet => ({ type: "fleet.state", agents, projects: [], presets: [] });
const open = (name: string) => ({ open: true, machine: { id: "x", name }, send: vi.fn() }) as unknown as MachineSession;

function appWith(o: { peers?: Peer[]; sessions?: [string, MachineSession][]; fleet?: [string, Fleet][]; invites?: [string, Invite][]; link?: App["link"] }): App {
  return {
    relay: { peers: o.peers ?? [] },
    sessions: new Map(o.sessions ?? []),
    fleet: new Map(o.fleet ?? []),
    invites: new Map(o.invites ?? []),
    trouble: new Map(),
    link: o.link ?? "up",
  } as unknown as App;
}
const texts = (root: HTMLElement, sel: string) => [...root.querySelectorAll(sel)].map((e) => e.textContent);

afterEach(() => { known.list = []; qr.toCanvas.mockClear(); });

describe("how long ago", () => {
  const at = 1_000_000_000_000;
  it("reads like a person says it", () => {
    expect(ago(at - 20_000, at)).toBe("just now");
    expect(ago(at - 60_000, at)).toBe("1 minute ago");
    expect(ago(at - 5 * 60_000, at)).toBe("5 minutes ago");
    expect(ago(at - 2 * 3_600_000, at)).toBe("2 hours ago");
    expect(ago(at - 3 * 86_400_000, at)).toBe("3 days ago");
  });
});

describe("the home page", () => {
  it("offers pairing when no computer is paired", async () => {
    const root = document.createElement("div");
    await showHome(root, appWith({}));
    expect(root.textContent).toContain("No computer paired yet");
    expect(root.querySelector<HTMLAnchorElement>('a[href="#/pair"]')?.textContent).toBe("Pair a computer");
    expect(root.querySelectorAll(".machine")).toHaveLength(0);
  });

  it("puts every agent that needs you first, then machines, then projects, each agent once", async () => {
    known.list = [machine("1", "Desk"), machine("2", "Laptop")];
    const root = document.createElement("div");
    await showHome(root, appWith({
      peers: [peer("1", "Desk", true), peer("2", "Laptop", true)],
      sessions: [["m:1", open("Desk")], ["m:2", open("Laptop")]],
      fleet: [
        ["m:1", fleet(agent("a", "web", "working", "fixing the build"), agent("b", "api", "idle"))],
        ["m:2", fleet(agent("c", "web", "needs", "Do you want to proceed?"), agent("d", "web", "working"))],
      ],
    }));
    expect(root.querySelector("h1")?.textContent).toBe("1 agent needs you");
    expect(texts(root, ".needs .agent-name")).toEqual(["web / agent-c"]);
    expect(texts(root, ".needs .agent-note")).toEqual(["Do you want to proceed? · Laptop"]);
    expect(texts(root, ".machine h2")).toEqual(["Desk", "Laptop"]);
    expect(texts(root, ".machine:first-of-type .project h3")).toEqual(["api", "web"]);
    expect(texts(root, ".machine .agent-name")).toEqual(["agent-b", "agent-a", "agent-d"]);
    expect(root.querySelector('.machine a[href="#/m/2/a/c"]')).toBeNull();
    expect(root.querySelector('.needs a[href="#/m/2/a/c"]')).not.toBeNull();
  });

  it("says when an offline computer was last seen, and asks nothing of it", async () => {
    known.list = [machine("1", "Desk")];
    const root = document.createElement("div");
    await showHome(root, appWith({ peers: [peer("1", "Desk", false, Date.now() - 2 * 3_600_000)] }));
    expect(root.querySelector(".machine-state")?.textContent).toBe("Offline, last seen 2 hours ago");
    expect(root.querySelector("h1")?.textContent).toBe("Nothing needs you");
  });

  it("does not call a computer offline while this page has no connection to know it", async () => {
    known.list = [machine("1", "Desk")];
    const root = document.createElement("div");
    await showHome(root, appWith({ peers: [peer("1", "Desk", false, 1)], link: "down" }));
    expect(root.querySelector(".machine-state")?.textContent).toBe("Waiting for the connection");
  });

  it("draws the last render only, whichever read of the machines ends first", async () => {
    known.list = [machine("1", "Desk")];
    const root = document.createElement("div");
    const app = appWith({ peers: [peer("1", "Desk", false, 1)] });
    const first = showHome(root, app);
    app.link = "down";
    await Promise.all([first, showHome(root, app)]);
    expect(root.querySelector(".machine-state")?.textContent).toBe("Waiting for the connection");
  });
});

describe("the computer page", () => {
  const invited = (expires: number): App => appWith({
    sessions: [["m:1", open("Desk")]],
    fleet: [["m:1", fleet(agent("a", "web", "working"), agent("b", "web", "needs"))]],
    invites: [["m:1", { type: "pair.code", code: "AB3-DE", expires }]],
  });

  it("lists what needs you first in its project", () => {
    const root = document.createElement("div");
    showMachine(root, appWith({ sessions: [["m:1", open("Desk")]], fleet: [["m:1", fleet(agent("a", "web", "working"), agent("b", "web", "needs"))]] }), "1");
    expect(root.querySelector("h1")?.textContent).toBe("Desk");
    expect(texts(root, ".agent-name")).toEqual(["agent-b", "agent-a"]);
    expect(texts(root, ".agent-status")).toEqual(["Needs you", "Working"]);
  });

  it("shows the code of a new device as text and as a QR of the pair link, drawn once", async () => {
    const app = invited(Date.now() + 600_000);
    const root = document.createElement("div");
    showMachine(root, app, "1");
    expect(root.querySelector(".invite-code")?.textContent).toBe("AB3-DE");
    const canvas = root.querySelector("canvas")!;
    await vi.waitFor(() => expect(qr.toCanvas).toHaveBeenCalledTimes(1));
    expect(qr.toCanvas.mock.calls[0].slice(0, 2)).toEqual([canvas, `${location.origin}/pair#1.AB3-DE`]);

    // A fleet update draws the page again: the same code stays the same picture.
    showMachine(root, app, "1");
    expect(root.querySelector("canvas")).toBe(canvas);
    await Promise.resolve();
    expect(qr.toCanvas).toHaveBeenCalledTimes(1);
  });

  it("drops a code that has run out", () => {
    const root = document.createElement("div");
    showMachine(root, invited(Date.now() - 1), "1");
    expect(root.querySelector("canvas")).toBeNull();
    expect(root.textContent).not.toContain("AB3-DE");
  });

  it("keeps the keyboard where it was when the page draws again", () => {
    const app = appWith({ sessions: [["m:1", open("Desk")]], fleet: [["m:1", fleet(agent("a", "web", "working"))]] });
    const root = document.createElement("div");
    document.body.append(root);
    showMachine(root, app, "1");
    root.querySelector<HTMLAnchorElement>('a[href="#/m/1/a/a"]')!.focus();
    showMachine(root, app, "1");
    expect(document.activeElement?.getAttribute("href")).toBe("#/m/1/a/a");
    root.remove();
  });
});
