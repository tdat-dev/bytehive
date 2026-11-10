import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Task } from "./tasks";

const io = vi.hoisted(() => ({
  tasks: [] as Array<Partial<Task>>,
  notes: [] as string[],
  stage: 0,
  set: [] as Array<string | null>,
  refuse: "",
}));
vi.mock("./tasks", () => ({ allTasks: () => io.tasks }));
vi.mock("./inbox", () => ({ focusStage: () => { io.stage++; } }));
vi.mock("./hint", () => ({ topNote: (s: string) => { io.notes.push(s); } }));
vi.mock("./ipc", () => ({
  // as Tauri hands back a CommandError
  hotkeySet: async (combo: string | null) => { io.set.push(combo); if (io.refuse) throw { Failed: io.refuse }; },
  onHotkey: async () => () => {},
}));

import { applyHotkey, hotkeyFailure, hotkeyPressed } from "./hotkey";
import { workspaces } from "./appstate";
import { setPref } from "./prefs";
import type { Pane, Workspace } from "./panetypes";

const needs = (paneId: string) => ({ paneId, status: { state: "needs", ask: null } }) as Partial<Task>;

function setup(): { term: HTMLTextAreaElement } {
  document.body.innerHTML = `
    <ul><li><button class="iq-row" data-id="a" aria-current="true">Ana</button></li><li><button class="iq-row" data-id="b">Bo</button></li></ul>
    <div class="pane" id="pa"><div class="xterm"><textarea></textarea></div></div>`;
  // happy-dom lays nothing out: anything in the page counts as seen here
  for (const el of document.querySelectorAll<HTMLElement>("button")) el.getClientRects = () => [{}] as unknown as DOMRectList;
  const term = document.querySelector("textarea")!;
  const pane = { id: "a", el: document.getElementById("pa"), term: { focus: () => term.focus() } } as unknown as Pane;
  workspaces.clear();
  workspaces.set("w", { id: "w", panes: new Map([["a", pane]]) } as unknown as Workspace);
  return { term };
}

beforeEach(() => {
  io.tasks = []; io.notes = []; io.stage = 0; io.set = []; io.refuse = "";
  localStorage.clear();
  (document.activeElement as HTMLElement | null)?.blur();
});

describe("the global key", () => {
  it("moves the keyboard to the next one waiting, and the next press hands it back", () => {
    const { term } = setup();
    io.tasks = [needs("b")];
    term.focus();
    hotkeyPressed();
    expect((document.activeElement as HTMLElement).dataset.id).toBe("b");
    hotkeyPressed();
    expect(document.activeElement).toBe(term);
    // and round again
    hotkeyPressed();
    expect((document.activeElement as HTMLElement).dataset.id).toBe("b");
  });

  it("says so when nobody waits, and still takes the keys out of the terminal", () => {
    const { term } = setup();
    term.focus();
    hotkeyPressed();
    expect(io.notes).toEqual(["Nothing is waiting for you"]);
    expect((document.activeElement as HTMLElement).dataset.id).toBe("a");
    hotkeyPressed();
    expect(document.activeElement).toBe(term);
  });

  it("goes to the agent on stage when no terminal had the keys", () => {
    setup();
    io.tasks = [needs("b")];
    hotkeyPressed();
    hotkeyPressed();
    expect(io.stage).toBe(1);
  });

  it("starts over when you have moved on from where it put you", () => {
    const { term } = setup();
    io.tasks = [needs("b")];
    term.focus();
    hotkeyPressed();
    term.focus(); // clicked back in by hand
    hotkeyPressed();
    expect((document.activeElement as HTMLElement).dataset.id).toBe("b");
  });

  it("prefers the office's rail while the office is up", () => {
    setup();
    document.body.insertAdjacentHTML("beforeend", `<div class="of"><ul data-needs><li><button id="row">Plan</button></li></ul></div>`);
    document.getElementById("row")!.getClientRects = () => [{}] as unknown as DOMRectList;
    io.tasks = [needs("b")];
    hotkeyPressed();
    expect(document.activeElement?.id).toBe("row");
  });

  it("finds Needs you in the column the Inbox shows when the office cannot draw", () => {
    setup();
    document.body.insertAdjacentHTML("beforeend", `
      <div class="of" hidden><ul data-needs><li><button id="gone">Review</button></li></ul></div>
      <aside class="of-side"><div class="co-rail"><ul data-needs><li><button id="row">Review</button></li></ul></div></aside>`);
    document.getElementById("gone")!.getClientRects = () => [] as unknown as DOMRectList;
    document.getElementById("row")!.getClientRects = () => [{}] as unknown as DOMRectList;
    hotkeyPressed();
    expect(document.activeElement?.id).toBe("row");
    expect(io.notes).toEqual([]);
  });
});

describe("registering it", () => {
  it("asks for the key in Settings, none when turned off", async () => {
    await applyHotkey();
    setPref("hotkey", "Win+Alt+J");
    await applyHotkey();
    setPref("hotkeyOn", false);
    await applyHotkey();
    expect(io.set).toEqual(["Win+Alt+N", "Win+Alt+J", null]);
  });

  it("keeps Windows' refusal in words, and forgets it once it works", async () => {
    io.refuse = "Windows would not give ByteHive Win+Alt+N: another app or another ByteHive window has it.";
    await applyHotkey();
    expect(hotkeyFailure()).toContain("another app");
    io.refuse = "";
    await applyHotkey();
    expect(hotkeyFailure()).toBe("");
  });
});
