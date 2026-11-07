// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";

const notes = vi.hoisted(() => [] as string[]);
vi.mock("./hint", () => ({ topNote: (t: string) => notes.push(t) }));
vi.mock("./appstate", () => ({ activeWs: null, workspaces: new Map() }));
vi.mock("./spawnmodal", () => ({ presetAvailable: () => false, loadCrew: () => ({}), spawnAgents: async () => [] }));

import { openNewAgent } from "./inboxnew";

describe("New agent with no project open", () => {
  beforeEach(() => { notes.length = 0; document.body.innerHTML = ""; });

  it("points at the start screen, where the owner is", () => {
    openNewAgent();
    expect(notes).toEqual(["Open a project first: pick a folder here"]);
  });
  it("points at the office's own button from the office: Home leads nowhere there", () => {
    document.body.innerHTML = '<div id="office"></div>';
    openNewAgent();
    expect(notes).toEqual(["Open a project first: <b>Open project</b>, at the top of the office"]);
  });
  it("does not count an office that is away", () => {
    document.body.innerHTML = '<div id="office" hidden></div>';
    openNewAgent();
    expect(notes).toEqual(["Open a project first: pick a folder here"]);
  });
});
