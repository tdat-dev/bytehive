// src/company/office/rail.test.ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { NeedRow } from "../needs";
import type { OfficeModel } from "./model";
import { BUILDING } from "./nav";
import { createJobBox, createRail } from "./rail";
import type { OfficeSource, Snapshot } from "./source";

const planRow: NeedRow = {
  key: "plan-J-1", kind: "plan", jobId: "J-1", personId: null, title: "Approve plan: Add login", detail: "Build it 0 cards, 0 hires.",
  plan: { hires: [], floors: [], tokenCap: 100, newRoles: [] },
};
const model = (needs: NeedRow[] = [planRow], over: Partial<OfficeModel> = {}): OfficeModel => ({
  founder: null, floors: [], needs, jobs: [], flights: [], counts: { people: 0, working: 0, needs: needs.length }, ...over,
});
function fake(snap: Partial<Snapshot> = {}) {
  const listeners = new Set<() => void>();
  const s = {
    snap: { model: model(), note: null, error: null, ...snap } as Snapshot,
    snapshot: () => s.snap,
    onChange: (fn: () => void) => { listeners.add(fn); return () => listeners.delete(fn); },
    fire: () => listeners.forEach((fn) => fn()),
    giveJob: vi.fn(async () => {}), messageFloor: vi.fn(async () => {}), approve: vi.fn(async () => {}), reject: vi.fn(async () => {}),
    raiseCap: vi.fn(async () => {}), finishJob: vi.fn(async () => {}), stopJob: vi.fn(async () => {}), dismiss: vi.fn(),
    showPerson: vi.fn(() => true), wakePerson: vi.fn(async () => {}), report: vi.fn(async () => null), watching: vi.fn(),
  };
  return s as typeof s & OfficeSource;
}
const mountRail = (s: OfficeSource, go = vi.fn()) => {
  const el = document.createElement("div");
  document.body.appendChild(el);
  createRail(s, go).mount(el);
  return { el, go };
};
const click = (el: HTMLElement, sel: string) => el.querySelector<HTMLElement>(sel)!.click();
const tick = () => new Promise((r) => setTimeout(r, 0));

describe("Needs you rail", () => {
  beforeEach(() => { document.body.innerHTML = ""; });

  it("says it is starting, then shows a load failure", () => {
    const s = fake({ model: null });
    const { el } = mountRail(s);
    expect(el.textContent).toContain("Starting...");
    s.snap = { model: null, note: null, error: "bad file" };
    s.fire();
    expect(el.querySelector("[data-msg]")?.textContent).toContain("bad file");
  });
  it("says where the Company runs, instead of Starting", () => {
    const { el } = mountRail(fake({ model: null, note: "The Company is running in another ByteHive window." }));
    expect(el.textContent).toContain("another ByteHive window");
    expect(el.textContent).not.toContain("Starting...");
  });
  it("says so when nothing needs you", () => {
    expect(mountRail(fake({ model: model([]) })).el.textContent).toContain("Nothing needs you.");
  });
  it("approves a plan, once, while it works", async () => {
    let done = () => {};
    const s = fake();
    s.approve.mockImplementation(() => new Promise<void>((r) => { done = r; }));
    const { el } = mountRail(s);
    click(el, '[data-act="approve"]');
    click(el, '[data-act="approve"]');
    expect(s.approve).toHaveBeenCalledTimes(1);
    expect(el.querySelector('[data-act="approve"]')?.getAttribute("aria-disabled")).toBe("true");
    done();
    await tick();
    expect(el.querySelector('[data-act="approve"]')?.getAttribute("aria-disabled")).toBeNull();
    click(el, '[data-act="approve"]');
    expect(s.approve).toHaveBeenCalledTimes(2);
  });
  it("sends a plan back with the note typed in its own row", async () => {
    const s = fake();
    const { el } = mountRail(s);
    el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value = "Fewer people";
    click(el, '[data-act="reject"]');
    expect(s.reject).toHaveBeenCalledWith("J-1", "Fewer people");
  });
  it("keeps a note while the rail redraws", () => {
    const s = fake();
    const { el } = mountRail(s);
    el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value = "half written";
    s.fire();
    expect(el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value).toBe("half written");
  });
  it("keeps the caret in a note through a redraw", () => {
    const s = fake();
    const { el } = mountRail(s);
    const note = () => el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!;
    note().value = "half written";
    note().focus();
    note().setSelectionRange(4, 4);
    s.fire();
    expect(document.activeElement).toBe(note());
    expect(note().selectionStart).toBe(4);
  });
  it("shows why an action failed", async () => {
    const s = fake();
    s.approve.mockRejectedValue(new Error("Codex could not load its settings."));
    const { el } = mountRail(s);
    click(el, '[data-act="approve"]');
    await tick();
    expect(el.querySelector("[data-msg]")?.textContent).toContain("Codex could not load");
    expect(el.querySelector('[data-act="approve"]')?.getAttribute("aria-disabled")).toBeNull();
  });
  it("falls back to a plain sentence when the failure has no words", async () => {
    const s = fake();
    s.approve.mockRejectedValue(new Error(""));
    const { el } = mountRail(s);
    click(el, '[data-act="approve"]');
    await tick();
    expect(el.querySelector("[data-msg]")?.textContent).toBe("Something went wrong.");
  });
  it("says Working... on a busy button and keeps keyboard focus there, even when the action fails", async () => {
    const s = fake();
    let fail!: (e: unknown) => void;
    s.approve.mockReturnValue(new Promise<void>((_, no) => { fail = no; }));
    const { el } = mountRail(s);
    el.querySelector<HTMLElement>('[data-act="approve"]')!.focus();
    click(el, '[data-act="approve"]');
    const busy = el.querySelector<HTMLElement>('[data-act="approve"]')!;
    expect(busy.textContent).toBe("Working...");
    expect(document.activeElement).toBe(busy);
    fail(new Error("The Founder could not start"));
    await tick();
    const back = el.querySelector<HTMLElement>('[data-act="approve"]')!;
    expect(back.textContent).toBe("Approve");
    expect(document.activeElement).toBe(back);
    expect(el.querySelector("[data-msg]")?.textContent).toBe("The Founder could not start");
  });
  it("shows the plan in full before you approve it, escaped", () => {
    const evil = `<img src=x onerror=alert(1)>`;
    const row: NeedRow = {
      ...planRow,
      plan: {
        hires: ["QA on gcare", "Analyst on gcare"], floors: [{ floor: "gcare", cards: ["Login page"] }], tokenCap: 1200000,
        newRoles: [{ id: "analyst", title: "Analyst", department: "Data", prompt: `Reads numbers.\nThen ${evil} says what they mean.` } as never],
      },
    };
    const plan = mountRail(fake({ model: model([row]) })).el.querySelector(".co-plan")!;
    expect(plan.textContent).toContain("QA on gcare");
    expect(plan.textContent).toContain("Analyst on gcare");
    expect(plan.textContent).toContain("Cards on gcare");
    expect(plan.textContent).toContain("Login page");
    expect(plan.textContent).toContain("1,200,000");
    expect(plan.textContent).toContain("New role: Analyst (analyst, Data)");
    expect(plan.querySelector(".co-prompt")?.textContent).toBe(`Reads numbers.\nThen ${evil} says what they mean.`);
    expect(plan.querySelector("img")).toBeNull();
  });
  it("flies to where a row points when the row is clicked", () => {
    const row: NeedRow = { key: "k", kind: "alert", jobId: null, personId: "founder", title: "Founder stuck", detail: "" };
    const s = fake({ model: model([row], { founder: { id: "founder", name: "Founder", floorId: null } as never }) });
    const { el, go } = mountRail(s);
    click(el, "[data-row]");
    expect(go).toHaveBeenCalledWith({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" });
  });
  it("opens the changes of a job waiting to be merged", () => {
    const row: NeedRow = { key: "merge-J-1", kind: "merge", jobId: "J-1", personId: "p1", title: "Review and merge: Add login", detail: "Shipped" };
    const person = { id: "p1", name: "Ana", floorId: "a" } as never;
    const { el, go } = mountRail(fake({ model: model([row], { floors: [{ id: "a", name: "gcare", people: [person] } as never] }) }));
    click(el, "[data-row]");
    expect(go).toHaveBeenCalledWith({ kind: "desk", personId: "p1", floorId: "a", tab: "changes" });
  });
  it("does not fly when a button in the row was the target", () => {
    const { el, go } = mountRail(fake());
    click(el, '[data-act="approve"]');
    expect(go).not.toHaveBeenCalled();
  });
  it("gives a merge row Mark done", () => {
    const row: NeedRow = { key: "merge-J-1", kind: "merge", jobId: "J-1", personId: null, title: "Review and merge: Add login", detail: "Shipped" };
    const s = fake({ model: model([row]) });
    const { el } = mountRail(s);
    click(el, ".co-merge [data-act=\"finish\"]");
    expect(s.finishJob).toHaveBeenCalledWith("J-1");
  });
  it("offers Dismiss on an alert that stands, and only there", () => {
    const rows: NeedRow[] = [
      { key: "file", kind: "alert", jobId: null, personId: null, title: "J-1.json could not be read.", detail: "", standing: true },
      { key: "stuck", kind: "alert", jobId: null, personId: "p1", title: "It is stuck.", detail: "" },
    ];
    const s = fake({ model: model(rows) });
    const { el } = mountRail(s);
    const buttons = [...el.querySelectorAll<HTMLElement>('[data-act="dismiss"]')];
    expect(buttons.map((b) => b.dataset.key)).toEqual(["file"]);
    buttons[0].click();
    expect(s.dismiss).toHaveBeenCalledWith("file");
  });
  it("adds to a paused Job's cap", () => {
    const row: NeedRow = { key: "paused-J-1", kind: "paused", jobId: "J-1", personId: null, title: "Paused: Add login", detail: "It reached its token cap." };
    const s = fake({ model: model([row]) });
    click(mountRail(s).el, '[data-act="raise"]');
    expect(s.raiseCap).toHaveBeenCalledWith("J-1", 1_000_000);
  });
  it("lists every Job still going with a Stop on each", () => {
    const row = (id: string, status: string, label: string) => ({ id, title: `Job ${id}`, status, label, tokensUsed: 1500, tokenCap: 5000000 });
    const jobs = [row("J-1", "running", "Running"), row("J-2", "paused", "Paused"), row("J-5", "planning", "Planning"), row("J-7", "awaiting-plan", "Waiting for your approval")] as never;
    const s = fake({ model: model([], { jobs }) });
    const { el } = mountRail(s);
    const list = el.querySelector("[data-jobs]")!;
    expect([...list.querySelectorAll<HTMLElement>('[data-act="stop"]')].map((b) => b.dataset.job)).toEqual(["J-1", "J-2", "J-5", "J-7"]);
    expect(list.textContent).toContain("1,500 of 5,000,000 tokens");
    expect(list.textContent).toContain("Paused");
    expect(list.textContent).toContain("J-5. Planning,");
    expect(list.textContent).toContain("J-7. Waiting for your approval,");
  });
  it("stops a Job from the Jobs list", () => {
    const s = fake({ model: model([], { jobs: [{ id: "J-2", title: "T", status: "running", label: "Running", tokensUsed: 1, tokenCap: 2 }] }) });
    const { el } = mountRail(s);
    expect(el.textContent).toContain("Running");
    click(el, '[data-act="stop"]');
    expect(s.stopJob).toHaveBeenCalledWith("J-2");
  });
  it("says when no job is running", () => {
    expect(mountRail(fake({ model: model([]) })).el.querySelector("[data-jobs]")?.textContent).toContain("No job is running.");
  });
  it("keeps keyboard focus on the same button through a redraw", () => {
    const s = fake();
    const { el } = mountRail(s);
    el.querySelector<HTMLElement>('[data-act="approve"]')!.focus();
    s.fire();
    expect((document.activeElement as HTMLElement).dataset.act).toBe("approve");
  });
  it("escapes what an agent wrote", () => {
    const evil = `<img src=x onerror=alert(1)>"'`;
    const row: NeedRow = { key: "k", kind: "merge", jobId: "J-1", personId: null, title: evil, detail: evil };
    const { el } = mountRail(fake({ model: model([row]) }));
    expect(el.querySelector("img")).toBeNull();
    expect(el.querySelector("[onerror]")).toBeNull();
    expect(el.querySelector(".co-merge span")?.textContent).toBe(evil);
  });
  it("has BUILDING to go to for a row with nobody", () => {
    const row: NeedRow = { key: "k", kind: "paused", jobId: "J-1", personId: null, title: "Paused", detail: "" };
    const { el, go } = mountRail(fake({ model: model([row]) }));
    click(el, "[data-row]");
    expect(go).toHaveBeenCalledWith(BUILDING);
  });
});

describe("job box", () => {
  beforeEach(() => { document.body.innerHTML = ""; });
  const mountBox = (s: OfficeSource) => {
    const el = document.createElement("div");
    document.body.appendChild(el);
    createJobBox(s).mount(el);
    return el;
  };
  const submit = (el: HTMLElement) => el.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true, bubbles: true }));

  it("gives the Founder a job and clears the box", async () => {
    const s = fake();
    const el = mountBox(s);
    el.querySelector("textarea")!.value = "Add marketing to gcare";
    submit(el);
    await tick();
    expect(s.giveJob).toHaveBeenCalledWith("Add marketing to gcare");
    expect(el.querySelector("textarea")!.value).toBe("");
  });
  it("keeps the words when the job is refused, and says why", async () => {
    const s = fake();
    s.giveJob.mockRejectedValue(new Error("Open a project first."));
    const el = mountBox(s);
    el.querySelector("textarea")!.value = "Do it";
    submit(el);
    await tick();
    expect(el.querySelector("textarea")!.value).toBe("Do it");
    expect(el.textContent).toContain("Open a project first.");
  });
  it("says in plain words when Node is missing", async () => {
    const needed = "ByteHive needs Node.js 22.13 or newer to run the Company. Node.js was not found. If you just installed it, restart ByteHive.";
    const s = fake();
    s.giveJob.mockRejectedValue(new Error(needed));
    const el = mountBox(s);
    el.querySelector("textarea")!.value = "Add search";
    submit(el);
    await tick();
    expect(el.querySelector("[data-msg]")?.textContent).toBe(needed);
  });
  it("sends nothing for an empty box", () => {
    const s = fake();
    const el = mountBox(s);
    submit(el);
    expect(s.giveJob).not.toHaveBeenCalled();
  });
  it("sends nothing while the Company is not running here", () => {
    const s = fake({ model: null });
    const el = mountBox(s);
    expect(el.querySelector("[data-send]")?.getAttribute("aria-disabled")).toBe("true");
    el.querySelector("textarea")!.value = "Do it";
    submit(el);
    expect(s.giveJob).not.toHaveBeenCalled();
  });
});
