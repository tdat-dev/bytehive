// src/company/office/office.test.ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { OfficeModel, OfficePerson } from "./model";
import type { SceneOptions } from "./scene";

const io = vi.hoisted(() => ({
  used: false,
  probe: { ok: true, renderer: "ANGLE (Intel)" },
  sceneLoads: 0,
  sceneThrows: false,
  notes: [] as string[],
  elsewhere: null as string | null,
  pane: null as string | null,
  model: null as unknown,
  options: null as unknown,
  /** What the app was asked to do, in order. */
  calls: [] as string[],
  engineChanged: null as (() => void) | null,
  liveThrows: false,
  anchors: [] as unknown[],
  moved: null as (() => void) | null,
  sourceChanged: null as (() => void) | null,
  snapshotThrows: 0,
  snapshotFailAt: 0,
  snapshots: 0,
  stageFails: false,
  revealFails: false,
  subscribers: 0,
}));
vi.mock("./live", () => ({
  liveSource: () => {
    if (io.liveThrows) throw new Error("no source");
    return mockSource();
  },
}));
vi.mock("./scene", () => {
  io.sceneLoads += 1;
  return {
    createScene: (_canvas: unknown, options: unknown) => {
      if (io.sceneThrows) throw new Error("no context");
      if (!io.probe.ok) throw new Error(`No usable WebGL (${io.probe.renderer || "no context"}).`);
      io.options = options;
      return { setModel() {}, setLevel() {}, resize() {}, setVisible() {}, pick: () => null, anchors: () => io.anchors, onMoved: (fn: () => void) => { io.moved = fn; return () => {}; }, stats: () => ({}), dispose() {} };
    },
  };
});
vi.mock("../../hint", () => ({ topNote: (t: string) => io.notes.push(t) }));
vi.mock("../../inbox", () => ({
  stageAgent: (id: string, focus = true) => {
    if (io.stageFails) return false;
    io.calls.push(focus ? `stage ${id}` : `stage quietly ${id}`);
    return true;
  },
  openChanges: (id: string) => { io.pane = id; io.calls.push(`changes ${id}`); },
  closeChanges: () => { io.pane = null; io.calls.push("close changes"); },
  changesOpenFor: () => io.pane,
  refitStage: () => { io.calls.push("refit"); },
  focusStage: () => { io.calls.push("focus stage"); },
}));
vi.mock("../../dock", () => ({ dockIsOpen: () => false }));
vi.mock("../engine", () => ({
  companyUsedNow: () => io.used,
  companyElsewhere: () => io.elsewhere,
  onCompanyChange: (fn: () => void) => { io.engineChanged = fn; return () => {}; },
}));

const EMPTY: OfficeModel = { founder: null, floors: [], needs: [], jobs: [], flights: [], counts: { people: 0, working: 0, needs: 0 } };

function mockSource() {
  return {
    snapshot: () => {
      if (io.snapshotThrows > 0) { io.snapshotThrows -= 1; throw new Error("no snapshot"); }
      io.snapshots += 1;
      if (io.snapshots === io.snapshotFailAt) throw new Error("no snapshot");
      return { model: (io.model as OfficeModel | null) ?? EMPTY, note: null, error: null };
    },
    onChange: (fn: () => void) => { io.sourceChanged = fn; io.subscribers += 1; return () => { io.subscribers -= 1; }; },
    watching() {},
    showPerson: (id: string, focus = true) => { io.calls.push(`reveal ${id} ${focus}`); return !io.revealFails; },
  };
}

const person = (id: string, paneId: string | null): OfficePerson => ({
  id, name: id, rank: "staff", title: "Engineer", department: "engineering", cli: "claude", shirt: "#000", hair: "#000", prop: "", state: "working", doing: null, floorId: "f1", seat: 0, paneId, wsId: paneId ? "w1" : null,
} as OfficePerson);

const withPeople = (): OfficeModel => ({
  ...EMPTY,
  floors: [{ id: "f1", name: "alpha", dir: "D:/alpha", open: true, lit: true, people: [person("hai", "pane-hai"), person("minh", null)], lead: "hai", needs: 0, papers: 0, board: { todo: 1, doing: 0, done: 0 } }],
});

const APP = '<div id="home"><button id="start">Start</button></div><div id="app"><header class="topbar"><div class="tb-center"></div><button id="under">Under</button></header><aside class="dock-panel"></aside></div>';

/** Each test's module keeps its listener on the document: close it, or it answers the next test's keys. */
const opened: Array<typeof import("./office")> = [];

async function fresh() {
  vi.resetModules();
  document.body.className = "";
  document.body.innerHTML = APP;
  localStorage.clear();
  const showApp = vi.fn();
  const showHome = vi.fn();
  const openBoard = vi.fn((dir: string) => dir === "D:/alpha");
  const openProject = vi.fn();
  const m = await import("./office");
  m.initOffice({ showApp, showHome, openBoard, openProject });
  opened.push(m);
  return { m, showApp, showHome, openBoard, openProject };
}
/** Dynamic imports take a few turns of the loop, even mocked. */
const settle = async () => { for (let i = 0; i < 20; i += 1) await new Promise((r) => setTimeout(r, 0)); };

const options = () => io.options as SceneOptions;
const inert = (id: string) => !!document.getElementById(id)?.closest("[inert]");
const esc = () => document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));

beforeEach(() => {
  io.used = false; io.probe = { ok: true, renderer: "ANGLE (Intel)" }; io.sceneThrows = false; io.notes = []; io.elsewhere = null; io.sceneLoads = 0;
  io.pane = null; io.model = null; io.options = null; io.calls = []; io.engineChanged = null; io.liveThrows = false; io.anchors = []; io.moved = null; io.snapshotThrows = 0;
  io.snapshotFailAt = 0; io.snapshots = 0; io.stageFails = false; io.revealFails = false; io.subscribers = 0;
});

afterEach(() => {
  vi.useRealTimers();
  for (const m of opened.splice(0)) if (m.officeVisible()) m.officeToggle();
});

describe("someone who never used the Company", () => {
  it("gets no office element and no three.js at start", async () => {
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.getElementById("office")).toBeNull();
    expect(io.sceneLoads).toBe(0);
    expect(m.officeVisible()).toBe(false);
  });
  it("keeps the house icon as it is", async () => {
    const { m } = await fresh();
    expect(m.officeGoHome()).toBe(false);
  });
  it("sees no fallback note", async () => {
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(io.notes).toEqual([]);
    expect(document.body.classList.contains("office-fallback")).toBe(false);
  });
  it("leaves the app alone: nothing is inert", async () => {
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.querySelector("[inert]")).toBeNull();
  });
  it("gets the office, and three.js, from the key", async () => {
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    await settle();
    expect(m.officeVisible()).toBe(true);
    expect(document.getElementById("office")?.dataset.level).toBe("building");
    expect(document.querySelector("#office [data-job-box] textarea")).not.toBeNull();
    expect(io.sceneLoads).toBe(1);
  });
  it("still has the job box from the key when WebGL is missing, as the panel gave", async () => {
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeToggle();
    await settle();
    expect(document.querySelector("#app .of-side [data-send]")).not.toBeNull();
    expect(io.notes).toEqual(["The 3D office could not start, so you are in the Inbox."]);
  });
  it("can put that column away again with the key", async () => {
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeToggle();
    await settle();
    m.officeToggle();
    await settle();
    expect(document.body.classList.contains("office-fallback")).toBe(false);
    expect(document.querySelector(".of-side")).toBeNull();
  });
  it("is not moved when the first Job is given, but the office is home from then on", async () => {
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    io.used = true;
    io.engineChanged?.();
    await settle();
    expect(localStorage.getItem("maestro.office.used")).toBe("1");
    expect(m.officeVisible()).toBe(false);
    expect(m.officeGoHome()).toBe(true);
    await settle();
    expect(m.officeVisible()).toBe(true);
  });
});

describe("once the Company has been used", () => {
  it("makes the office the home screen", async () => {
    io.used = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(true);
    expect(m.officeGoHome()).toBe(true);
  });
  it("knows that before the engine has loaded, from the last run", async () => {
    const first = await fresh();
    localStorage.setItem("maestro.office.used", "1");
    first.m.officeBoot();
    await settle();
    expect(first.m.officeVisible()).toBe(true);
  });
  it("opens behind the splash when the engine says so first, with no key from a last run", async () => {
    const { m } = await fresh();
    document.body.classList.add("boot");
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
    io.used = true;
    io.engineChanged?.();
    await settle();
    expect(m.officeVisible()).toBe(true);
  });
  it("toggles between the office and the Inbox", async () => {
    io.used = true;
    const { m, showHome } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    expect(m.officeVisible()).toBe(false);
    expect(showHome).toHaveBeenCalled();
    m.officeToggle();
    await settle();
    expect(m.officeVisible()).toBe(true);
  });
  it("closes to the app's own view, from the key and from the Inbox button", async () => {
    io.used = true;
    const { m, showHome } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    expect(showHome).toHaveBeenCalledTimes(1);
    m.officeToggle();
    await settle();
    document.querySelector<HTMLElement>("#office [data-leave]")!.click();
    expect(m.officeVisible()).toBe(false);
    expect(showHome).toHaveBeenCalledTimes(2);
  });
  it("opens at the level it was left", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
    m.officeToggle();
    m.officeToggle();
    await settle();
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
  });
  it("goes to the Building from the house icon, from wherever it was left", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    m.officeToggle();
    expect(m.officeGoHome()).toBe(true);
    await settle();
    expect(document.getElementById("office")?.dataset.level).toBe("building");
  });
  it("falls back to the Inbox with Needs you when WebGL is missing", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
    expect(document.body.classList.contains("office-fallback")).toBe(true);
    expect(document.querySelector("#app .of-side [data-needs]")).not.toBeNull();
    expect(io.notes).toEqual(["The 3D office could not start, so you are in the Inbox."]);
  });
  it("treats a software renderer as missing", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "Google SwiftShader" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.body.classList.contains("office-fallback")).toBe(true);
  });
  it("falls back when the scene cannot be made", async () => {
    io.used = true;
    io.sceneThrows = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.body.classList.contains("office-fallback")).toBe(true);
  });
  it("tries the scene again on the key, from the fallback", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    io.probe = { ok: true, renderer: "ANGLE (Intel)" };
    m.officeToggle();
    await settle();
    expect(m.officeVisible()).toBe(true);
    expect(document.body.classList.contains("office-fallback")).toBe(false);
  });
  it("says so once a run, however often the scene fails again", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    await settle();
    expect(document.body.classList.contains("office-fallback")).toBe(true);
    expect(document.querySelectorAll(".of-side").length).toBe(1);
    expect(io.notes.length).toBe(1);
  });
  it("does not take the house icon from the fallback", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeGoHome()).toBe(false);
  });
  it("sends a fallback row to the agent, and to its changes for a merge", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const row = (kind: string, key: string) => ({ key, kind, title: key, detail: "", jobId: "j1", personId: "hai", standing: false });
    io.model = { ...withPeople(), needs: [row("question", "ask"), row("merge", "done")] };
    const { m, showApp } = await fresh();
    m.officeBoot();
    await settle();
    document.querySelector<HTMLElement>('.of-side [data-row="ask"]')!.click();
    expect(io.calls).toContain("stage pane-hai");
    expect(showApp).toHaveBeenCalled();
    document.querySelector<HTMLElement>('.of-side [data-row="done"]')!.click();
    expect(io.calls).toContain("changes pane-hai");
  });
  it("says who is not running when a fallback row has no pane", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    io.model = { ...withPeople(), needs: [{ key: "ask", kind: "question", title: "ask", detail: "", jobId: "j1", personId: "minh", standing: false }] };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    document.querySelector<HTMLElement>('.of-side [data-row="ask"]')!.click();
    expect(io.notes[io.notes.length - 1]).toBe("minh is not running.");
  });
  it("falls back when the shell cannot be made", async () => {
    io.used = true;
    io.snapshotThrows = 1;
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
    expect(document.body.classList.contains("office-fallback")).toBe(true);
    errors.mockRestore();
  });
  it("gets nothing worse than the Inbox when even the fallback cannot be drawn", async () => {
    io.used = true;
    io.snapshotThrows = 9;
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
    expect(document.body.classList.contains("office-fallback")).toBe(false);
    expect(document.querySelector(".of-side")).toBeNull();
    errors.mockRestore();
  });
  it("leaves nothing listening when the fallback fails half way", async () => {
    io.used = true;
    io.snapshotThrows = 1; // the shell's rail, so no shell stays to hold a subscription
    io.snapshotFailAt = 2; // the fallback's box draws, then its rail cannot
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.querySelector(".of-side")).toBeNull();
    expect(document.body.classList.contains("office-fallback")).toBe(false);
    expect(io.subscribers).toBe(0);
    errors.mockRestore();
  });
  it("says where the rail is when no project is open: it is in the Inbox, and Start shows", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    document.getElementById("app")!.hidden = true;
    m.officeBoot();
    await settle();
    expect(io.notes).toEqual(["The 3D office could not start. Open a project to see the Company."]);
  });
  it("falls back when the source cannot load", async () => {
    io.used = true;
    io.liveThrows = true;
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
    expect(errors).toHaveBeenCalled();
    errors.mockRestore();
  });
});

describe("what sits under the layer", () => {
  it("is out of reach at Building and Floor, and back at a Desk and in the Inbox", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(inert("under")).toBe(true);
    expect(inert("start")).toBe(true);
    m.officeReveal("pane-hai");
    expect(inert("under")).toBe(false);
    m.officeToggle();
    expect(inert("under")).toBe(false);
    expect(inert("start")).toBe(false);
  });
  it("leaves the dock's panel alone: it stands over the office", async () => {
    io.used = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.querySelector(".dock-panel")?.closest("[inert]")).toBeNull();
  });
  it("closes Changes on the way up, or it would stand over the office out of reach", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    io.calls = [];
    document.querySelector<HTMLElement>('#office [data-crumb="0"]')!.click();
    expect(io.calls).toContain("close changes");
  });
  it("is given back when the scene fails", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(document.querySelector("[inert]")).toBeNull();
  });
});

describe("at a Desk", () => {
  async function atDesk() {
    io.used = true;
    io.model = withPeople();
    const t = await fresh();
    t.m.officeBoot();
    await settle();
    t.m.officeReveal("pane-hai");
    return t;
  }

  it("puts the person's pane on the stage, with the app showing", async () => {
    const { showApp } = await atDesk();
    expect(io.calls).toContain("stage pane-hai");
    expect(showApp).toHaveBeenCalled();
  });
  it("fits the terminal when the column settles, with nobody moved and the keyboard where it was", async () => {
    await atDesk();
    io.calls = [];
    const office = document.getElementById("office")!;
    const end = new Event("transitionend");
    Object.defineProperty(end, "propertyName", { value: "width" });
    office.dispatchEvent(end);
    expect(io.calls).toEqual(["refit"]);
  });
  it("moves nobody when asked again for the same pane and tab: a floor change lands here", async () => {
    await atDesk();
    const moved = withPeople();
    moved.floors[0].people = moved.floors[0].people.map((p) => ({ ...p, floorId: "f2" }));
    moved.floors.push({ ...moved.floors[0], id: "f2", name: "beta" });
    io.model = moved;
    io.calls = [];
    io.sourceChanged?.();
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
    expect(io.calls.filter((c) => c.startsWith("stage"))).toEqual([]);
  });
  it("gives the terminal no keyboard when the Report covers it", async () => {
    const { m } = await atDesk();
    document.querySelector<HTMLElement>('[data-tab="report"]')!.click();
    m.officeToggle();
    io.calls = [];
    m.officeToggle();
    await settle();
    expect(io.calls).toContain("stage quietly pane-hai");
    expect(io.calls).not.toContain("stage pane-hai");
  });
  it("gives it none behind the Changes drawer either", async () => {
    io.used = true;
    io.model = { ...withPeople(), needs: [{ key: "done", kind: "merge", title: "done", detail: "", jobId: "j1", personId: "hai", standing: false }] };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    document.querySelector<HTMLElement>('#office [data-row="done"]')!.click();
    expect(io.calls).toContain("stage quietly pane-hai");
    expect(io.calls).toContain("changes pane-hai");
  });
  it("lets a terminal the owner went to take it", async () => {
    await atDesk();
    expect(io.calls).toContain("stage pane-hai");
  });
  it("falls back to a reveal that takes no keyboard, when an agent has no task yet", async () => {
    const { m } = await atDesk();
    document.querySelector<HTMLElement>('[data-tab="report"]')!.click();
    m.officeToggle();
    io.stageFails = true;
    io.calls = [];
    m.officeToggle();
    await settle();
    expect(io.calls).toContain("reveal hai false");
    expect(io.calls).not.toContain("reveal hai true");
  });
  it("lets the owner's own arrival take it through that fallback", async () => {
    io.used = true;
    io.model = withPeople();
    io.stageFails = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    expect(io.calls).toContain("reveal hai true");
  });
  it("tries again when neither could: the Desk must not keep another agent's stage", async () => {
    io.used = true;
    io.model = withPeople();
    io.stageFails = true;
    io.revealFails = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    expect(document.querySelector<HTMLElement>("#office [data-wake-card]")!.hidden).toBe(false);
    io.stageFails = false;
    io.revealFails = false;
    io.calls = [];
    io.sourceChanged?.();
    expect(io.calls).toContain("stage quietly pane-hai");
    expect(document.querySelector<HTMLElement>("#office [data-wake-card]")!.hidden).toBe(true);
  });
  it("closes Changes on every level change, not only on the way up", async () => {
    await atDesk();
    document.querySelector<HTMLElement>('[data-tab="changes"]')!.click();
    io.calls = [];
    document.querySelector<HTMLElement>('[data-tab="terminal"]')!.click();
    expect(io.calls).toContain("close changes");
  });
  it("opens Changes for that pane, and closes it on the other tabs", async () => {
    await atDesk();
    document.querySelector<HTMLElement>('[data-tab="changes"]')!.click();
    expect(io.calls).toContain("changes pane-hai");
    document.querySelector<HTMLElement>('[data-tab="report"]')!.click();
    expect(io.pane).toBeNull();
  });
  it("takes the terminal back from the Terminal tab", async () => {
    await atDesk();
    document.querySelector<HTMLElement>('[data-tab="report"]')!.click();
    io.calls = [];
    document.querySelector<HTMLElement>('[data-tab="terminal"]')!.click();
    expect(io.calls).toContain("stage pane-hai");
  });
});

describe("the keyboard after closing", () => {
  async function up() {
    io.used = true;
    const t = await fresh();
    t.m.officeBoot();
    await settle();
    io.calls = [];
    return t;
  }

  it("goes back to the agent when it was in the office", async () => {
    const { m } = await up();
    document.querySelector<HTMLElement>("#office [data-leave]")!.focus();
    m.officeToggle();
    expect(io.calls).toContain("focus stage");
  });
  it("goes back to the agent when nothing had it", async () => {
    const { m } = await up();
    (document.activeElement as HTMLElement | null)?.blur();
    m.officeToggle();
    expect(io.calls).toContain("focus stage");
  });
  it("stays where the owner put it", async () => {
    const { m } = await up();
    const box = document.createElement("input");
    document.querySelector(".dock-panel")!.append(box);
    box.focus();
    m.officeToggle();
    expect(io.calls).not.toContain("focus stage");
    expect(document.activeElement).toBe(box);
  });
  it("comes after the app is back: it cannot land on what is still covered", async () => {
    const { m, showHome } = await up();
    showHome.mockImplementation(() => { io.calls.push("home"); });
    document.querySelector<HTMLElement>("#office [data-leave]")!.focus();
    m.officeToggle();
    expect(io.calls.indexOf("home")).toBeLessThan(io.calls.indexOf("focus stage"));
  });
});

describe("leaving a Desk with Changes open", () => {
  async function inChanges() {
    io.used = true;
    io.model = withPeople();
    const t = await fresh();
    t.m.officeBoot();
    await settle();
    t.m.officeReveal("pane-hai");
    document.querySelector<HTMLElement>('[data-tab="changes"]')!.click();
    io.calls = [];
    return t;
  }

  it("keeps the drawer from the key: a comment being typed lives there", async () => {
    const { m } = await inChanges();
    document.querySelector<HTMLElement>('[data-tab="changes"]')!.focus();
    m.officeToggle();
    expect(io.calls).not.toContain("close changes");
    expect(io.pane).toBe("pane-hai");
    expect(io.calls).toContain("focus stage");
  });
  it("keeps it from the Inbox button", async () => {
    await inChanges();
    const leave = document.querySelector<HTMLElement>("#office [data-leave]")!;
    leave.focus();
    leave.click();
    expect(io.calls).not.toContain("close changes");
    expect(io.pane).toBe("pane-hai");
  });
  it("comes back to the same drawer, with no second one", async () => {
    const { m } = await inChanges();
    m.officeToggle();
    m.officeToggle();
    await settle();
    expect(document.querySelector('.of-tab[aria-selected="true"]')?.textContent).toBe("Changes");
    expect(io.pane).toBe("pane-hai");
  });
});

describe("a new project", () => {
  it("can be opened from the office, or nothing could be once Start is out of reach", async () => {
    io.used = true;
    const { m, openProject } = await fresh();
    m.officeBoot();
    await settle();
    document.querySelector<HTMLElement>("#office [data-open-project]")!.click();
    expect(openProject).toHaveBeenCalledTimes(1);
  });
});

describe("Ctrl+K and notifications", () => {
  it("does nothing from the Inbox", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    m.officeReveal("pane-hai");
    expect(m.officeVisible()).toBe(false);
  });
  it("goes to a member's Desk from the office", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    expect(m.officeVisible()).toBe(true);
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
  });
  it("closes the office for anyone else, so the Inbox can show them", async () => {
    io.used = true;
    io.model = withPeople();
    const { m, showHome } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-stranger");
    expect(m.officeVisible()).toBe(false);
    expect(showHome).toHaveBeenCalled();
  });
});

describe("with the Company in another app", () => {
  it("stays on the Inbox at start", async () => {
    io.used = true;
    io.elsewhere = "The Company is running in another ByteHive window.";
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    expect(m.officeVisible()).toBe(false);
  });
  it("leaves the house icon to the Inbox", async () => {
    io.used = true;
    io.elsewhere = "The Company is running in another ByteHive window.";
    const { m } = await fresh();
    expect(m.officeGoHome()).toBe(false);
  });
  it("still opens the layer from the key", async () => {
    io.used = true;
    io.elsewhere = "The Company is running in another ByteHive window.";
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    await settle();
    expect(m.officeVisible()).toBe(true);
  });
  it("closes an office that boot opened, once the lock has been read", async () => {
    io.used = true;
    const { m, showHome } = await fresh();
    m.officeBoot();
    await settle();
    io.elsewhere = "The Company is running in another ByteHive window.";
    io.engineChanged?.();
    expect(m.officeVisible()).toBe(false);
    expect(showHome).toHaveBeenCalled();
  });
  it("leaves one the owner opened with the key", async () => {
    io.used = true;
    const { m } = await fresh();
    m.officeToggle();
    await settle();
    io.elsewhere = "The Company is running in another ByteHive window.";
    io.engineChanged?.();
    expect(m.officeVisible()).toBe(true);
  });
  it("takes the fallback down too, when boot put it up", async () => {
    io.used = true;
    io.probe = { ok: false, renderer: "" };
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    io.elsewhere = "The Company is running in another ByteHive window.";
    io.engineChanged?.();
    expect(document.body.classList.contains("office-fallback")).toBe(false);
  });
});

describe("a detached window", () => {
  it("has no office: the key says so, and the house icon stays the app's", async () => {
    vi.resetModules();
    document.body.innerHTML = APP;
    localStorage.setItem("maestro.office.used", "1");
    const m = await import("./office");
    m.officeToggle();
    await settle();
    expect(io.notes).toEqual(["The Company runs in the main window."]);
    expect(document.getElementById("office")).toBeNull();
    expect(m.officeGoHome()).toBe(false);
    expect(m.officeVisible()).toBe(false);
  });
});

describe("a lost context", () => {
  async function running() {
    io.used = true;
    const t = await fresh();
    t.m.officeBoot();
    await settle();
    vi.useFakeTimers();
    return t;
  }

  it("falls back after two seconds", async () => {
    const { m } = await running();
    options().onLost?.();
    vi.advanceTimersByTime(1999);
    expect(m.officeVisible()).toBe(true);
    vi.advanceTimersByTime(1);
    vi.useRealTimers();
    await settle();
    expect(m.officeVisible()).toBe(false);
    expect(document.body.classList.contains("office-fallback")).toBe(true);
  });
  it("changes nothing when it is back in time", async () => {
    const { m } = await running();
    options().onLost?.();
    vi.advanceTimersByTime(1500);
    options().onRestored?.();
    vi.advanceTimersByTime(5000);
    vi.useRealTimers();
    await settle();
    expect(m.officeVisible()).toBe(true);
    expect(document.body.classList.contains("office-fallback")).toBe(false);
    expect(io.notes).toEqual([]);
  });
  it("does not interrupt the Inbox: the next open makes a new scene", async () => {
    const { m } = await running();
    m.officeToggle();
    options().onLost?.();
    vi.advanceTimersByTime(2000);
    vi.useRealTimers();
    await settle();
    expect(document.body.classList.contains("office-fallback")).toBe(false);
    expect(io.notes).toEqual([]);
    io.options = null;
    m.officeToggle();
    await settle();
    expect(m.officeVisible()).toBe(true);
    expect(io.options).not.toBeNull();
  });
});

describe("the board on the wall", () => {
  async function pressBoard(open: boolean) {
    io.used = true;
    const model = withPeople();
    model.floors[0].dir = open ? "D:/alpha" : "D:/shut";
    io.model = model;
    io.anchors = [{ kind: "floor", id: "f1", x: 10, y: 10, visible: true }, { kind: "board", id: "f1", x: 40, y: 40, visible: true }];
    const t = await fresh();
    t.m.officeBoot();
    await settle();
    document.querySelector<HTMLElement>('#office [data-tag="f1"]')!.click();
    io.moved?.();
    document.querySelector<HTMLElement>('#office [data-board="f1"]')!.click();
    return t;
  }

  it("opens the Board on that project", async () => {
    const { openBoard } = await pressBoard(true);
    expect(openBoard).toHaveBeenCalledWith("D:/alpha");
    expect(io.notes).toEqual([]);
  });
  it("says so when the project is closed", async () => {
    await pressBoard(false);
    expect(io.notes).toEqual(["alpha is closed. Open it to see its board."]);
  });
});

describe("Esc", () => {
  it("goes up a level, and is kept from the app when it did", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    expect(esc()).toBe(false);
    expect(document.getElementById("office")?.dataset.level).toBe("floor");
  });
  it("stays with a terminal that has the keyboard", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    const term = document.createElement("textarea");
    const wrap = document.createElement("div");
    wrap.className = "xterm";
    wrap.append(term);
    document.getElementById("app")!.append(wrap);
    term.focus();
    expect(esc()).toBe(true);
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
  });
  it("stays with the Changes drawer while it is up", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    const drawer = document.createElement("aside");
    drawer.className = "inbox-review";
    document.getElementById("app")!.append(drawer);
    expect(esc()).toBe(true);
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
  });
  it.each([["a context menu", "cm-menu"], ["a drop-down", "sm-pop"]])("stays with %s while it is up", async (_what, cls) => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    const menu = document.createElement("div");
    menu.className = cls;
    document.body.append(menu);
    expect(esc()).toBe(true);
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
  });
  it("stays with the editor while it has the keyboard", async () => {
    io.used = true;
    io.model = withPeople();
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeReveal("pane-hai");
    const editor = document.createElement("div");
    editor.className = "cm-editor";
    editor.innerHTML = '<div class="cm-content" contenteditable="true" tabindex="0"></div>';
    document.getElementById("app")!.append(editor);
    editor.querySelector<HTMLElement>(".cm-content")!.focus();
    expect(esc()).toBe(true);
    expect(document.getElementById("office")?.dataset.level).toBe("desk");
  });
  it("is not touched while the office is away", async () => {
    io.used = true;
    const { m } = await fresh();
    m.officeBoot();
    await settle();
    m.officeToggle();
    expect(esc()).toBe(true);
  });
});
