import { beforeEach, describe, expect, it, vi } from "vitest";

const eng = vi.hoisted(() => ({ view: null as unknown, messagePerson: vi.fn(async () => {}) }));
vi.mock("../../boardfile", () => ({ readBoardFile: async () => null }));
vi.mock("../../agentbridge", () => ({ revealPane: () => true }));
vi.mock("../../appstate", () => ({ activeWs: null, workspaces: new Map() }));
vi.mock("../../tasks", () => ({ onTasksChange: () => () => {}, taskOf: () => undefined }));
vi.mock("../engine", () => ({
  companyView: () => eng.view, messagePerson: eng.messagePerson,
  giveJob() {}, approve() {}, reject() {}, raiseCap() {}, finishJob() {}, stopJob() {}, dismissAlert() {}, wakePerson() {},
}));

import { liveSource } from "./live";

const person = (id: string, rank: string, floor: string) => ({ id, name: id, rank, floor });
const company = (people: unknown[]) => ({ company: { floors: [{ id: "a", dir: "D:/a", name: "alpha" }], people } });

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
