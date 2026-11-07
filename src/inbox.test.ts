// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Task } from "./tasks";
import type { Ask } from "./askparse";

// The inbox reads tasks and drives panes through these modules; fake them so
// the DOM behaviour can be checked without PTYs or Tauri.
const state = vi.hoisted(() => ({
  tasks: [] as Task[],
  answered: [] as Array<[string, string]>,
  texts: [] as Array<[string, string]>,
  focused: [] as string[], quiet: [] as string[],
  listeners: [] as Array<() => void>,
  reviewed: [] as Array<string | null>,
  scoped: [] as Array<[string | null, string[] | null]>,
  markedReviewed: [] as string[],
  settings: 0,
  killed: [] as string[],
  renamed: [] as Array<[string, string]>,
  confirm: { ok: true, value: "" },
  sent: [] as Array<[string, string]>,
  lands: [] as Array<{ wt: string; dryRun: boolean; title: string | null }>,
  land: { dry: null as unknown, real: null as unknown },
  discarded: [] as string[],
  merged: [] as Array<[string, string, string]>,
  diff: "",
}));
vi.mock("./tasks", () => ({
  allTasks: () => state.tasks,
  taskOf: (id: string) => state.tasks.find((t) => t.paneId === id),
  markMerged: (id: string, base: string, branch: string) => { state.merged.push([id, base, branch]); },
  markReviewed: (id: string) => { state.markedReviewed.push(id); },
  changeScope: () => ({ dir: "D:/bytehive", from: "h1", paths: ["src/a.ts"], shared: 0 }),
  onTasksChange: (cb: () => void) => { state.listeners.push(cb); return () => {}; },
  answerOption: async (id: string, o: { key: string }) => { state.answered.push([id, o.key]); },
  answerText: async (id: string, t: string) => { state.texts.push([id, t]); },
}));
vi.mock("./panelayout", () => ({
  focusPane: (_ws: unknown, pane: { id: string; el: HTMLElement }, _ev?: unknown, take = true) => {
    document.querySelectorAll(".focused").forEach((e) => e.classList.remove("focused"));
    pane.el.classList.add("focused");
    pane.el.parentElement?.classList.add("has-focus");
    state.focused.push(pane.id);
    if (!take) state.quiet.push(pane.id);
  },
  renamePane: (pane: { id: string; spec: { name: string } }, name: string) => { pane.spec.name = name; state.renamed.push([pane.id, name]); },
}));
vi.mock("./agentbridge", () => ({ revealPane: () => true }));
vi.mock("./zoom", () => ({ paneFont: (_ws: unknown, bump = 0) => 13 + bump }));
vi.mock("./ipc", () => ({
  resizePty: async () => {},
  killPty: async (id: string) => { state.killed.push(id); },
  sendMessage: async (id: string, text: string) => { state.sent.push([id, text]); },
  reviewBranchDiff: async (wt: string) => { state.reviewed.push(wt); return state.diff; },
  agentDiff: async (dir: string, from: string | null, paths: string[] | null) => { state.reviewed.push(dir); state.scoped.push([from, paths]); return state.diff; },
  reposUnder: async (dir: string) => [{ path: dir, name: "bytehive" }],
  reviewLand: async (wt: string, o: { dryRun?: boolean; title?: string | null }) => {
    state.lands.push({ wt, dryRun: !!o.dryRun, title: o.title ?? null });
    return o.dryRun ? state.land.dry : state.land.real;
  },
  reviewDiscardBranch: async (wt: string) => { state.discarded.push(wt); },
}));
vi.mock("./confirmmodal", () => ({ confirmModal: async () => ({ ok: state.confirm.ok, dontAsk: false, value: state.confirm.value }) }));
vi.mock("./spawnmodal", () => ({ openModal: () => {}, presetAvailable: (p: string) => p === "codex", loadCrew: () => ({ skipPerms: false }), spawnAgents: async () => ["Bob"] }));
vi.mock("./settingsmodal", () => ({ openSettings: () => { state.settings++; } }));
vi.mock("./dock", () => ({ dockToggle: () => {} }));
vi.mock("./finishwatch", () => ({ startFinishWatch: () => {}, stopFinishWatch: () => {} }));
vi.mock("./sessionfollow", () => ({ startSessionFollow: () => {}, stopSessionFollow: () => {} }));
vi.mock("./company/engine", () => ({ companyNeeds: () => 0, onCompanyChange: () => () => {} }));
vi.mock("./company/office/office", () => ({ officeReveal() {}, officeToggle() {} }));

import { workspaces, setActiveWs } from "./appstate";
import { groupTasks, projectGroups, headline, rowLine, lineCounts, ago, togglePin, fillPins, gridOrder, railGroups, shortLabel, isYesNo, initInbox, setInbox, setSplit, stageAgent, openChanges, closeChanges, changesOpenFor, focusStage } from "./inbox";
import type { Workspace, Pane } from "./panetypes";
import { setPref } from "./prefs";
import { sortFleet, needsCount } from "./fleet";

const RUN_ASK: Ask = {
  kind: "run", prompt: "Do you want to proceed?", detail: "npm run build",
  options: [
    { n: 1, label: "Yes", key: "1" },
    { n: 2, label: "Yes, and don't ask again", key: "2", always: true },
    { n: 3, label: "No, and tell Claude what to do differently", key: "\x1b", deny: true },
  ],
};
const DIFF = [
  "diff --git a/src/a.ts b/src/a.ts", "--- a/src/a.ts", "+++ b/src/a.ts",
  "@@ -1,2 +1,2 @@", " keep", "-old", "+new",
].join("\n");
const flush = async () => { for (let i = 0; i < 6; i++) await new Promise((r) => setTimeout(r, 0)); };

const task = (paneId: string, name: string, state: Task["status"]["state"], over: Partial<Task> = {}): Task => ({
  paneId, wsId: "ws-1", name, project: "bytehive", branch: null, title: null, race: null,
  changedFiles: null, added: null, removed: null, since: 0,
  status: { state, ask: state === "needs" ? RUN_ASK : null }, ...over,
});

describe("inbox helpers", () => {
  it("groups by project in a fixed order: projects as opened, agents as made; a state change moves nothing", () => {
    const order = { ws: new Map([["m", 0], ["z", 1], ["a", 2]]), pane: new Map([["m1", 0], ["z1", 1], ["z2", 2], ["a1", 3]]) };
    const list = [
      task("z2", "Zia", "needs", { wsId: "z", project: "zoldify" }),
      task("a1", "Ann", "working", { wsId: "a", project: "app" }),
      task("z1", "Zed", "stopped", { wsId: "z", project: "zoldify" }),
      task("m1", "Mo", "working", { wsId: "m", project: "bytehive" }),
    ];
    const g = projectGroups(list, order);
    expect(g.map((x) => [x.name, x.tasks.map((t) => t.paneId), x.needs])).toEqual([
      ["bytehive", ["m1"], 0], ["zoldify", ["z1", "z2"], 1], ["app", ["a1"], 0],
    ]);
    // Zia answered, Zed started working: same places
    const later = list.map((t) => (t.paneId === "z2" ? { ...t, status: { state: "working" as const, ask: null } } : t.paneId === "z1" ? { ...t, status: { state: "needs" as const, ask: null } } : t));
    expect(projectGroups(later, order).map((x) => x.tasks.map((t) => t.paneId))).toEqual([["m1"], ["z1", "z2"], ["a1"]]);
  });

  it("groups the Grid's cards by project, the terminal's project first", () => {
    const list = [
      task("z1", "Ana", "working", { wsId: "z", project: "zoldify" }),
      task("m1", "Ana", "needs", { wsId: "m", project: "bytehive" }),
      task("a1", "Ana", "working", { wsId: "a", project: "app" }),
      task("z2", "Bob", "idle", { wsId: "z", project: "zoldify" }),
    ];
    expect(railGroups(list, "z").map((g) => [g.project, g.tasks.map((t) => t.paneId)]))
      .toEqual([["zoldify", ["z1", "z2"]], ["app", ["a1"]], ["bytehive", ["m1"]]]);
    expect(railGroups(list, null).map((g) => g.project)).toEqual(["app", "bytehive", "zoldify"]);
  });

  it("groups in queue order and keeps an empty Needs you group", () => {
    const g = groupTasks([task("w", "Eli", "working"), task("r", "Cy", "review")]);
    expect(g.map((x) => [x.state, x.tasks.length])).toEqual([["needs", 0], ["review", 1], ["working", 1]]);
  });

  it("writes the headline as a sentence", () => {
    expect(headline([task("a", "Ana", "needs"), task("b", "Bo", "needs"), task("c", "Cy", "review"), task("e", "Eli", "working")]))
      .toEqual({ lead: "2 agents need you.", rest: " 1 is ready to review, 1 is working." });
    expect(headline([task("e", "Eli", "working")]).lead).toBe("Nothing needs you.");
    expect(headline([]).lead).toBe("No agents yet.");
    // only what is so: no "0 are ..." clauses; with nothing going on, who is idle or stopped
    expect(headline([task("e", "Eli", "working")]).rest).toBe(" 1 is working.");
    expect(headline([task("a", "Ana", "stopped"), task("b", "Bo", "stopped"), task("h", "Hal", "idle")]).rest).toBe(" 1 is waiting for a task, 2 are stopped.");
    expect(headline([task("h", "Hal", "idle", { worked: true }), task("i", "Ivy", "idle")]).rest).toBe(" 1 is done, 1 is waiting for a task.");
  });

  it("counts what the Company needs from you", () => {
    expect(headline([], 2)).toEqual({ lead: "2 Company items need you.", rest: "" });
    expect(headline([], 1).lead).toBe("1 Company item needs you.");
    expect(headline([task("a", "Ana", "needs")], 1).lead).toBe("1 agent and 1 Company item need you.");
    expect(headline([task("e", "Eli", "working")], 1)).toEqual({ lead: "1 Company item needs you.", rest: " 1 is working." });
  });
  it("says what it always said with nothing from the Company", () => {
    expect(headline([]).lead).toBe("No agents yet.");
    expect(headline([], 0).lead).toBe("No agents yet.");
  });

  it("says what each agent wants on its row", () => {
    expect(rowLine(task("a", "Ana", "needs"))).toBe("Wants to run npm run build");
    expect(rowLine(task("c", "Cy", "review", { changedFiles: 3 }))).toBe("3 files changed");
    expect(rowLine(task("c", "Cy", "review", { changedFiles: 1 }))).toBe("1 file changed");
    expect(rowLine(task("h", "Hal", "idle"))).toBe("Waiting for a task");
    // idle after it worked on something is done, not still waiting for a first task
    expect(rowLine(task("h", "Hal", "idle", { worked: true }))).toBe("Done");
    expect(ago(90_000)).toBe("2m");
  });

  it("pins up to the Split size and never pushes out the one on the stage", () => {
    expect(togglePin(["a", "b"], "b")).toEqual(["a"]);
    expect(togglePin(["a", "b", "c", "d"], "e", "a", 4)).toEqual(["a", "c", "d", "e"]);
    expect(togglePin(["a", "b", "c", "d"], "e", "b", 4)).toEqual(["b", "c", "d", "e"]);
    // up to a 3 x 3 grid
    expect(togglePin(["a", "b", "c", "d", "e"], "f")).toEqual(["a", "b", "c", "d", "e", "f"]);
  });

  it("puts a plain permission prompt on one row with short button words", () => {
    expect(isYesNo(RUN_ASK)).toBe(true);
    expect(RUN_ASK.options.map(shortLabel)).toEqual(["Allow", "Always allow here", "Deny"]);
    expect(isYesNo({ kind: "question", options: RUN_ASK.options })).toBe(false);
  });

  it("fills Split from the queue without moving the agents already there", () => {
    expect(fillPins([], ["a", "b", "c", "d", "e"], new Set(), 4)).toEqual(["a", "b", "c", "d"]);
    expect(fillPins([], ["a", "b", "c", "d", "e", "f", "g"], new Set(), 6)).toEqual(["a", "b", "c", "d", "e", "f"]);
    expect(fillPins(["c", "a"], ["a", "b", "c"], new Set())).toEqual(["c", "a", "b"]);
    expect(fillPins(["a", "gone"], ["a", "b", "c"], new Set(["b"]))).toEqual(["a", "c"]);
  });

  it("keeps stopped agents out of the Grid until they are resumed", () => {
    const list = [task("a", "Ana", "working"), task("b", "Bo", "stopped"), task("c", "Cy", "needs"), task("m", "Mo", "merged")];
    expect(gridOrder(list)).toEqual(["a", "c"]);
    // b was in the Grid when it stopped: it leaves, and comes back once it runs again.
    expect(fillPins(["b", "a"], gridOrder(list), new Set())).toEqual(["a", "c"]);
    list[1] = task("b", "Bo", "idle");
    expect(fillPins(["a", "c"], gridOrder(list), new Set())).toEqual(["a", "c", "b"]);
  });
});

describe("who needs you", () => {
  it("Fleet and the headline give the same count and the same names", () => {
    // The audit's moment: Dao asks something, Eli and Hai only went quiet.
    const list = [
      task("a", "Ana", "needs"), task("d", "Dao", "needs"),
      task("e", "Eli", "idle"), task("h", "Hai", "review", { changedFiles: 1 }), task("w", "Wes", "working"),
    ];
    const rows = sortFleet(list.map((t) => ({
      id: t.paneId, name: t.name, color: "#fff", wsId: t.wsId, wsName: t.project, running: true,
      state: t.status.state, spawnedAt: 1, lastOutputAt: 1,
    })));
    const fleet = rows.filter((r) => r.status === "needs").map((r) => r.name);
    expect(headline(list).lead).toBe(`${fleet.length} agents need you.`);
    expect(fleet).toEqual(list.filter((t) => t.status.state === "needs").map((t) => t.name));
    expect(needsCount(rows)).toBe(fleet.length);
  });
});

describe("inbox DOM", () => {
  let panes: Pane[];
  beforeEach(() => {
    document.body.className = "";
    document.body.innerHTML = `<div id="app"><header class="topbar"><div class="tb-center"></div></header></div>
`;
    localStorage.clear();
    state.tasks = []; state.answered = []; state.texts = []; state.focused = []; state.listeners = []; state.reviewed = []; state.scoped = []; state.markedReviewed = [];
    state.sent = []; state.lands = []; state.discarded = []; state.merged = []; state.killed = [];
    state.diff = DIFF;
    state.land = { dry: { kind: "ok", base: "main", branch: "bytehive/e", sha: "", removed: false, note: null }, real: { kind: "ok", base: "main", branch: "bytehive/e", sha: "abc", removed: true, note: null } };
    panes = ["a", "e"].map((id) => ({
      id, el: document.createElement("div"), color: "#f2b27a", running: false,
      spec: { name: id === "a" ? "Ana" : "Eli", cwd: "D:/bytehive", worktree: `D:/wt/${id}`, branch: `bytehive/${id}` },
      term: { focus: () => {}, setFontSize: () => {}, fit: () => ({ cols: 80, rows: 24 }) },
    } as unknown as Pane));
    const gridEl = document.createElement("div");
    panes.forEach((p) => {
      p.el.className = "pane";
      p.el.innerHTML = '<div class="pane-bar"><span class="pb-sp"></span><span data-where></span></div>';
      gridEl.appendChild(p.el);
    });
    const main = document.createElement("main");
    main.id = "workspaces";
    main.appendChild(gridEl);
    document.getElementById("app")!.appendChild(main);
    const ws = { id: "ws-1", name: "bytehive", gridEl, panes: new Map(panes.map((p) => [p.id, p])) } as unknown as Workspace;
    workspaces.clear(); workspaces.set("ws-1", ws); setActiveWs(ws);
    setInbox(false);
  });

  it("puts an agent on the stage for the office, out of the Grid", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    setInbox(true);
    setSplit(true);
    expect(stageAgent("e")).toBe(true);
    expect(panes[1].el.classList.contains("focused")).toBe(true);
    expect(document.querySelector('[data-dock="split"]')?.getAttribute("aria-pressed") ?? "false").toBe("false");
    expect(stageAgent("gone")).toBe(false);
  });

  it("hands the keyboard to the agent on the stage, only while the app shows", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    setInbox(true);
    stageAgent("e");
    const focus = vi.spyOn(panes[1].term, "focus");
    document.getElementById("app")!.hidden = true;
    focusStage();
    expect(focus).not.toHaveBeenCalled();
    document.getElementById("app")!.hidden = false;
    focusStage();
    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("hands the keyboard to an open drawer, not to the terminal under it", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    setInbox(true);
    openChanges("e");
    const focus = vi.spyOn(panes[1].term, "focus");
    (document.activeElement as HTMLElement | null)?.blur();
    focusStage();
    expect(focus).not.toHaveBeenCalled();
    expect(document.activeElement?.classList.contains("ir-close")).toBe(true);
    closeChanges();
    focusStage();
    expect(focus).toHaveBeenCalledTimes(1);
  });

  it("can stage an agent and leave the keyboard alone, for a terminal that will be covered", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    state.quiet = [];
    setInbox(true);
    const focus = vi.spyOn(panes[1].term, "focus");
    expect(stageAgent("e", false)).toBe(true);
    expect(panes[1].el.classList.contains("focused")).toBe(true);
    expect(state.quiet).toEqual(["e"]);
    expect(focus).not.toHaveBeenCalled();
  });

  it("gives the drawers' terminal no keyboard when the office closes them", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    setInbox(true);
    openChanges("e");
    const focus = vi.spyOn(panes[1].term, "focus");
    closeChanges();
    expect(changesOpenFor()).toBeNull();
    expect(focus).not.toHaveBeenCalled();
  });

  it("stages for Changes without the keyboard: the drawer stands over that terminal", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    state.quiet = [];
    setInbox(true);
    openChanges("e");
    expect(state.quiet).toEqual(["e"]);
  });

  it("opens an agent's changes for the office, and says whose are open", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    setInbox(true);
    expect(changesOpenFor()).toBeNull();
    openChanges("e");
    expect(panes[1].el.classList.contains("focused")).toBe(true);
    expect(changesOpenFor()).toBe("e");
    openChanges("e");
    expect(changesOpenFor()).toBe("e");
    closeChanges();
    expect(changesOpenFor()).toBeNull();
    openChanges("gone");
    expect(changesOpenFor()).toBeNull();
  });

  it("shows the Founder in a project where it sits alone, with no empty state", () => {
    panes[0].spec.personId = "founder";
    workspaces.get("ws-1")!.panes.delete("e");
    panes[1].el.remove();
    state.tasks = [task("a", "Founder", "idle")];
    setInbox(true);
    expect(panes[0].el.classList.contains("focused")).toBe(true);
    expect(document.querySelector<HTMLElement>(".ib-empty")!.hidden).toBe(true);
  });

  it("opens Settings from the gear in the dock and with Ctrl+,", () => {
    state.settings = 0;
    initInbox();
    (document.querySelector('[data-dock="settings"]') as HTMLButtonElement).click();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: ",", ctrlKey: true }));
    expect(state.settings).toBe(2);
  });

  it("offers an agent's actions on right-click and runs them", async () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    state.killed = []; state.renamed = [];
    (panes[1] as unknown as { running: boolean }).running = true;
    let removed = 0;
    panes[1].el.insertAdjacentHTML("beforeend", '<button data-kill></button><button data-restart></button>');
    panes[1].el.querySelector("[data-kill]")!.addEventListener("click", () => removed++);
    setInbox(true);
    const open = () => document.querySelector('.iq-row[data-id="e"]')!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 40, clientY: 80 }));
    const item = (label: string) => [...document.querySelectorAll<HTMLButtonElement>(".cm-item")].find((b) => b.textContent!.startsWith(label))!;
    open();
    expect([...document.querySelectorAll(".cm-item span")].map((x) => x.textContent))
      .toEqual(["Open", "Rename…", "Add to Grid", "Changes", "History", "Project memory", "Copy branch name", "Hand over to a new Codex agent", "Restart", "Stop", "Remove agent…"]);
    item("Stop").click();
    await Promise.resolve();
    expect(state.killed).toEqual(["e"]);
    expect(document.querySelector(".cm-menu")).toBeNull();
    state.confirm = { ok: true, value: "Zed" };
    open(); item("Rename").click();
    await new Promise((r) => setTimeout(r, 0));
    expect(state.renamed).toEqual([["e", "Zed"]]);
    open(); item("Remove agent").click();
    await new Promise((r) => setTimeout(r, 0));
    expect(removed).toBe(1);
    state.confirm = { ok: false, value: "" };
    open(); item("Remove agent").click();
    await new Promise((r) => setTimeout(r, 0));
    expect(removed).toBe(1);
  });

  it("keeps the list as it is between ticks, so a focused row keeps the keyboard", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    setInbox(true);
    const row = document.querySelector<HTMLButtonElement>('.iq-row[data-id="e"]')!;
    row.focus();
    state.listeners.forEach((cb) => cb()); // a tick with nothing new
    expect(document.querySelector('.iq-row[data-id="e"]')).toBe(row);
    expect(document.activeElement).toBe(row);
    // something did change: rebuilt, and the keyboard stays on the same agent
    state.tasks = [task("a", "Ana", "needs"), task("e", "Eli", "working")];
    state.listeners.forEach((cb) => cb());
    const again = document.querySelector<HTMLButtonElement>('.iq-row[data-id="e"]')!;
    expect(again).not.toBe(row);
    expect(document.activeElement).toBe(again);
  });

  it("asks before the ✕ on an agent's header removes it", async () => {
    state.tasks = [task("a", "Ana", "working")];
    let removed = 0;
    panes[0].el.insertAdjacentHTML("beforeend", '<button data-kill></button>');
    panes[0].el.querySelector("[data-kill]")!.addEventListener("click", () => removed++);
    setInbox(true);
    state.confirm = { ok: false, value: "" };
    // a real click (trusted) is held for the question
    const kill = panes[0].el.querySelector<HTMLElement>("[data-kill]")!;
    const real = new MouseEvent("click", { bubbles: true, cancelable: true });
    Object.defineProperty(real, "isTrusted", { value: true });
    kill.dispatchEvent(real);
    await new Promise((r) => setTimeout(r, 0));
    expect(removed).toBe(0);
    state.confirm = { ok: true, value: "" };
    const again = new MouseEvent("click", { bubbles: true, cancelable: true });
    Object.defineProperty(again, "isTrusted", { value: true });
    kill.dispatchEvent(again);
    await new Promise((r) => setTimeout(r, 0));
    expect(removed).toBe(1);
  });

  it("shows the agent menu from a row's ⋯ and the help page from the dock", () => {
    state.tasks = [task("a", "Ana", "working")];
    setInbox(true);
    (document.querySelector('.iq-row[data-id="a"] [data-more]') as HTMLElement).click();
    expect(document.querySelector(".cm-menu")).not.toBeNull();
    expect(state.focused.filter((x) => x === "a").length).toBeLessThanOrEqual(1); // ⋯ does not also open the agent
    document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    (document.querySelector('[data-dock="help"]') as HTMLButtonElement).click();
    expect(document.querySelector(".hp-back")).not.toBeNull();
    document.querySelector(".hp-back")!.remove();
  });

  it("resumes a stopped agent from its row and from the stage, one click each", () => {
    state.tasks = [task("a", "Ana", "stopped"), task("e", "Eli", "working")];
    const resumed: string[] = [];
    for (const p of panes) p.restart = async () => { resumed.push(p.id); };
    setInbox(true);
    expect(document.querySelector('.iq-row[data-id="e"] [data-resume]')).toBeNull();
    (document.querySelector('.iq-row[data-id="a"] [data-resume]') as HTMLElement).click();
    expect(resumed).toEqual(["a"]);
    // on the stage a stopped agent gets Resume instead of the CLI's buttons
    const stage = panes[0].el;
    expect(stage.querySelector('[data-stage="cmds"]')).toBeNull();
    (stage.querySelector('[data-stage="resume"]') as HTMLButtonElement).click();
    expect(resumed).toEqual(["a", "a"]);
  });

  it("mounts at startup", () => {
    initInbox();
    expect(document.body.classList.contains("inbox-ui")).toBe(true);
    expect(document.querySelector(".inbox-queue")).not.toBeNull();
  });

  it("turns a pane still marked as the stage back into the stage when its grid lost it", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "working")];
    // What a pane handed back from Split looks like: marked focused, grid not a stage.
    panes[1].el.classList.add("focused");
    setInbox(true);
    expect(panes[1].el.parentElement!.classList.contains("has-focus")).toBe(true);
    expect(state.focused).toContain("e");
  });

  it("puts the agent that needs you on the stage and leaves the answer to its terminal", () => {
    state.tasks = [task("a", "Ana", "needs"), task("e", "Eli", "working")];
    setInbox(true);
    expect(state.focused[0]).toBe("a");
    expect(document.querySelector(".inbox-head")!.textContent).toBe("1 agent needs you. 1 is working.");
    expect(document.querySelectorAll(".iq-row")).toHaveLength(2);
    // no card of ByteHive's own over the terminal: the CLI asks there
    expect((document.querySelector(".inbox-ask") as HTMLElement).hidden).toBe(true);
    expect(document.querySelector(".ia-opt")).toBeNull();
    expect(panes[0].el.querySelector(".ib-pill")!.textContent).toBe("Needs you");
    expect([...panes[0].el.querySelectorAll(".ib-act")].map((b) => b.textContent)).toEqual(["Commands", "Changes", "History"]);
  });

  it("switches the stage from the queue and with Alt+arrows, and leaves Alt+number to the terminal", async () => {
    state.tasks = [task("a", "Ana", "needs"), task("e", "Eli", "working")];
    setInbox(true);
    (document.querySelector('.iq-row[data-id="e"]') as HTMLButtonElement).click();
    expect(state.focused[state.focused.length - 1]).toBe("e");
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", altKey: true }));
    expect(state.focused[state.focused.length - 1]).toBe("a");
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "1", altKey: true }));
    await Promise.resolve();
    expect(state.answered).toEqual([]);
  });

  it("shows one agent as the Grid's terminal and the others as cards that answer and swap in", async () => {
    state.tasks = [task("a", "Ana", "needs"), task("e", "Eli", "needs")];
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "s", altKey: true }));
    expect(document.querySelector('[data-dock="split"]')!.getAttribute("aria-pressed")).toBe("true");
    // Ana is the terminal, Eli a card
    expect([...document.querySelectorAll(".split-pin")]).toEqual([panes[0].el]);
    expect(panes[0].el.style.getPropertyValue("--sw")).not.toBe("");
    const card = () => document.querySelector<HTMLElement>('.ib-rail [data-card="e"]');
    expect(card()!.querySelector(".rc-n")!.textContent).toBe("Eli");
    expect((document.querySelector(".inbox-ask") as HTMLElement).hidden).toBe(true);
    // Ana answers in her terminal, Eli on his card
    expect(panes[0].el.querySelector(".ia-opt")).toBeNull();
    (card()!.querySelector('.rc-opts [data-n="1"]') as HTMLButtonElement).click();
    await Promise.resolve();
    expect(state.answered).toEqual([["e", "1"]]);
    // a click on Eli's card puts him in the terminal and Ana on a card
    card()!.click();
    expect([...document.querySelectorAll(".split-pin")]).toEqual([panes[1].el]);
    expect(panes[1].el.classList.contains("focused")).toBe(true);
    expect(document.querySelector('.ib-rail [data-card="a"]')).not.toBeNull();
    // ✕ takes Eli out; with one agent left it is a single stage again
    (panes[1].el.querySelector('[data-stage="close"]') as HTMLButtonElement).click();
    expect(document.querySelectorAll(".split-pin")).toHaveLength(0);
    expect(document.querySelector(".ib-rail")).toBeNull();
    expect(panes[0].el.classList.contains("focused")).toBe(true);
    expect(panes[0].el.querySelector(".ib-acts")!.className).toBe("ib-acts");
    (document.querySelector('[data-dock="queue"]') as HTMLButtonElement).click();
    expect(document.querySelector('[data-dock="split"]')!.getAttribute("aria-pressed")).toBe("false");
    expect(panes[0].el.style.getPropertyValue("--sw")).toBe("");
  });

  it("offers Review when an agent is done, and sends it back with feedback", async () => {
    state.tasks = [task("e", "Eli", "review", { changedFiles: 3 })];
    (panes[1] as unknown as { running: boolean }).running = true;
    setInbox(true);
    const chip = document.querySelector('.inbox-ask [data-act="review"]') as HTMLButtonElement;
    expect(chip.textContent).toContain("Eli is ready · 3 files changed");
    chip.click();
    await flush();
    expect(state.reviewed).toEqual(["D:/wt/e"]);
    const drawer = document.querySelector(".inbox-review")!;
    expect(drawer.getAttribute("aria-label")).toBe("Eli's changes");
    expect((document.querySelector(".inbox-ask") as HTMLElement).hidden).toBe(true);
    // the design's panel: what changed on the branch, where it goes, whether it merges
    expect(drawer.querySelector(".rhead h2")!.textContent).toBe("1 file changed +1−1");
    expect(drawer.querySelector(".br")!.textContent).toBe("bytehive/e → main");
    expect(drawer.querySelector(".check")!.textContent).toContain("Merges cleanly into main");
    expect([...drawer.querySelectorAll(".dl .ln")].map((x) => x.textContent)).toEqual(["1", "", "2"]);
    expect([...drawer.querySelectorAll(".row2 button")].map((b) => b.textContent!.trim())).toEqual(["Merge into main M", "Send back", "Discard branch"]);
    // Send back with nothing to say asks for a note first
    (drawer.querySelector('[data-act="back"]') as HTMLButtonElement).click();
    await flush();
    expect(state.sent).toEqual([]);
    const note = document.getElementById("irNote") as HTMLTextAreaElement;
    note.value = "add a test for the empty case";
    note.dispatchEvent(new Event("input", { bubbles: true }));
    (drawer.querySelector('[data-act="back"]') as HTMLButtonElement).click();
    await flush();
    expect(state.sent).toEqual([["e", "add a test for the empty case"]]);
    expect(document.querySelector(".inbox-review")).toBeNull();
  });

  it("keeps a note per agent while you look elsewhere", async () => {
    state.tasks = [task("e", "Eli", "review", { changedFiles: 1 })];
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true }));
    await flush();
    const note = document.getElementById("irNote") as HTMLTextAreaElement;
    note.value = "half a thought";
    note.dispatchEvent(new Event("input", { bubbles: true }));
    (document.querySelector('.inbox-review [data-act="close"]') as HTMLButtonElement).click();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true }));
    await flush();
    expect((document.getElementById("irNote") as HTMLTextAreaElement).value).toBe("half a thought");
  });

  it("merges with M: stops the agent, lands its branch and marks it merged", async () => {
    state.tasks = [task("e", "Eli", "review", { changedFiles: 1, title: "Fix upload" })];
    (panes[1] as unknown as { running: boolean }).running = true;
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true }));
    await flush();
    const diff = document.querySelector<HTMLElement>(".inbox-review .diff")!;
    diff.dispatchEvent(new KeyboardEvent("keydown", { key: "m", bubbles: true, cancelable: true }));
    await flush();
    // a dry run on open, one before stopping the agent, then the real one
    expect(state.lands.map((l) => l.dryRun)).toEqual([true, true, false]);
    expect(state.lands[2]).toEqual({ wt: "D:/wt/e", dryRun: false, title: "Fix upload" });
    expect(state.killed).toEqual(["e"]);
    expect(state.merged).toEqual([["e", "main", "bytehive/e"]]);
    expect(panes[1].spec.worktree).toBeUndefined();
    expect(document.querySelector(".inbox-review")).toBeNull();
  });

  it("on a conflict merges nothing and Send back types what to resolve", async () => {
    state.tasks = [task("e", "Eli", "review", { changedFiles: 1 })];
    (panes[1] as unknown as { running: boolean }).running = true;
    state.land.dry = { kind: "conflict", base: "main", branch: "bytehive/e", files: ["src/a.ts"] };
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true }));
    await flush();
    const drawer = document.querySelector(".inbox-review")!;
    expect(drawer.querySelector(".check")!.textContent).toContain("Conflicts with main in 1 file");
    expect(drawer.querySelector(".ir-msg")!.textContent).toContain("would conflict in src/a.ts. Nothing was merged.");
    (drawer.querySelector('[data-act="merge"]') as HTMLButtonElement).click();
    await flush();
    expect(state.lands.every((l) => l.dryRun)).toBe(true);
    expect(state.killed).toEqual([]);
    // one click, no note needed
    (document.querySelector('.inbox-review [data-act="back"]') as HTMLButtonElement).click();
    await flush();
    expect(state.sent[0][0]).toBe("e");
    expect(state.sent[0][1]).toContain("conflicts in:\n- src/a.ts");
    expect(state.sent[0][1]).toContain("git merge main");
  });

  it("discards a branch only after you confirm", async () => {
    state.tasks = [task("e", "Eli", "review", { changedFiles: 1 })];
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true }));
    await flush();
    state.confirm = { ok: false, value: "" };
    (document.querySelector('.inbox-review [data-act="discard"]') as HTMLButtonElement).click();
    await flush();
    expect(state.discarded).toEqual([]);
    state.confirm = { ok: true, value: "" };
    (document.querySelector('.inbox-review [data-act="discard"]') as HTMLButtonElement).click();
    await flush();
    expect(state.discarded).toEqual(["D:/wt/e"]);
    expect(panes[1].spec.branch).toBeUndefined();
    expect(document.querySelector(".inbox-review")).toBeNull();
  });

  it("offers no Merge to an agent working in the project folder", async () => {
    delete (panes[1].spec as { worktree?: string }).worktree;
    state.tasks = [task("e", "Eli", "review", { changedFiles: 1 })];
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true }));
    await flush();
    const drawer = document.querySelector(".inbox-review")!;
    // the files the row counted, not the whole folder
    expect(state.reviewed).toEqual(["D:/bytehive"]);
    expect(state.scoped).toEqual([["h1", ["src/a.ts"]]]);
    expect(state.lands).toEqual([]);
    expect([...drawer.querySelectorAll(".row2 button")].map((b) => b.textContent!.trim())).toEqual(["Mark as reviewed", "Send back"]);
    expect(drawer.querySelector(".ir-msg")!.textContent).toContain("works in the project folder");
    drawer.querySelector(".diff")!.dispatchEvent(new KeyboardEvent("keydown", { key: "m", bubbles: true }));
    await flush();
    expect(state.lands).toEqual([]);
    drawer.querySelector<HTMLElement>('[data-act="reviewed"]')!.click();
    expect(state.markedReviewed).toEqual(["e"]);
    expect(document.querySelector(".inbox-review")).toBeNull();
  });

  it("says when a change in a shared folder may be another agent's", async () => {
    delete (panes[1].spec as { worktree?: string }).worktree;
    state.tasks = [task("e", "Eli", "review", { changedFiles: 2, shared: 1 })];
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true }));
    await flush();
    expect(document.querySelector(".inbox-review .ir-msg")!.textContent).toContain("1 of these changed while another agent was working too");
    expect(rowLine(state.tasks[0])).toBe("2 files changed, 1 maybe another agent's");
  });

  it("comments on diff lines, keeps them per agent, and sends them all back in one message", async () => {
    state.tasks = [task("e", "Eli", "review", { changedFiles: 1 })];
    (panes[1] as unknown as { running: boolean }).running = true;
    setInbox(true);
    const openR = async () => { document.dispatchEvent(new KeyboardEvent("keydown", { key: "r", altKey: true })); await flush(); };
    await openR();
    const q = <T extends HTMLElement>(sel: string) => document.querySelector<T>(`.inbox-review ${sel}`)!;
    const lns = () => [...document.querySelectorAll<HTMLButtonElement>(".inbox-review .ln")];
    // one line number in the Tab order; the arrows walk the rest
    expect(lns().map((b) => b.tabIndex)).toEqual([0, -1, -1]);
    expect(lns()[1].getAttribute("aria-label")).toBe("Comment on removed line 2");
    lns()[0].focus();
    lns()[0].dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(document.activeElement).toBe(lns()[1]);
    // click the number of "+new": a box opens under it, with the keyboard in it
    lns()[2].click();
    const box = q<HTMLTextAreaElement>("#rcText");
    expect(document.activeElement).toBe(box);
    expect(q(".rc-box .rc-where").textContent).toBe("Comment on src/a.ts:2");
    // Esc puts the box away, not the drawer
    box.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    expect(document.querySelector(".rc-box")).toBeNull();
    expect(document.querySelector(".inbox-review")).not.toBeNull();
    lns()[2].click();
    q<HTMLTextAreaElement>("#rcText").value = "Call it fresh";
    q<HTMLTextAreaElement>("#rcText").dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true, cancelable: true }));
    expect(q(".rc-note .rc-text").textContent).toBe("Call it fresh");
    expect(q(".dl.cm .ln").textContent).toBe("2");
    expect(q('[data-act="back"]').textContent!.replace(/\s+/g, " ").trim()).toBe("Send back 1");
    expect(q('[data-act="back"]').getAttribute("aria-label")).toBe("Send back with 1 comment");
    // Shift-click a run of lines: the context line and the new one
    lns()[0].click();
    q('[data-act="rc-cancel"]').click();
    lns()[2].dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
    expect(q(".rc-box .rc-where").textContent).toBe("Comment on src/a.ts:1-2");
    q<HTMLTextAreaElement>("#rcText").value = "Merge these two";
    q('[data-act="rc-save"]').click();
    // edit one, remove none
    (document.querySelectorAll<HTMLButtonElement>('.inbox-review [data-act="rc-edit"]')[0]).click();
    q<HTMLTextAreaElement>("#rcText").value = "Call it fresh, please";
    q('[data-act="rc-save"]').click();
    // switching screens keeps them
    q('[data-act="close"]').click();
    await openR();
    expect([...document.querySelectorAll(".inbox-review .rc-note .rc-text")].map((x) => x.textContent)).toEqual(["Call it fresh, please", "Merge these two"]);
    // a comment can go
    const del = document.querySelectorAll<HTMLButtonElement>('.inbox-review [data-act="rc-del"]');
    expect(del[1].getAttribute("aria-label")).toBe("Remove comment on src/a.ts:1-2");
    del[1].click();
    expect(document.querySelectorAll(".inbox-review .rc-note")).toHaveLength(1);
    q('[data-act="back"]').click();
    await flush();
    expect(state.sent).toHaveLength(1);
    expect(state.sent[0][1]).toBe("Review comments on your changes:\n\nsrc/a.ts:2\n> new\nCall it fresh, please\n\nAddress this comment, then tell me when it is done.");
    // sent: the drafts are gone
    await openR();
    expect(document.querySelectorAll(".inbox-review .rc-note")).toHaveLength(0);
    expect(q('[data-act="back"]').textContent!.trim()).toBe("Send back");
  });

  it("shows merged agents as Merged and clears them", () => {
    state.tasks = [task("a", "Ana", "working"), task("e", "Eli", "merged", { mergedInto: "main" })];
    let removed = 0;
    panes[1].el.insertAdjacentHTML("beforeend", "<button data-kill></button>");
    panes[1].el.querySelector("[data-kill]")!.addEventListener("click", () => removed++);
    setInbox(true);
    expect(document.querySelector('.iq-row[data-id="e"] .iq-l')!.textContent).toBe("Merged into main");
    expect(document.querySelector('.iq-row[data-id="e"]')!.classList.contains("st-merged")).toBe(true);
    const clear = document.querySelector<HTMLButtonElement>("[data-clear-merged]")!;
    expect(clear.textContent).toBe("Clear 1 merged");
    clear.click();
    expect(removed).toBe(1);
  });

  it("lists agents under their project, the project that needs you first", () => {
    const other = { id: "ws-2", name: "quy", gridEl: document.createElement("div"), panes: new Map() } as unknown as Workspace;
    workspaces.set("ws-2", other);
    document.querySelector(".topbar")!.insertAdjacentHTML("beforeend", '<div class="tb-right"></div>');
    state.tasks = [task("e", "Eli", "stopped"), task("q", "Quinn", "working", { wsId: "ws-2", project: "quy" }), task("a", "Ana", "needs")];
    setInbox(true);
    const groups = [...document.querySelectorAll(".iq-group")].map((g) => [
      g.querySelector(".iq-pn")!.textContent,
      [...g.querySelectorAll(".iq-row")].map((r) => (r as HTMLElement).dataset.id),
    ]);
    expect(groups).toEqual([["bytehive", ["a", "e"]], ["quy", ["q"]]]);
    expect(document.querySelector(".iq-chip")).toBeNull();
    // the second line says the state, not the project
    expect(document.querySelector('.iq-row[data-id="e"] .iq-l')!.textContent).toBe("Stopped");
    // who needs you is counted on its project
    expect(document.querySelector('[data-proj="ws-1"] .iq-need')!.textContent).toBe("1");
    // a project folds its stopped agents away, keeps the running ones and its count, and stays folded
    (document.querySelector('[data-proj="ws-1"]') as HTMLButtonElement).click();
    const head = document.querySelector('[data-proj="ws-1"]')!;
    expect(head.getAttribute("aria-expanded")).toBe("false");
    expect([...head.closest(".iq-group")!.querySelectorAll(".iq-row")].map((r) => (r as HTMLElement).dataset.id)).toEqual(["a"]);
    expect(head.getAttribute("title")).toContain("show 1 stopped");
    expect(head.querySelector(".iq-need")!.textContent).toBe("1");
    // a folded project with nobody running shows no rows at all
    (document.querySelector('[data-proj="ws-2"]') as HTMLButtonElement).click();
    state.tasks = [task("e", "Eli", "stopped"), task("q", "Quinn", "stopped", { wsId: "ws-2", project: "quy" }), task("a", "Ana", "needs")];
    state.listeners.forEach((cb) => cb());
    expect((document.querySelector('[data-proj="ws-2"]')!.closest(".iq-group")!.querySelector(".iq-rows") as HTMLElement).hidden).toBe(true);
    (document.querySelector('[data-proj="ws-2"]') as HTMLButtonElement).click();
    expect(JSON.parse(localStorage.getItem("maestro.inbox.folded")!)).toEqual(["ws-1"]);
    (document.querySelector('[data-proj="ws-1"]') as HTMLButtonElement).click();
    expect(document.querySelector('[data-proj="ws-1"]')!.getAttribute("aria-expanded")).toBe("true");
  });

  it("opens its own palette on Ctrl K with every agent and the actions", () => {
    state.tasks = [task("a", "Ana", "needs"), task("e", "Eli", "working")];
    setInbox(true);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, cancelable: true }));
    const labels = [...document.querySelectorAll(".pal-it .pal-t")].map((t) => t.firstChild!.textContent);
    expect(labels.slice(0, 3)).toEqual(["Ana", "Eli", "New agent"]);
    expect(labels).toContain("Pomodoro timer");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, cancelable: true }));
    expect(document.querySelector(".inbox-pal")).toBeNull();
  });

  it("titles a row with the job, names the agent under it, tags races and counts lines", () => {
    expect(lineCounts({ added: 148, removed: 455 })).toContain("+148");
    expect(lineCounts({ added: 0, removed: 0 })).toBe("");
    state.tasks = [task("e", "Eli", "review", { title: "Fix the flaky upload test", race: { id: "r", n: 1, of: 3 }, changedFiles: 2, added: 12, removed: 5 })];
    setInbox(true);
    const row = document.querySelector('.iq-row[data-id="e"]')!;
    expect(row.querySelector(".iq-t")!.textContent).toBe("Fix the flaky upload test");
    expect(row.querySelector(".iq-p")!.textContent).toBe("Eli");
    expect(row.querySelector(".iq-race")!.textContent).toBe("race 1/3");
    expect(row.querySelector(".iq-ln")!.textContent).toBe("+12 −5");
    expect(panes[1].el.querySelector(".ib-ttl")!.textContent).toBe("Fix the flaky upload test");
  });

  it("shows agents of other projects as cards, brings one in to work with it and gives it back after", () => {
    const qEl = document.createElement("div");
    qEl.className = "pane";
    qEl.innerHTML = '<div class="pane-bar"><span class="pb-sp"></span><span data-where></span></div>';
    const qGrid = document.createElement("div");
    qGrid.appendChild(qEl);
    const q = { id: "q", el: qEl, color: "#8fb3ff", running: false, spec: { name: "Quinn", cwd: "D:/quy" },
      term: { focus: () => {}, setFontSize: () => {}, fit: () => ({ cols: 80, rows: 24 }) } } as unknown as Pane;
    workspaces.set("ws-2", { id: "ws-2", name: "quy", gridEl: qGrid, panes: new Map([["q", q]]) } as unknown as Workspace);
    state.tasks = [task("a", "Ana", "needs"), task("q", "Quinn", "working", { wsId: "ws-2", project: "quy" })];
    setInbox(true);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "s", altKey: true }));
    const grid = panes[0].el.parentElement!;
    // Quinn waits on a card, his terminal stays home
    expect(qEl.parentElement).toBe(qGrid);
    // his card sits under his project's name, as it is not the terminal's
    expect([...document.querySelectorAll(".ib-rail .rc-g b")].map((b) => b.textContent)).toEqual(["quy"]);
    expect(document.querySelector('.ib-rail [data-card="q"]')!.getAttribute("aria-label")).toBe("Quinn, in quy");
    (document.querySelector('.ib-rail [data-card="q"]') as HTMLElement).click();
    expect(qEl.parentElement).toBe(grid);
    expect(qEl.classList.contains("split-pin")).toBe(true);
    expect(qEl.classList.contains("focused")).toBe(true);
    // the terminal names its project; Ana's card now goes under hers
    expect(qEl.querySelector("[data-where]")!.textContent).toBe("quy");
    expect([...document.querySelectorAll(".ib-rail .rc-g b")].map((b) => b.textContent)).toEqual(["bytehive"]);
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "s", altKey: true }));
    expect(qEl.parentElement).toBe(qGrid);
    expect(qEl.classList.contains("split-pin")).toBe(false);
  });

  it("compares agents racing on one job and opens the one you pick in Review", async () => {
    const race = (n: number) => ({ id: "r1", n, of: 2 });
    state.tasks = [
      task("a", "Ana", "review", { title: "Fix upload", race: race(1), changedFiles: 2, added: 10, removed: 1 }),
      task("e", "Eli", "working", { title: "Fix upload", race: race(2) }),
    ];
    setInbox(true);
    (panes[0].el.querySelector('[data-stage="compare"]') as HTMLButtonElement).click();
    const cols = [...document.querySelectorAll(".cmp-col")];
    expect(cols.map((c) => c.querySelector("b")!.textContent)).toEqual(["Ana", "Eli"]);
    expect(cols[0].querySelector(".cmp-n")!.textContent).toBe("2 files changed+10 −1");
    (cols[1].querySelector("[data-review]") as HTMLButtonElement).click();
    expect(document.querySelector(".cmp-back")).toBeNull();
    await flush();
    expect(state.reviewed).toEqual(["D:/wt/e"]);
  });

  it("brings up an agent that starts needing you when Settings says so and the stage is quiet", () => {
    setPref("jumpToNeeds", true);
    state.tasks = [task("a", "Ana", "idle"), task("e", "Eli", "working")];
    setInbox(true);
    expect(state.focused[state.focused.length - 1]).toBe("a");
    state.tasks = [task("e", "Eli", "needs"), task("a", "Ana", "idle")];
    state.listeners.forEach((cb) => (cb as (l: Task[]) => void)(state.tasks));
    expect(state.focused[state.focused.length - 1]).toBe("e");
  });
});
