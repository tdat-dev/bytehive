import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Peer } from "../../../src/remote/protocol";
import type { App } from "../main";
import { leavePair, parsePairLink, showPair } from "./pair";

const scanner = vi.hoisted(() => ({ start: vi.fn(async () => {}), destroy: vi.fn(), seen: null as ((r: { data: string }) => void) | null }));
vi.mock("qr-scanner", () => ({
  default: class {
    constructor(_video: HTMLVideoElement, onScan: (r: { data: string }) => void) { scanner.seen = onScan; }
    start = scanner.start;
    destroy = scanner.destroy;
  },
}));

const peer = (id: string, name: string, online = true): Peer => ({ id, name, online, lastSeen: 1 });
const appWith = (peers: Peer[]) => ({ relay: { peers }, pair: vi.fn(), pairNote: "" }) as unknown as App;
const options = (root: HTMLElement) => [...root.querySelectorAll("option")].map((o) => o.textContent);

it("reads a link and a bare value, and refuses anything else", () => {
  expect(parsePairLink("https://app.x/pair#abc.DEF-1")).toEqual({ machine: "abc", code: "DEF-1" });
  expect(parsePairLink("abc.DEF")).toEqual({ machine: "abc", code: "DEF" });
  expect(parsePairLink("nonsense")).toBeNull();
});

describe("the pair page", () => {
  beforeEach(() => { scanner.start.mockClear(); scanner.destroy.mockClear(); });
  afterEach(() => leavePair());

  it("keeps a half-typed code and the chosen computer through a peers message, and lists who came online", () => {
    const app = appWith([peer("m:1", "Desk"), peer("m:2", "Laptop")]);
    const root = document.createElement("div");
    showPair(root, app);
    const code = root.querySelector<HTMLInputElement>("#c")!;
    code.value = "AB3";
    root.querySelector<HTMLSelectElement>("#m")!.value = "2";

    app.relay.peers = [peer("m:2", "Laptop"), peer("m:1", "Desk"), peer("m:3", "Studio")];
    showPair(root, app);

    expect(root.querySelector("#c")).toBe(code);
    expect(code.value).toBe("AB3");
    expect(root.querySelector<HTMLSelectElement>("#m")!.value).toBe("2");
    expect(options(root)).toContain("Studio");
  });

  it("says what to do when no computer is online, and shows the form when one comes", () => {
    const app = appWith([peer("m:1", "Desk", false)]);
    const root = document.createElement("div");
    showPair(root, app);
    expect(root.textContent).toContain("No computer is online");
    app.relay.peers = [peer("m:1", "Desk")];
    showPair(root, app);
    expect(root.textContent).not.toContain("No computer is online");
    expect(options(root)).toEqual(["Desk"]);
  });

  it("does not stop a running scan for a peers message, and stops the camera on leaving", async () => {
    const app = appWith([peer("m:1", "Desk")]);
    const root = document.createElement("div");
    showPair(root, app);
    root.querySelector<HTMLButtonElement>("#scan")!.click();
    await vi.waitFor(() => expect(scanner.start).toHaveBeenCalled());

    showPair(root, app);
    expect(scanner.destroy).not.toHaveBeenCalled();
    expect(root.querySelector<HTMLVideoElement>("#v")!.hidden).toBe(false);

    leavePair();
    expect(scanner.destroy).toHaveBeenCalledTimes(1);
  });

  it("stops the camera when leaving during the wait for it", async () => {
    let allow = () => {};
    scanner.start.mockImplementationOnce(() => new Promise<void>((done) => { allow = done; }));
    const root = document.createElement("div");
    showPair(root, appWith([peer("m:1", "Desk")]));
    root.querySelector<HTMLButtonElement>("#scan")!.click();
    await vi.waitFor(() => expect(scanner.start).toHaveBeenCalled());
    leavePair();
    allow();
    await vi.waitFor(() => expect(scanner.destroy).toHaveBeenCalledTimes(1));
  });

  it("pairs from a scanned code and stops the camera", async () => {
    const app = appWith([]);
    const root = document.createElement("div");
    showPair(root, app);
    root.querySelector<HTMLButtonElement>("#scan")!.click();
    await vi.waitFor(() => expect(scanner.start).toHaveBeenCalled());
    scanner.seen!({ data: "https://app.x/pair#abc.DEF-1" });
    expect(scanner.destroy).toHaveBeenCalledTimes(1);
    expect(app.pair).toHaveBeenCalledWith("abc", "DEF-1");
  });

  it("says so when the camera can't start", async () => {
    scanner.start.mockRejectedValueOnce(new Error("denied"));
    const root = document.createElement("div");
    showPair(root, appWith([peer("m:1", "Desk")]));
    root.querySelector<HTMLButtonElement>("#scan")!.click();
    await vi.waitFor(() => expect(root.textContent).toContain("didn't let the page use the camera"));
    expect(root.querySelector<HTMLVideoElement>("#v")!.hidden).toBe(true);
  });

  it("pairs once for two presses, and takes a press again after a refused code, with the code still there", () => {
    const app = appWith([peer("m:1", "Desk")]);
    const root = document.createElement("div");
    showPair(root, app);
    root.querySelector<HTMLInputElement>("#c")!.value = " AB3 ";
    const press = () => root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    press();
    press();
    expect(app.pair).toHaveBeenCalledTimes(1);
    expect(app.pair).toHaveBeenCalledWith("1", "AB3");
    expect(root.querySelector<HTMLButtonElement>("#go")!.disabled).toBe(true);

    app.pairNote = "That code didn't work. Ask for a new one.";
    showPair(root, app);
    expect(root.textContent).toContain("That code didn't work.");
    expect(root.querySelector<HTMLInputElement>("#c")!.value).toBe(" AB3 ");
    expect(root.querySelector<HTMLButtonElement>("#go")!.disabled).toBe(false);
    press();
    expect(app.pair).toHaveBeenCalledTimes(2);
  });
});
