// src/company/office/shell.test.ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { demoSource } from "./fixtures";
import { BUILDING } from "./nav";
import { createShell, settleTags, type ShellHost } from "./shell";

const scene = vi.hoisted(() => ({
  setModel: vi.fn(), setLevel: vi.fn(), resize: vi.fn(), setVisible: vi.fn(), pick: vi.fn(() => null),
  anchors: vi.fn(() => []), onMoved: vi.fn(), stats: vi.fn(), dispose: vi.fn(),
}));
const sceneModule = { createScene: vi.fn(() => scene), webglProbe: () => ({ ok: true, renderer: "test" }) };

const made: Array<ReturnType<typeof createShell>> = [];

function setup(over: Partial<ShellHost> = {}) {
  const host: ShellHost = {
    showDesk: vi.fn(() => true), levelChanged: vi.fn(), leave: vi.fn(), failed: vi.fn(),
    loadScene: vi.fn(async () => sceneModule as never), ...over,
  };
  const source = demoSource();
  const shell = createShell(source, host);
  document.body.appendChild(shell.el);
  made.push(shell);
  return { host, source, shell };
}
const calm = { terminal: false, dialog: false, dock: false, typing: false };
const tick = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => { document.body.innerHTML = ""; vi.clearAllMocks(); });
afterEach(() => { for (const shell of made.splice(0)) shell.dispose(); });

describe("the office shell", () => {
  it("does not load the scene until it is shown", async () => {
    const { host, shell } = setup();
    expect(host.loadScene).not.toHaveBeenCalled();
    shell.show();
    await tick();
    expect(host.loadScene).toHaveBeenCalledTimes(1);
  });
  it("hands the scene the model and the level", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    expect(scene.setModel).toHaveBeenCalled();
    expect(scene.setLevel).toHaveBeenLastCalledWith(BUILDING, true);
  });
  it("gives the scene the same model again after a redraw, so it holds nothing of its own", async () => {
    const { shell, source } = setup();
    shell.show();
    await tick();
    const first = scene.setModel.mock.lastCall![0];
    await source.approve("nothing");
    expect(JSON.stringify(scene.setModel.mock.lastCall![0])).toBe(JSON.stringify(first));
  });
  it("shows the job box at the building only, and the message box at a floor only", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    expect(shell.el.querySelector("[data-job-box]")?.hasAttribute("hidden")).toBe(false);
    expect(shell.el.querySelector("[data-msg-box]")?.hasAttribute("hidden")).toBe(true);
    shell.go({ kind: "floor", floorId: "bytehive" });
    expect(shell.el.querySelector("[data-job-box]")?.hasAttribute("hidden")).toBe(true);
    expect(shell.el.querySelector("[data-msg-box]")?.hasAttribute("hidden")).toBe(false);
    expect(shell.el.querySelector("[data-msg-box] label")?.textContent).toBe("Message Hai");
  });
  it("keeps Needs you at all three levels", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    for (const l of [BUILDING, { kind: "floor", floorId: "bytehive" } as const, { kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" } as const]) {
      shell.go(l);
      expect(shell.el.querySelector("[data-needs]")?.children.length).toBeGreaterThan(0);
    }
  });
  it("walks the breadcrumb back up", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    const labels = () => [...shell.el.querySelectorAll("[data-crumb]")].map((c) => c.textContent);
    expect(labels()).toEqual(["ByteHive", "bytehive", "Linh"]);
    shell.el.querySelector<HTMLElement>('[data-crumb="1"]')!.click();
    expect(shell.level()).toEqual({ kind: "floor", floorId: "bytehive" });
  });
  it("opens the person's real pane at Desk", async () => {
    const { shell, host } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    expect(host.showDesk).toHaveBeenCalledWith("linh", "terminal", true);
    expect(host.levelChanged).toHaveBeenLastCalledWith({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
  });
  it("offers to start someone with no pane", async () => {
    const { shell, source } = setup({ showDesk: vi.fn(() => false) });
    const wake = vi.spyOn(source, "wakePerson");
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "bao", floorId: "oldsite", tab: "terminal" });
    const b = shell.el.querySelector<HTMLElement>("[data-wake]")!;
    expect(b.textContent).toBe("Start Bao");
    b.click();
    expect(wake).toHaveBeenCalledWith("bao");
  });
  it("goes up one level on Esc, not past the building", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "floor", floorId: "bytehive" });
    expect(shell.escape(calm)).toBe(true);
    expect(shell.level()).toEqual(BUILDING);
    expect(shell.escape(calm)).toBe(false);
  });
  it("leaves Esc to a focused terminal", async () => {
    const { shell } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    expect(shell.escape({ ...calm, terminal: true })).toBe(false);
    expect(shell.level().kind).toBe("desk");
  });
  it("uses a button for every person and floor, for the keyboard", async () => {
    scene.anchors.mockReturnValue([{ id: "bytehive", kind: "floor", x: 10, y: 10, visible: true }] as never);
    const { shell } = setup();
    shell.show();
    await tick();
    const tag = shell.el.querySelector<HTMLButtonElement>('button[data-tag="bytehive"]')!;
    expect(tag.textContent).toContain("bytehive");
    tag.click();
    expect(shell.level()).toEqual({ kind: "floor", floorId: "bytehive" });
  });
  it("falls back to its level when a person leaves", async () => {
    const { shell, source } = setup();
    shell.show();
    await tick();
    shell.go({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" });
    vi.spyOn(source, "snapshot").mockReturnValue({ model: { ...source.snapshot(0).model!, floors: [] }, note: null, error: null });
    await source.approve("nothing");
    expect(shell.level()).toEqual(BUILDING);
  });
  it("reports a scene that cannot be made", async () => {
    sceneModule.createScene.mockImplementationOnce(() => { throw new Error("no context"); });
    const { shell, host } = setup();
    shell.show();
    await tick();
    expect(host.failed).toHaveBeenCalledWith("no context");
  });
  it("reports a three.js chunk that will not load", async () => {
    const { shell, host } = setup({ loadScene: vi.fn(async () => { throw new Error("offline"); }) });
    shell.show();
    await tick();
    expect(host.failed).toHaveBeenCalledWith("offline");
  });
  it("says where the Company runs when there is no model", async () => {
    const { shell, source } = setup();
    vi.spyOn(source, "snapshot").mockReturnValue({ model: null, note: "The Company is running in another ByteHive window.", error: null });
    shell.show();
    await tick();
    expect(shell.el.textContent).toContain("another ByteHive window");
  });
});

const desk = (personId: string, floorId: string | null, tab: "terminal" | "changes" | "report" = "terminal") => ({ kind: "desk", personId, floorId, tab }) as const;
const floor = (floorId: string) => ({ kind: "floor", floorId }) as const;
const anchor = (id: string, kind: string, x = 10, y = 10, visible = true) => ({ id, kind, x, y, visible });
const send = (el: HTMLElement) => el.querySelector("[data-msg-box]")!.dispatchEvent(new Event("submit", { cancelable: true, bubbles: true }));
const evil = `<img src=x onerror=alert(1)>"'`;

async function shown(over: Partial<ShellHost> = {}) {
  document.body.innerHTML = `<header class="topbar"><div class="tb-center"></div></header>`;
  const s = setup(over);
  s.shell.show();
  await tick();
  return s;
}
/** The camera comes to rest. */
const rest = () => scene.onMoved.mock.lastCall![0]();
const tabs = () => [...document.querySelectorAll<HTMLElement>('.tb-center [role="tab"]')];
const selected = () => tabs().filter((t) => t.getAttribute("aria-selected") === "true").map((t) => t.textContent);

describe("tags", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); scene.pick.mockReturnValue(null); });

  it("says who is on a floor and who needs you", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor")] as never);
    const { shell } = await shown();
    const tag = shell.el.querySelector('[data-tag="bytehive"]')!;
    expect(tag.querySelector("b")?.textContent).toBe("bytehive");
    expect(tag.textContent).toContain("4 people");
    expect(tag.textContent).toContain("2 need you");
  });
  it("counts what belongs to no floor at the top, so the tags add up to the rail", async () => {
    scene.anchors.mockReturnValue([anchor("top", "top"), anchor("bytehive", "floor"), anchor("gcare", "floor")] as never);
    const { shell, source } = await shown();
    expect(shell.el.querySelector('[data-tag="top"]')?.textContent).toBe("Founder 2 need you");
    const say = (id: string) => Number(/(\d+) needs? you/.exec(shell.el.querySelector(`[data-tag="${id}"]`)?.textContent ?? "")?.[1]);
    expect(say("top") + say("bytehive") + say("gcare")).toBe(source.snapshot(0).model!.needs.length);
  });
  it("says only Founder at the top when nothing waits there", async () => {
    scene.anchors.mockReturnValue([anchor("top", "top")] as never);
    const { shell, source } = await shown();
    const real = source.snapshot(0).model!;
    vi.spyOn(source, "snapshot").mockReturnValue({ model: { ...real, needs: [] }, note: null, error: null });
    await source.approve("nothing");
    expect(shell.el.querySelector('[data-tag="top"]')?.textContent).toBe("Founder");
  });
  it("marks a closed floor", async () => {
    scene.anchors.mockReturnValue([anchor("oldsite", "floor")] as never);
    const { shell } = await shown();
    expect(shell.el.querySelector('[data-tag="oldsite"]')?.textContent).toContain("closed");
  });
  it("takes the top straight to the Founder's Desk", async () => {
    scene.anchors.mockReturnValue([anchor("top", "top")] as never);
    const { shell } = await shown();
    const tag = shell.el.querySelector<HTMLElement>('[data-tag="top"]')!;
    expect(tag.textContent).toContain("Founder");
    tag.click();
    expect(shell.level()).toEqual(desk("founder", null));
  });
  it("names the people of this floor only, at a floor", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor"), anchor("hai", "person"), anchor("linh", "person"), anchor("mai", "person")] as never);
    const { shell } = await shown();
    shell.go(floor("bytehive"));
    rest();
    expect(shell.el.querySelector('[data-tag="bytehive"]')).toBeNull();
    expect(shell.el.querySelector('[data-tag="mai"]')).toBeNull();
    expect(shell.el.querySelector('[data-tag="hai"]')?.textContent).toBe("Hai Lead, needs you");
    expect(shell.el.querySelector('[data-tag="linh"]')?.textContent).toBe("Linh Developer");
    shell.el.querySelector<HTMLElement>('[data-tag="linh"]')!.click();
    expect(shell.level()).toEqual(desk("linh", "bytehive"));
  });
  it("draws no tags at a Desk", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor"), anchor("linh", "person")] as never);
    const { shell } = await shown();
    shell.go(desk("linh", "bytehive"));
    rest();
    expect(shell.el.querySelectorAll("[data-tag]").length).toBe(0);
  });
  it("hides a tag whose place is off screen", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor", 10, 10, false)] as never);
    const { shell } = await shown();
    expect(shell.el.querySelector('[data-tag="bytehive"]')?.hasAttribute("hidden")).toBe(true);
  });
  it("hides the tags while the camera moves, and brings them back when it rests", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor"), anchor("hai", "person")] as never);
    const { shell } = await shown();
    const layer = shell.el.querySelector("[data-tags]")!;
    expect(layer.classList.contains("is-moving")).toBe(false);
    shell.go(floor("bytehive"));
    expect(layer.classList.contains("is-moving")).toBe(true);
    scene.onMoved.mock.lastCall![0]();
    expect(layer.classList.contains("is-moving")).toBe(false);
  });
  it("keeps the same button through a redraw, so the keyboard stays on it", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor")] as never);
    const { shell, source } = await shown();
    const tag = shell.el.querySelector<HTMLElement>('[data-tag="bytehive"]')!;
    tag.focus();
    await source.approve("nothing");
    expect(shell.el.querySelector('[data-tag="bytehive"]')).toBe(tag);
    expect(document.activeElement).toBe(tag);
  });
  it("puts a tag in the order the eye reads the tower, top first", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor", 10, 300), anchor("gcare", "floor", 10, 200), anchor("top", "top", 10, 100)] as never);
    const { shell } = await shown();
    expect([...shell.el.querySelectorAll<HTMLElement>("[data-tag]")].map((t) => t.dataset.tag)).toEqual(["top", "gcare", "bytehive"]);
  });
  it("escapes a name", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor")] as never);
    const { shell, source } = await shown();
    const real = source.snapshot(0).model!;
    const bad = { ...real, floors: real.floors.map((f) => ({ ...f, name: evil })) };
    vi.spyOn(source, "snapshot").mockReturnValue({ model: bad, note: null, error: null });
    await source.approve("nothing");
    shell.go(floor("bytehive"));
    expect(shell.el.querySelector("img")).toBeNull();
    expect(shell.el.querySelector("[data-crumb='1']")?.textContent).toBe(evil);
  });
  it("offers the board at a floor that has one, and it opens that project's board", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "board")] as never);
    const openBoard = vi.fn();
    const { shell } = await shown({ openBoard });
    shell.go(floor("bytehive"));
    rest();
    const chip = shell.el.querySelector<HTMLElement>("button[data-board]")!;
    expect(chip.textContent).toBe("To do 2, Doing 2, Done 5");
    chip.click();
    expect(openBoard).toHaveBeenCalledWith("bytehive");
  });
  it("shows the board's counts as plain text when nothing can open it", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "board")] as never);
    const { shell } = await shown();
    shell.go(floor("bytehive"));
    rest();
    expect(shell.el.querySelector("button[data-board]")).toBeNull();
    expect(shell.el.querySelector("[data-board]")?.textContent).toBe("To do 2, Doing 2, Done 5");
  });
});

describe("settleTags", () => {
  const box = (x: number, y: number, w = 100, h = 26) => ({ x, y, w, h });

  it("leaves tags alone when they are apart", () => {
    const a = box(0, 0);
    const b = box(200, 0);
    settleTags([a, b], []);
    expect([a.x, a.y, b.x, b.y]).toEqual([0, 0, 200, 0]);
  });
  it("keeps the nearer tag in place and steps the one behind up", () => {
    const near = box(10, 100);
    const far = box(30, 90);
    settleTags([far, near], []);
    expect(near.y).toBe(100);
    expect(far.y).toBe(100 - 26 - 2);
  });
  it("steps up past a whole pile", () => {
    const a = box(0, 100);
    const b = box(0, 95);
    const c = box(0, 90);
    settleTags([a, b, c], []);
    expect([a.y, b.y, c.y]).toEqual([100, 72, 44]);
  });
  it("slides a tag off a bubble, to the side it was on", () => {
    const left = box(50, 20);
    const right = box(170, 20);
    settleTags([left, right], [box(120, 0, 60, 50)]);
    expect(left.x).toBe(120 - 100 - 2);
    expect(right.x).toBe(182);
    expect([left.y, right.y]).toEqual([20, 20]);
  });
  it("stops at the edge", () => {
    const t = box(10, 20);
    settleTags([t], [box(40, 0, 60, 50)], 6);
    expect(t.x).toBe(6);
  });
});

describe("clicking the scene", () => {
  const click = (shell: { el: HTMLElement }) => shell.el.querySelector<HTMLElement>("canvas")!.click();
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); scene.pick.mockReturnValue(null); });

  it("goes to a person's Desk", async () => {
    const { shell } = await shown();
    scene.pick.mockReturnValue({ personId: "linh", floorId: "bytehive" } as never);
    click(shell);
    expect(shell.level()).toEqual(desk("linh", "bytehive"));
  });
  it("goes to a floor", async () => {
    const { shell } = await shown();
    scene.pick.mockReturnValue({ floorId: "gcare" } as never);
    click(shell);
    expect(shell.level()).toEqual(floor("gcare"));
  });
  it("goes to the Founder's Desk from the top", async () => {
    const { shell } = await shown();
    scene.pick.mockReturnValue({ personId: "founder" } as never);
    click(shell);
    expect(shell.level()).toEqual(desk("founder", null));
  });
  it("does not pick while the camera moves: a double click would go two levels down", async () => {
    const { shell } = await shown();
    scene.pick.mockReturnValue({ floorId: "gcare" } as never);
    click(shell);
    expect(shell.level()).toEqual(floor("gcare"));
    scene.pick.mockClear();
    scene.pick.mockReturnValue({ personId: "linh", floorId: "bytehive" } as never);
    click(shell);
    expect(scene.pick).not.toHaveBeenCalled();
    expect(shell.level()).toEqual(floor("gcare"));
    rest();
    click(shell);
    expect(shell.level()).toEqual(desk("linh", "bytehive"));
  });
  it("does not pick while a new size is waiting: the camera still has the old one", async () => {
    let resized = () => {};
    vi.stubGlobal("ResizeObserver", class {
      constructor(cb: () => void) { resized = cb; }
      observe() {}
      disconnect() {}
    });
    try {
      const { shell } = await shown();
      scene.pick.mockReturnValue({ personId: "linh", floorId: "bytehive" } as never);
      resized();
      click(shell);
      expect(scene.pick).not.toHaveBeenCalled();
      expect(shell.level()).toEqual(BUILDING);
      await new Promise((r) => setTimeout(r, 150));
      click(shell);
      expect(shell.level()).toEqual(desk("linh", "bytehive"));
    } finally {
      vi.unstubAllGlobals();
    }
  });
  it("does nothing on the sky", async () => {
    const { shell, host } = await shown();
    const before = vi.mocked(host.levelChanged).mock.calls.length;
    click(shell);
    expect(shell.level()).toEqual(BUILDING);
    expect(vi.mocked(host.levelChanged).mock.calls.length).toBe(before);
  });
  it("shows a pointer over something it can open, and picks at most every 80ms, only at rest", async () => {
    vi.useFakeTimers({ toFake: ["Date", "performance"] });
    try {
      const { shell } = await shownWithRealTick();
      const canvas = shell.el.querySelector<HTMLElement>("canvas")!;
      const move = () => canvas.dispatchEvent(new PointerEvent("pointermove", { bubbles: true, clientX: 5, clientY: 5 }));
      scene.pick.mockClear();
      scene.pick.mockReturnValue({ floorId: "gcare" } as never);
      move();
      expect(canvas.style.cursor).toBe("pointer");
      move();
      expect(scene.pick).toHaveBeenCalledTimes(1);
      vi.advanceTimersByTime(81);
      scene.pick.mockReturnValue(null);
      move();
      expect(scene.pick).toHaveBeenCalledTimes(2);
      expect(canvas.style.cursor).toBe("");
      shell.go(floor("gcare"));
      vi.advanceTimersByTime(81);
      move();
      expect(scene.pick).toHaveBeenCalledTimes(2);
    } finally { vi.useRealTimers(); }
  });
});

async function shownWithRealTick() {
  document.body.innerHTML = `<header class="topbar"><div class="tb-center"></div></header>`;
  const s = setup();
  s.shell.show();
  await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
  return s;
}

describe("the message box", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); });

  it("sends to the floor and clears, and says who got it", async () => {
    const { shell, source } = await shown();
    const message = vi.spyOn(source, "messageFloor");
    shell.go(floor("bytehive"));
    const box = shell.el.querySelector<HTMLTextAreaElement>("#ofMsg")!;
    box.value = "Ship it";
    send(shell.el);
    await tick();
    expect(message).toHaveBeenCalledWith("bytehive", "Ship it");
    expect(box.value).toBe("");
    expect(shell.el.querySelector("[data-msg-box]")?.textContent).toContain("Sent to Hai.");
  });
  it("names the one person on a floor with no Lead", async () => {
    const { shell } = await shown();
    shell.go(floor("vanta"));
    expect(shell.el.querySelector("[data-msg-box] label")?.textContent).toBe("Message Sol");
  });
  it("is hidden on an empty floor, where a line says so", async () => {
    const { shell } = await shown();
    shell.go(floor("scratch"));
    expect(shell.el.querySelector("[data-msg-box]")?.hasAttribute("hidden")).toBe(true);
    const note = shell.el.querySelector("[data-stage] [data-note]")!;
    expect(note.hasAttribute("hidden")).toBe(false);
    expect(note.textContent).toBe("Nobody here yet.");
  });
  it("keeps the words and says why when the message cannot go", async () => {
    const { shell, source } = await shown();
    vi.spyOn(source, "messageFloor").mockRejectedValue(new Error("Nobody on gcare can get a message."));
    shell.go(floor("bytehive"));
    const box = shell.el.querySelector<HTMLTextAreaElement>("#ofMsg")!;
    box.value = "Ship it";
    send(shell.el);
    await tick();
    expect(box.value).toBe("Ship it");
    expect(shell.el.querySelector("[data-msg-box] [role=alert]")?.textContent).toBe("Nobody on gcare can get a message.");
  });
  it("sends nothing blank, and once while it works", async () => {
    const { shell, source } = await shown();
    let done = () => {};
    const message = vi.spyOn(source, "messageFloor").mockImplementation(() => new Promise<void>((r) => { done = r; }));
    shell.go(floor("bytehive"));
    const box = shell.el.querySelector<HTMLTextAreaElement>("#ofMsg")!;
    box.value = "  ";
    send(shell.el);
    expect(message).not.toHaveBeenCalled();
    box.value = "Ship it";
    send(shell.el);
    send(shell.el);
    expect(message).toHaveBeenCalledTimes(1);
    done();
    await tick();
  });
  it("keeps a draft to its own floor, so a word for Hai cannot go to Mai", async () => {
    const { shell } = await shown();
    shell.go(floor("bytehive"));
    const box = shell.el.querySelector<HTMLTextAreaElement>("#ofMsg")!;
    box.value = "For Hai";
    shell.go(floor("gcare"));
    expect(box.value).toBe("");
    shell.go(floor("bytehive"));
    expect(box.value).toBe("For Hai");
  });
  it("gives up a text box on Esc, so a second Esc goes up", async () => {
    const { shell } = await shown();
    shell.go(floor("bytehive"));
    const box = shell.el.querySelector<HTMLTextAreaElement>("#ofMsg")!;
    box.focus();
    box.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(document.activeElement).not.toBe(box);
  });
});

describe("the Desk", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); });

  it("marks the app and puts the tabs in the top bar", async () => {
    const { shell } = await shown();
    shell.go(desk("linh", "bytehive"));
    expect(document.body.classList.contains("office-desk")).toBe(true);
    expect(tabs().map((t) => t.textContent)).toEqual(["Terminal", "Changes", "Report"]);
    expect(document.querySelector('.tb-center [role="tablist"]')?.getAttribute("aria-label")).toBe("Desk");
    expect(selected()).toEqual(["Terminal"]);
    shell.go(floor("bytehive"));
    expect(document.body.classList.contains("office-desk")).toBe(false);
    expect(tabs()).toEqual([]);
  });
  it("hands a tab to the host, and only one tab can be in the Tab order", async () => {
    const { shell, host } = await shown();
    shell.go(desk("linh", "bytehive"));
    tabs()[1].click();
    expect(host.showDesk).toHaveBeenLastCalledWith("linh", "changes", true);
    expect(shell.level()).toEqual(desk("linh", "bytehive", "changes"));
    expect(selected()).toEqual(["Changes"]);
    expect(tabs().map((t) => t.tabIndex)).toEqual([-1, 0, -1]);
  });
  it("moves between tabs with the arrows, round the ends", async () => {
    const { shell } = await shown();
    shell.go(desk("linh", "bytehive"));
    const key = (k: string) => (document.querySelector<HTMLElement>('[role="tab"][tabindex="0"]')!).dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true }));
    key("ArrowRight");
    expect(selected()).toEqual(["Changes"]);
    key("ArrowLeft");
    key("ArrowLeft");
    expect(selected()).toEqual(["Report"]);
    key("ArrowRight");
    expect(selected()).toEqual(["Terminal"]);
  });
  it("goes back to Terminal when the owner closed the drawer", async () => {
    let open: string | null = null;
    const { shell, source } = await shown({ changesOpenFor: () => open });
    open = "pane-linh";
    shell.go(desk("linh", "bytehive", "changes"));
    expect(selected()).toEqual(["Changes"]);
    open = null;
    await source.approve("nothing");
    expect(selected()).toEqual(["Terminal"]);
  });
  it("shows a report, in the owner's words, escaped", async () => {
    const { shell, source } = await shown();
    shell.go(desk("linh", "bytehive", "report"));
    await tick();
    const report = shell.el.querySelector<HTMLElement>(".of-report")!;
    expect(report.hasAttribute("hidden")).toBe(false);
    expect(report.textContent).toContain("Checkout errors");
    expect(report.textContent).toContain("J-0141");
    expect(report.textContent).toContain("Doing");
    expect(report.textContent).toContain("Network lost");
    expect(report.textContent).toContain("src/login.ts");
    expect(report.textContent).toContain("Login page is done.");
    expect(report.querySelectorAll('[data-done="true"]').length).toBe(1);
    vi.spyOn(source, "report").mockResolvedValue({ name: evil, branch: evil, cards: [{ title: evil, list: evil, job: evil, jobTitle: evil, summary: evil, files: [evil], checklist: [{ text: evil, done: true }], stopped: false }] });
    shell.go(desk("linh", "bytehive", "terminal"));
    expect(report.hasAttribute("hidden")).toBe(true);
    shell.go(desk("linh", "bytehive", "report"));
    await tick();
    expect(report.querySelector("img")).toBeNull();
    expect(report.textContent).toContain(evil);
  });
  it("says when there is nothing to report", async () => {
    const { shell } = await shown();
    shell.go(desk("sol", "vanta", "report"));
    await tick();
    expect(shell.el.querySelector(".of-report")?.textContent).toContain("Nothing to report yet.");
  });
  it("drops an answer about someone you already left", async () => {
    const { shell, source } = await shown();
    let answer = (_: null) => {};
    vi.spyOn(source, "report").mockImplementationOnce(() => new Promise((r) => { answer = r; }));
    shell.go(desk("linh", "bytehive", "report"));
    shell.go(desk("an", "bytehive", "report"));
    await tick();
    answer(null);
    await tick();
    expect(shell.el.querySelector(".of-report")?.textContent).toContain("New icons");
  });
  it("fits the terminal again once the column has stopped moving, and only fits it", async () => {
    const refit = vi.fn();
    const { shell, host } = await shown({ refit });
    shell.go(desk("linh", "bytehive"));
    const before = vi.mocked(host.showDesk).mock.calls.length;
    const end = new Event("transitionend", { bubbles: true }) as Event & { propertyName?: string };
    end.propertyName = "width";
    shell.el.dispatchEvent(end);
    expect(refit).toHaveBeenCalledTimes(1);
    expect(host.showDesk).toHaveBeenCalledTimes(before);
    shell.go(floor("bytehive"));
    shell.el.dispatchEvent(end);
    expect(refit).toHaveBeenCalledTimes(1);
  });
  it("says a closed floor opens again, and clears the card once someone starts", async () => {
    let up = false;
    const { shell } = await shown({ showDesk: vi.fn(() => up) });
    shell.go(desk("bao", "oldsite"));
    const card = shell.el.querySelector<HTMLElement>("[data-wake-card]")!;
    expect(card.hasAttribute("hidden")).toBe(false);
    expect(card.textContent).toContain("oldsite is closed. Starting Bao opens it again.");
    up = true;
    shell.el.querySelector<HTMLElement>("[data-wake]")!.click();
    await tick();
    expect(card.hasAttribute("hidden")).toBe(true);
  });
  it("says someone is not running on an open floor", async () => {
    const { shell } = await shown({ showDesk: vi.fn(() => false) });
    shell.go(desk("linh", "bytehive"));
    expect(shell.el.querySelector("[data-wake-card]")?.textContent).toContain("Linh is not running.");
  });
  it("shows why someone could not start", async () => {
    const { shell, source } = await shown({ showDesk: vi.fn(() => false) });
    vi.spyOn(source, "wakePerson").mockRejectedValue(new Error("No project is open."));
    shell.go(desk("bao", "oldsite"));
    shell.el.querySelector<HTMLElement>("[data-wake]")!.click();
    await tick();
    expect(shell.el.querySelector("[data-wake-card] [role=alert]")?.textContent).toBe("No project is open.");
    expect(shell.el.querySelector("[data-wake]")?.textContent).toBe("Start Bao");
  });
});

describe("its life", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); });

  it("opens where the caller says, at once", async () => {
    const { shell, host } = await shown();
    shell.hide();
    shell.show(desk("linh", "bytehive"));
    expect(host.showDesk).toHaveBeenLastCalledWith("linh", "terminal", true);
    expect(scene.setLevel).toHaveBeenLastCalledWith(desk("linh", "bytehive"), true);
    expect(document.body.classList.contains("office-desk")).toBe(true);
  });
  it("keeps the scene through hide and show, and stops watching", async () => {
    const { shell, host, source } = await shown();
    const watching = vi.spyOn(source, "watching");
    shell.go(desk("linh", "bytehive"));
    shell.hide();
    expect(shell.el.hasAttribute("hidden")).toBe(true);
    expect(scene.setVisible).toHaveBeenLastCalledWith(false);
    expect(watching).toHaveBeenLastCalledWith(false);
    expect(document.body.classList.contains("office-desk")).toBe(false);
    expect(tabs()).toEqual([]);
    shell.show();
    expect(watching).toHaveBeenLastCalledWith(true);
    expect(scene.setVisible).toHaveBeenLastCalledWith(true);
    expect(host.loadScene).toHaveBeenCalledTimes(1);
    expect(sceneModule.createScene).toHaveBeenCalledTimes(1);
  });
  it("makes no scene for a load that ends after it was hidden", async () => {
    let loaded = (_: never) => {};
    const { shell } = setup({ loadScene: vi.fn(() => new Promise<never>((r) => { loaded = r; })) });
    shell.show();
    shell.hide();
    loaded(sceneModule as never);
    await tick();
    expect(sceneModule.createScene).not.toHaveBeenCalled();
  });
  it("follows the reduced motion setting while it runs", async () => {
    const changes: Array<(e: { matches: boolean }) => void> = [];
    const media = vi.spyOn(window, "matchMedia").mockReturnValue({ matches: false, addEventListener: (_: string, fn: never) => changes.push(fn), removeEventListener() {} } as never);
    try {
      await shown();
      const options = sceneModule.createScene.mock.lastCall![1 as never] as unknown as { reducedMotion: boolean };
      expect(options.reducedMotion).toBe(false);
      changes.forEach((fn) => fn({ matches: true }));
      expect(options.reducedMotion).toBe(true);
    } finally { media.mockRestore(); }
  });
  it("stops drawing for a hidden window, not for an unfocused one", async () => {
    await shown();
    scene.setVisible.mockClear();
    const hidden = vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(scene.setVisible).toHaveBeenLastCalledWith(false);
    hidden.mockReturnValue(false);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(scene.setVisible).toHaveBeenLastCalledWith(true);
    scene.setVisible.mockClear();
    window.dispatchEvent(new Event("blur"));
    expect(scene.setVisible).not.toHaveBeenCalled();
    hidden.mockRestore();
  });
  it("frees everything on dispose", async () => {
    const off = vi.fn();
    scene.onMoved.mockReturnValueOnce(off as never);
    const { shell, source } = await shown();
    const redraw = vi.mocked(scene.setModel).mock.calls.length;
    shell.dispose();
    expect(off).toHaveBeenCalled();
    expect(scene.dispose).toHaveBeenCalled();
    expect(shell.el.isConnected).toBe(false);
    await source.approve("nothing");
    expect(vi.mocked(scene.setModel).mock.calls.length).toBe(redraw);
  });
  it("drops the scene and tells the host when it fails later", async () => {
    const { shell, host } = await shown();
    const canvas = shell.el.querySelector("canvas");
    shell.sceneFailed("lost");
    expect(scene.dispose).toHaveBeenCalled();
    expect(host.failed).toHaveBeenCalledWith("lost");
    expect(shell.el.querySelector("canvas")).not.toBe(canvas);
  });
  it("leaves for the Inbox on the button", async () => {
    const { shell, host } = await shown();
    shell.el.querySelector<HTMLElement>("[data-leave]")!.click();
    expect(host.leave).toHaveBeenCalled();
  });
  it("moves the keyboard to where you landed, and a redraw does not take it", async () => {
    const { shell, source } = await shown();
    shell.go(floor("bytehive"));
    expect((document.activeElement as HTMLElement).dataset.crumb).toBe("1");
    const box = shell.el.querySelector<HTMLTextAreaElement>("#ofMsg")!;
    box.focus();
    await source.approve("nothing");
    expect(document.activeElement).toBe(box);
  });
  it("does not make the current crumb a button", async () => {
    const { shell } = await shown();
    shell.go(floor("bytehive"));
    expect(shell.el.querySelector('button[data-crumb="1"]')).toBeNull();
    expect(shell.el.querySelector('[data-crumb="1"]')?.getAttribute("aria-current")).toBe("page");
    expect(shell.el.querySelector('button[data-crumb="0"]')).not.toBeNull();
  });
  it("tells the rail where it stands", async () => {
    const { shell } = await shown();
    const titles = () => shell.el.querySelectorAll("[data-needs] [data-row]").length;
    const atBuilding = titles();
    shell.go(floor("bytehive"));
    expect(titles()).toBeGreaterThan(atBuilding);
  });
});

describe("what a review of the shell found", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); scene.pick.mockReturnValue(null); });

  it("keeps a checklist item's done out of the page, whatever a board file holds", async () => {
    const { shell, source } = await shown();
    const item = { text: "t", done: `"><img src=x onerror=alert(1)>` as unknown as boolean };
    vi.spyOn(source, "report").mockResolvedValue({ name: "Linh", branch: null, cards: [{ title: "c", list: "Done", job: null, jobTitle: null, summary: null, files: [], checklist: [item], stopped: false }] });
    shell.go(desk("linh", "bytehive", "report"));
    await tick();
    await tick();
    const report = shell.el.querySelector(".of-report")!;
    expect(report.querySelector("img")).toBeNull();
    expect(report.querySelector("li")?.getAttribute("data-done")).toBe("true");
  });

  it("leaves the stage alone when a start lands after the owner left", async () => {
    let finish = () => {};
    const { shell, source, host } = await shown({ showDesk: vi.fn(() => false) });
    vi.spyOn(source, "wakePerson").mockImplementation(() => new Promise((r) => { finish = () => r(); }));
    shell.go(desk("bao", "oldsite"));
    shell.el.querySelector<HTMLElement>("[data-wake]")!.click();
    shell.hide();
    const before = vi.mocked(host.showDesk).mock.calls.length;
    finish();
    await tick();
    expect(host.showDesk).toHaveBeenCalledTimes(before);
  });

  it("keeps the keyboard on a tag when the tags change order", async () => {
    scene.anchors.mockReturnValue([anchor("bytehive", "floor", 10, 10), anchor("gcare", "floor", 10, 40)] as never);
    const { shell, source } = await shown();
    const tag = shell.el.querySelector<HTMLElement>('[data-tag="gcare"]')!;
    tag.focus();
    scene.anchors.mockReturnValue([anchor("bytehive", "floor", 10, 40), anchor("gcare", "floor", 10, 10)] as never);
    source.dismiss("nothing");
    expect(shell.el.querySelector("[data-tags]")!.firstElementChild).toBe(tag);
    expect(document.activeElement).toBe(tag);
  });

  it("takes the keyboard off a terminal when its Desk goes from under it", async () => {
    const { shell, source } = await shown();
    shell.go(desk("linh", "bytehive"));
    const box = document.createElement("div");
    box.className = "xterm";
    box.innerHTML = "<textarea></textarea>";
    document.body.append(box);
    box.querySelector("textarea")!.focus();
    const real = source.snapshot.bind(source);
    vi.spyOn(source, "snapshot").mockImplementation((now) => {
      const s = real(now);
      return { ...s, model: s.model && { ...s.model, floors: s.model.floors.map((f) => ({ ...f, people: f.people.filter((p) => p.id !== "linh") })) } };
    });
    source.dismiss("nothing");
    expect(shell.level().kind).toBe("floor");
    expect((document.activeElement as HTMLElement).closest(".xterm")).toBeNull();
    expect((document.activeElement as HTMLElement).closest("#office")).not.toBeNull();
  });
});

describe("who may take the keyboard at a Desk", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); scene.pick.mockReturnValue(null); });

  const terminal = () => {
    const box = document.createElement("div");
    box.className = "xterm";
    box.innerHTML = "<textarea></textarea>";
    document.body.append(box);
    return box.querySelector("textarea")!;
  };

  it("takes it off a terminal when the office comes back at a Report that covers it", async () => {
    const { shell } = await shown();
    shell.go(desk("linh", "bytehive", "report"));
    shell.hide();
    terminal().focus();
    shell.show();
    expect((document.activeElement as HTMLElement).closest(".xterm")).toBeNull();
    expect(document.activeElement?.getAttribute("aria-selected")).toBe("true");
  });
  it("leaves a terminal the Desk shows", async () => {
    const { shell } = await shown();
    shell.go(desk("linh", "bytehive"));
    shell.hide();
    const t = terminal();
    t.focus();
    shell.show();
    expect(document.activeElement).toBe(t);
  });
  it("says the owner did not ask when the drawer closed by itself", async () => {
    let open: string | null = "linh";
    const { shell, host, source } = await shown({ changesOpenFor: () => open });
    shell.go(desk("linh", "bytehive", "changes"));
    open = null;
    source.dismiss("nothing");
    expect(shell.level()).toEqual(desk("linh", "bytehive"));
    expect(host.showDesk).toHaveBeenLastCalledWith("linh", "terminal", false);
  });
  it("says the owner did not ask when a person turns up on a Desk that had no pane", async () => {
    let up = false;
    const showDesk = vi.fn(() => up);
    const { shell, source } = await shown({ showDesk });
    shell.go(desk("bao", "oldsite"));
    up = true;
    const real = source.snapshot.bind(source);
    vi.spyOn(source, "snapshot").mockImplementation((now) => {
      const s = real(now);
      return { ...s, model: s.model && { ...s.model, floors: s.model.floors.map((f) => ({ ...f, people: f.people.map((p) => (p.id === "bao" ? { ...p, paneId: "p9" } : p)) })) } };
    });
    source.dismiss("nothing");
    expect(showDesk).toHaveBeenLastCalledWith("bao", "terminal", false);
  });
  it("lets a started person take it: the owner pressed Start", async () => {
    let up = false;
    const showDesk = vi.fn(() => up);
    const { shell, source } = await shown({ showDesk });
    vi.spyOn(source, "wakePerson").mockImplementation(async () => { up = true; });
    shell.go(desk("bao", "oldsite"));
    shell.el.querySelector<HTMLElement>("[data-wake]")!.click();
    await tick();
    await tick();
    expect(showDesk).toHaveBeenLastCalledWith("bao", "terminal", true);
  });
});

describe("opening a project from the bar", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); });
  const button = (el: HTMLElement) => el.querySelector<HTMLElement>("[data-open-project]")!;

  it("offers it where the host can do it, and hands it the press", async () => {
    const openProject = vi.fn();
    const { shell } = await shown({ openProject });
    expect(button(shell.el).hidden).toBe(false);
    expect(button(shell.el).textContent).toBe("Open project");
    button(shell.el).click();
    expect(openProject).toHaveBeenCalledTimes(1);
  });
  it("says nothing where the host cannot", async () => {
    const { shell } = await shown();
    expect(button(shell.el).hidden).toBe(true);
  });
  it("is gone at a Desk: a new project would take the stage from the person there", async () => {
    const { shell } = await shown({ openProject: vi.fn() });
    shell.go(desk("linh", "bytehive"));
    expect(button(shell.el).hidden).toBe(true);
    shell.go(floor("bytehive"));
    expect(button(shell.el).hidden).toBe(false);
  });
});

describe("the doors in the bar", () => {
  beforeEach(() => { scene.anchors.mockReturnValue([] as never); });
  const doors = (el: HTMLElement) => ({
    jump: el.querySelector<HTMLElement>("[data-jump]")!,
    board: el.querySelector<HTMLElement>("[data-board-door]")!,
    agent: el.querySelector<HTMLElement>("[data-new-agent]")!,
  });

  it("offers each where the host can do it, and hands it the press", async () => {
    const openPalette = vi.fn(), openBoardTool = vi.fn(), newAgent = vi.fn();
    const { shell } = await shown({ openPalette, openBoardTool, newAgent });
    const d = doors(shell.el);
    expect([d.jump.hidden, d.board.hidden, d.agent.hidden]).toEqual([false, false, false]);
    d.jump.click(); d.board.click(); d.agent.click();
    expect([openPalette, openBoardTool, newAgent].map((f) => f.mock.calls.length)).toEqual([1, 1, 1]);
  });
  it("says nothing where the host cannot", async () => {
    const { shell } = await shown();
    const d = doors(shell.el);
    expect([d.jump.hidden, d.board.hidden, d.agent.hidden]).toEqual([true, true, true]);
  });
  it("keeps only Jump at a Desk, where the bar is narrow", async () => {
    const { shell } = await shown({ openPalette: vi.fn(), openBoardTool: vi.fn(), newAgent: vi.fn() });
    shell.go(desk("linh", "bytehive"));
    const d = doors(shell.el);
    expect([d.jump.hidden, d.board.hidden, d.agent.hidden]).toEqual([false, true, true]);
    shell.go(floor("bytehive"));
    expect([d.jump.hidden, d.board.hidden, d.agent.hidden]).toEqual([false, false, false]);
  });
});

describe("a shell that cannot be made", () => {
  it("leaves nothing listening: the rail is up when the job box fails", () => {
    const source = demoSource();
    const real = source.onChange.bind(source);
    let live = 0;
    vi.spyOn(source, "onChange").mockImplementation((fn) => {
      const off = real(fn);
      live += 1;
      return () => { live -= 1; off(); };
    });
    const snapshot = source.snapshot.bind(source);
    let calls = 0;
    vi.spyOn(source, "snapshot").mockImplementation((now) => {
      calls += 1;
      if (calls === 2) throw new Error("no snapshot");
      return snapshot(now);
    });
    const host = { showDesk: vi.fn(() => true), levelChanged: vi.fn(), leave: vi.fn(), failed: vi.fn(), loadScene: vi.fn() } as unknown as ShellHost;
    expect(() => createShell(source, host)).toThrow("no snapshot");
    expect(live).toBe(0);
  });
});
