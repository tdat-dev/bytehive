// src/company/office/rail.test.ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { NeedRow } from "../needs";
import type { OfficeModel } from "./model";
import { BUILDING, type Level } from "./nav";
import { createJobBox, createRail, planSentence, sendsOnEnter, tokensShort, type RailOptions } from "./rail";
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
const mountRail = (s: OfficeSource, go = vi.fn(), where?: () => Level, o: RailOptions = { confirm: async () => true }) => {
  const el = document.createElement("div");
  document.body.appendChild(el);
  createRail(s, go, where, o).mount(el);
  return { el, go };
};
const founder = { id: "founder", name: "Founder", title: "Founder", shirt: "#eef0f4", floorId: null };
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
  it("opens the note box on Send back, and an empty note never reaches the job", async () => {
    const s = fake();
    const { el } = mountRail(s);
    const field = () => el.querySelector<HTMLElement>(".co-field")!;
    const note = () => el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!;
    expect(field().hidden).toBe(true);
    click(el, '[data-act="reject"]');
    expect(field().hidden).toBe(false);
    expect(document.activeElement).toBe(note());
    click(el, '[data-act="reject"]');
    click(el, '[data-act="reject"]');
    expect(s.reject).not.toHaveBeenCalled();
    expect(el.textContent).toContain("Send back needs a note.");
    await tick();
    expect(s.reject).not.toHaveBeenCalled();
  });
  it("takes spaces for an empty note", () => {
    const s = fake();
    const { el } = mountRail(s);
    el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value = "   ";
    click(el, '[data-act="reject"]');
    expect(s.reject).not.toHaveBeenCalled();
  });
  it("stops saying a note is needed when the owner starts writing one", () => {
    const s = fake();
    const { el } = mountRail(s);
    click(el, '[data-act="reject"]');
    click(el, '[data-act="reject"]');
    expect(el.querySelector(".co-field .co-hint")).not.toBeNull();
    const note = el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!;
    note.value = "F";
    note.dispatchEvent(new Event("input", { bubbles: true }));
    expect(el.querySelector(".co-field .co-hint")).toBeNull();
    expect(el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value).toBe("F");
  });
  it("clears the note once the plan went back, even with the row still there", async () => {
    const s = fake();
    const { el } = mountRail(s);
    const note = () => el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!;
    note().value = "Fewer people";
    click(el, '[data-act="reject"]');
    await tick();
    expect(note().value).toBe("");
    s.fire();
    expect(note().value).toBe("");
  });
  it("keeps what was typed while the plan was going back", async () => {
    let done = () => {};
    const s = fake();
    s.reject.mockImplementation(() => new Promise<void>((r) => { done = r; }));
    const { el } = mountRail(s);
    const note = () => el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!;
    note().value = "Fewer people";
    click(el, '[data-act="reject"]');
    note().value = "Fewer people, and no QA";
    done();
    await tick();
    expect(note().value).toBe("Fewer people, and no QA");
  });
  it("keeps the note when sending it back failed", async () => {
    const s = fake();
    s.reject.mockRejectedValue(new Error("offline"));
    const { el } = mountRail(s);
    el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value = "Fewer people";
    click(el, '[data-act="reject"]');
    await tick();
    expect(el.querySelector<HTMLTextAreaElement>('[data-note="J-1"]')!.value).toBe("Fewer people");
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
    const s = fake({ model: model([row], { founder: founder as never }) });
    const { el, go } = mountRail(s);
    click(el, "[data-row]");
    expect(go).toHaveBeenCalledWith({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" });
  });
  it("opens the changes of a job waiting to be merged", () => {
    const row: NeedRow = { key: "merge-J-1", kind: "merge", jobId: "J-1", personId: "p1", title: "Review and merge: Add login", detail: "Shipped" };
    const person = { id: "p1", name: "Ana", title: "QA", shirt: "#5fd08a", floorId: "a" } as never;
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
  const going = (status = "running", label = "Running") => ({ id: "J-2", title: "T", status, label, tokensUsed: 1, tokenCap: 2 });
  it("stops a Job from its menu, once the owner says yes", async () => {
    const s = fake({ model: model([], { jobs: [going()] as never }) });
    const { el } = mountRail(s);
    expect(el.textContent).toContain("Running");
    click(el, '[data-act="stop"]');
    await tick();
    expect(s.stopJob).toHaveBeenCalledWith("J-2");
  });
  it("leaves the Job alone when the owner says no", async () => {
    const s = fake({ model: model([], { jobs: [going()] as never }) });
    const { el } = mountRail(s, vi.fn(), undefined, { confirm: async () => false });
    click(el, '[data-act="stop"]');
    await tick();
    expect(s.stopJob).not.toHaveBeenCalled();
  });
  it("calls it Cancel job before anyone has started", () => {
    const { el } = mountRail(fake({ model: model([], { jobs: [going("awaiting-plan", "Waiting for your approval")] as never }) }));
    expect(el.querySelector('[data-act="stop"]')?.textContent).toBe("Cancel job");
  });
  it("keeps Stop in a menu, which opens from its button and closes on Esc with the keyboard back there", () => {
    const { el } = mountRail(fake({ model: model([], { jobs: [going()] as never }) }));
    const menu = () => el.querySelector<HTMLElement>(".co-menu")!;
    const opener = () => el.querySelector<HTMLElement>("[data-menu]")!;
    expect(menu().hidden).toBe(true);
    opener().click();
    expect(menu().hidden).toBe(false);
    expect(opener().getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(el.querySelector('[data-act="stop"]'));
    el.querySelector('[data-act="stop"]')!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(menu().hidden).toBe(true);
    expect(document.activeElement).toBe(opener());
  });
  it("draws a Job as one line with how far along it is, the tokens for whoever reads them", () => {
    const row = { ...going(), tokensUsed: 1_200_000, tokenCap: 5_000_000 };
    const { el } = mountRail(fake({ model: model([], { jobs: [row] as never }) }));
    expect(el.querySelector<HTMLElement>(".co-job")!.getAttribute("style")).toContain("--done:24%");
    expect(el.querySelector(".co-state")?.textContent).toBe("Running, 24%");
    expect(el.querySelector(".of-sr")?.textContent).toContain("1,200,000 of 5,000,000 tokens");
  });
  it("has no honey and no primary button when nothing needs you", () => {
    const { el } = mountRail(fake({ model: model([], { jobs: [going()] as never }) }));
    expect(el.querySelector(".btn.primary")).toBeNull();
    expect(el.querySelector(".co-n")?.textContent).toBe("");
  });
  it("gives the first thing that needs you the one primary button", () => {
    const rows: NeedRow[] = [planRow, { key: "paused-J-2", kind: "paused", jobId: "J-2", personId: null, title: "Paused: B", detail: "" }, { key: "merge-J-3", kind: "merge", jobId: "J-3", personId: null, title: "Merge C", detail: "" }];
    const { el } = mountRail(fake({ model: model(rows) }));
    const primary = [...el.querySelectorAll<HTMLElement>(".btn.primary")];
    expect(primary.map((b) => b.dataset.act)).toEqual(["approve"]);
  });
  it("reads the plan as a sentence, with the list folded under it", () => {
    const row: NeedRow = { ...planRow, plan: { hires: ["Developer on bytehive", "Copywriter on gcare"], floors: [{ floor: "bytehive", cards: ["a", "b"] }, { floor: "gcare", cards: ["c"] }], tokenCap: 5_000_000, newRoles: [] } };
    const { el } = mountRail(fake({ model: model([row]) }));
    expect(el.querySelector(".co-what")?.textContent).toBe("The Founder wants to hire a Developer on bytehive and a Copywriter on gcare, 3 cards, up to 5M tokens.");
    expect(el.querySelector<HTMLDetailsElement>("details")?.open).toBe(false);
  });
  it("leaves the details open when a plan writes a new role: the owner reads it word for word", () => {
    const row: NeedRow = { ...planRow, plan: { hires: [], floors: [], tokenCap: 1, newRoles: [{ id: "a", title: "A", department: "D", prompt: "p" } as never] } };
    const { el } = mountRail(fake({ model: model([row]) }));
    expect(el.querySelector<HTMLDetailsElement>("details")?.open).toBe(true);
  });
  it("shows a person's portrait, and until there is one their initial on their colour", () => {
    const person = { id: "p1", name: "ana", title: "QA", shirt: "#5fd08a", floorId: "a" } as never;
    const row: NeedRow = { key: "k", kind: "alert", jobId: null, personId: "p1", title: "Ana is stuck", detail: "" };
    const s = fake({ model: model([row], { floors: [{ id: "a", name: "gcare", people: [person] } as never] }) });
    let url: string | null = null;
    const { el } = mountRail(s, vi.fn(), undefined, { face: () => url });
    expect(el.querySelector("span.co-face")?.textContent).toBe("A");
    expect(el.querySelector("span.co-face")?.getAttribute("style")).toBe("--c:#5fd08a");
    url = "blob:face";
    s.fire();
    expect(el.querySelector<HTMLImageElement>("img.co-face")?.getAttribute("src")).toBe("blob:face");
  });
  it("lets no name or colour from a file into the page as markup", () => {
    const person = { id: "p1", name: `<img src=x onerror=alert(1)>`, title: `<b>x</b>`, shirt: `red;"><img src=x onerror=alert(1)>`, floorId: "a" } as never;
    const row: NeedRow = { key: "k", kind: "alert", jobId: null, personId: "p1", title: `<img src=x>`, detail: "" };
    const { el } = mountRail(fake({ model: model([row], { floors: [{ id: "a", name: `<i>`, people: [person] } as never] }) }));
    expect(el.querySelector("img")).toBeNull();
    expect(el.querySelector("[onerror]")).toBeNull();
    expect(el.querySelector(".co-who b")?.textContent).toBe(`<img src=x onerror=alert(1)>`);
    expect(el.querySelector("span.co-face")?.getAttribute("style")).toBe("--c:#252a37");
  });
  it("goes from an Open button too, not only the head of the card", () => {
    const row: NeedRow = { key: "k", kind: "alert", jobId: null, personId: "founder", title: "Founder stuck", detail: "" };
    const { el, go } = mountRail(fake({ model: model([row], { founder: founder as never }) }));
    click(el, "[data-open]");
    expect(go).toHaveBeenCalledWith({ kind: "desk", personId: "founder", floorId: null, tab: "terminal" });
  });
  it("says what the team is doing when nothing needs you", () => {
    const { el } = mountRail(fake({ model: model([], { counts: { people: 4, working: 3, needs: 0 } }) }));
    expect(el.textContent).toContain("3 people are working.");
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
    expect(el.querySelector(".co-merge .co-what")?.textContent).toBe(evil);
  });
  it("has BUILDING to go to for a row with nobody, from a floor", () => {
    const row: NeedRow = { key: "k", kind: "paused", jobId: "J-1", personId: null, title: "Paused", detail: "" };
    const { el, go } = mountRail(fake({ model: model([row]) }), vi.fn(), () => ({ kind: "floor", floorId: "a" }));
    click(el, "[data-row]");
    expect(go).toHaveBeenCalledWith(BUILDING);
  });
  it("draws a title that goes nowhere as plain text, not a button", () => {
    const row: NeedRow = { key: "k", kind: "paused", jobId: "J-1", personId: null, title: "Paused", detail: "" };
    const { el } = mountRail(fake({ model: model([row]) }));
    expect(el.querySelector("[data-row]")).toBeNull();
    expect(el.querySelector(".co-paused b")?.textContent).toBe("Paused");
  });
  it("draws the row of the desk you are at as plain text", () => {
    const row: NeedRow = { key: "k", kind: "alert", jobId: null, personId: "founder", title: "Founder stuck", detail: "" };
    const s = fake({ model: model([row], { founder: founder as never }) });
    let at: Level = BUILDING;
    const { el } = mountRail(s, vi.fn(), () => at);
    expect(el.querySelector("[data-row]")).not.toBeNull();
    at = { kind: "desk", personId: "founder", floorId: null, tab: "terminal" };
    s.fire();
    expect(el.querySelector("[data-row]")).toBeNull();
  });
  it("leaves the rows alone when nothing about them changed, so a click in progress lands", () => {
    const s = fake();
    const { el } = mountRail(s);
    const button = el.querySelector('[data-act="approve"]');
    s.fire();
    expect(el.querySelector('[data-act="approve"]')).toBe(button);
  });
  it("hears nothing more after it is disposed", () => {
    const s = fake();
    const el = document.createElement("div");
    const rail = createRail(s, vi.fn());
    rail.mount(el);
    rail.dispose();
    s.snap = { model: model([]), note: null, error: null };
    s.fire();
    expect(el.textContent).not.toContain("Nothing needs you.");
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
  const key = (over: KeyboardEventInit & { keyCode?: number } = {}) => {
    const e = new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true, ...over });
    if (over.keyCode !== undefined) Object.defineProperty(e, "keyCode", { value: over.keyCode });
    return e;
  };
  const typed = (el: HTMLElement, text: string) => { const box = el.querySelector("textarea")!; box.value = text; return box; };
  it("sends on Enter", async () => {
    const s = fake();
    const el = mountBox(s);
    typed(el, "Add search").dispatchEvent(key());
    await tick();
    expect(s.giveJob).toHaveBeenCalledWith("Add search");
  });
  it("starts a new line on Shift+Enter", () => {
    const s = fake();
    const el = mountBox(s);
    typed(el, "Add").dispatchEvent(key({ shiftKey: true }));
    expect(s.giveJob).not.toHaveBeenCalled();
  });
  it("leaves Enter to the IME while it is composing, or Vietnamese would send half a word", () => {
    const s = fake();
    const el = mountBox(s);
    typed(el, "Thêm trang gi").dispatchEvent(key({ isComposing: true }));
    typed(el, "Thêm trang gi").dispatchEvent(key({ keyCode: 229 }));
    expect(s.giveJob).not.toHaveBeenCalled();
  });
  it("knows the same rule wherever it is asked", () => {
    expect(sendsOnEnter(key())).toBe(true);
    expect(sendsOnEnter(key({ key: "a" }))).toBe(false);
    expect(sendsOnEnter(key({ isComposing: true }))).toBe(false);
  });
  it("puts an example in the box and leaves the sending to the owner", () => {
    const s = fake();
    const el = mountBox(s);
    click(el, "[data-chip]");
    expect(el.querySelector("textarea")!.value).toBe("Add a pricing page");
    expect(document.activeElement).toBe(el.querySelector("textarea"));
    expect(s.giveJob).not.toHaveBeenCalled();
    expect(el.querySelector<HTMLElement>("[data-chips]")!.hidden).toBe(true);
  });
  it("is honey only with something written and nothing else waiting", () => {
    const s = fake({ model: model([]) });
    const el = mountBox(s);
    const send = () => el.querySelector<HTMLElement>("[data-send]")!;
    expect(send().classList.contains("primary")).toBe(false);
    typed(el, "Do it").dispatchEvent(new Event("input", { bubbles: true }));
    expect(send().classList.contains("primary")).toBe(true);
    s.snap = { model: model(), note: null, error: null };
    s.fire();
    expect(send().classList.contains("primary")).toBe(false);
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

describe("the words of a plan", () => {
  it("says an, before a vowel", () => {
    expect(planSentence({ hires: ["Analyst on gcare"], floors: [], tokenCap: 1000, newRoles: [] })).toBe("The Founder wants to hire an Analyst on gcare, 0 cards, up to 1K tokens.");
  });
  it("says so when nobody new is needed", () => {
    expect(planSentence({ hires: [], floors: [{ floor: "a", cards: ["x"] }], tokenCap: 1_200_000, newRoles: [] })).toBe("The Founder wants to do this with the people you have, 1 card, up to 1.2M tokens.");
  });
  it("shortens tokens the way people say them", () => {
    expect(tokensShort(5_000_000)).toBe("5M");
    expect(tokensShort(1_250_000)).toBe("1.3M");
    expect(tokensShort(900)).toBe("900");
  });
});
