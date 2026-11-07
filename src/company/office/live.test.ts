import { beforeEach, describe, expect, it, vi } from "vitest";

const eng = vi.hoisted(() => ({ view: null as unknown, messagePerson: vi.fn(async () => {}), reveal: [] as unknown[][], workspaces: new Map<string, unknown>() }));
vi.mock("../../boardfile", () => ({ readBoardFile: async () => null }));
vi.mock("../../agentbridge", () => ({ revealPane: (...a: unknown[]) => { eng.reveal.push(a); return true; } }));
vi.mock("../../appstate", () => ({ activeWs: null, workspaces: eng.workspaces }));
vi.mock("../../tasks", () => ({ onTasksChange: () => () => {}, taskOf: () => undefined }));
vi.mock("../engine", () => ({
  companyView: () => eng.view, messagePerson: eng.messagePerson,
  giveJob() {}, approve() {}, reject() {}, raiseCap() {}, finishJob() {}, stopJob() {}, dismissAlert() {}, wakePerson() {},
}));

import { liveSource } from "./live";

const person = (id: string, rank: string, floor: string) => ({ id, name: id, rank, floor });
const company = (people: unknown[]) => ({ company: { floors: [{ id: "a", dir: "D:/a", name: "alpha" }], people } });

describe("showPerson", () => {
  beforeEach(() => {
    eng.reveal = [];
    eng.workspaces.clear();
    eng.workspaces.set("w", { id: "w", panes: new Map([["p", { id: "p", spec: { personId: "hai" } }]]) });
  });

  it("takes the keyboard unless told not to", () => {
    expect(liveSource().showPerson("hai")).toBe(true);
    expect(liveSource().showPerson("hai", false)).toBe(true);
    expect(eng.reveal).toEqual([["w", "p", true], ["w", "p", false]]);
  });
  it("says false for someone with no pane", () => {
    expect(liveSource().showPerson("nobody", false)).toBe(false);
  });
});

describe("messageFloor", () => {
  beforeEach(() => { eng.messagePerson.mockClear(); });

  it("goes to the Lead", async () => {
    eng.view = company([person("sol", "staff", "a"), person("hai", "lead", "a")]);
    await liveSource().messageFloor("a", "Hello");
    expect(eng.messagePerson).toHaveBeenCalledWith("hai", "Hello");
  });
  it("says so when the floor has nobody to receive it", async () => {
    eng.view = company([]);
    await expect(liveSource().messageFloor("a", "Hello")).rejects.toThrow("Nobody on alpha");
    expect(eng.messagePerson).not.toHaveBeenCalled();
  });
  it("says so when the Company is not running here", async () => {
    eng.view = null;
    await expect(liveSource().messageFloor("a", "Hello")).rejects.toThrow("Nobody on this floor");
  });
});
