// src/company/panel.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";

const eng = vi.hoisted(() => ({
  view: null as unknown,
  error: null as string | null,
  approve: vi.fn(),
  giveJob: vi.fn(),
  finishJob: vi.fn(),
}));
vi.mock("./engine", () => ({
  companyView: () => eng.view,
  companyError: () => eng.error,
  companyAlerts: () => [],
  onCompanyChange: () => {},
  openPerson: () => {},
  approve: eng.approve,
  giveJob: eng.giveJob,
  finishJob: eng.finishJob,
  reject: vi.fn(),
  raiseCap: vi.fn(),
}));

import { createCompanyPanel } from "./panel";

const state = () => ({
  company: { version: 1, floors: [], people: [], limits: {}, nextJob: 2 },
  roles: [],
  jobs: [{ id: "J-1", title: "Add login", ask: "a", status: "awaiting-plan", plan: { summary: "Build it", cards: [], hires: [] }, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 1 }],
});
const mount = () => {
  const el = document.createElement("div");
  document.body.appendChild(el);
  createCompanyPanel().mount(el);
  return el;
};
const tick = () => new Promise((r) => setTimeout(r, 0));

describe("company panel", () => {
  beforeEach(() => { document.body.innerHTML = ""; eng.view = null; eng.error = null; eng.approve.mockReset(); eng.giveJob.mockReset(); eng.finishJob.mockReset(); });

  it("says it is starting, then shows a load failure", () => {
    const el = mount();
    expect(el.textContent).toContain("Starting...");
    expect(el.querySelector("[data-send]")?.getAttribute("aria-disabled")).toBe("true");
    eng.error = "bad file";
    const again = mount();
    expect(again.querySelector("[data-msg]")?.textContent).toContain("bad file");
  });

  it("shows why an action failed instead of leaving a rejection, and lets you try again", async () => {
    eng.view = state();
    eng.approve.mockRejectedValueOnce(new Error("The Founder could not start"));
    const el = mount();
    const btn = () => el.querySelector<HTMLButtonElement>('[data-act="approve"]')!;
    btn().click();
    await tick();
    expect(el.querySelector("[data-msg]")?.textContent).toBe("The Founder could not start");
    expect(btn().getAttribute("aria-disabled")).toBeNull();
  });

  it("says Working... on a busy button and keeps keyboard focus there, even when the action fails", async () => {
    eng.view = state();
    let fail!: (e: unknown) => void;
    eng.approve.mockReturnValue(new Promise<void>((_, no) => { fail = no; }));
    const el = mount();
    el.querySelector<HTMLButtonElement>('[data-act="approve"]')!.focus();
    el.querySelector<HTMLButtonElement>('[data-act="approve"]')!.click();
    const busy = el.querySelector<HTMLButtonElement>('[data-act="approve"]')!;
    expect(busy.textContent).toBe("Working...");
    expect(document.activeElement).toBe(busy);
    fail(new Error("The Founder could not start"));
    await tick();
    const back = el.querySelector<HTMLButtonElement>('[data-act="approve"]')!;
    expect(back.textContent).toBe("Approve");
    expect(document.activeElement).toBe(back);
    expect(el.querySelector("[data-msg]")?.textContent).toBe("The Founder could not start");
  });

  it("falls back to a plain sentence when the failure has no words", async () => {
    eng.view = state();
    eng.approve.mockRejectedValueOnce(new Error(""));
    const el = mount();
    el.querySelector<HTMLButtonElement>('[data-act="approve"]')!.click();
    await tick();
    expect(el.querySelector("[data-msg]")?.textContent).toBe("Something went wrong.");
  });

  it("ignores a second click while the first is still running", async () => {
    eng.view = state();
    let done!: () => void;
    eng.approve.mockReturnValue(new Promise<void>((r) => { done = r; }));
    const el = mount();
    const approveBtn = () => el.querySelector<HTMLButtonElement>('[data-act="approve"]')!;
    approveBtn().click();
    approveBtn().click(); // a busy button still takes the click: the guard is what stops it
    expect(approveBtn().getAttribute("aria-disabled")).toBe("true");
    expect(eng.approve).toHaveBeenCalledTimes(1);
    done();
    await tick();
    expect(approveBtn().getAttribute("aria-disabled")).toBeNull();
    approveBtn().click();
    expect(eng.approve).toHaveBeenCalledTimes(2);
  });

  it("gives a merge row Open and Mark done", async () => {
    const s = state();
    s.jobs[0] = { ...s.jobs[0], status: "awaiting-merge", summary: "Shipped", people: ["p1"] } as never;
    eng.view = s;
    eng.finishJob.mockResolvedValueOnce(undefined);
    const el = mount();
    const row = el.querySelector(".co-merge")!;
    expect(row.querySelector('[data-act="open"]')?.getAttribute("data-person")).toBe("p1");
    row.querySelector<HTMLButtonElement>('[data-act="finish"]')!.click();
    expect(eng.finishJob).toHaveBeenCalledWith("J-1");
  });

  it("escapes what jobs, roles and people write", () => {
    const evil = `<img src=x onerror=alert(1)>"'`;
    const s = state();
    s.jobs[0] = { ...s.jobs[0], title: evil, status: "awaiting-merge", summary: evil, people: ["p1"] } as never;
    (s.company as { people: unknown[]; floors: unknown[] }).floors = [{ id: "a", dir: "D:\a", name: evil }];
    (s.company as { people: unknown[] }).people = [{ id: "p1", name: evil, role: evil, rank: "staff", floor: "a", look: { shirt: "red;position:fixed" } }];
    eng.view = s;
    const el = mount();
    expect(el.querySelector("img")).toBeNull();
    expect(el.querySelector(".co-merge span")?.textContent).toBe(evil);
    expect(el.querySelector(".co-person")?.textContent).toContain(evil);
    expect(el.querySelector("h4")?.textContent).toContain("Top floor");
    expect(el.querySelector<HTMLElement>(".co-person i")?.getAttribute("style")).toBe("background:var(--idle)");
    expect(el.querySelector("[onerror]")).toBeNull();
  });

  it("keeps the job text when giving it fails, and asks why while a plan waits", async () => {
    eng.view = state();
    eng.giveJob.mockRejectedValueOnce("no Founder");
    const el = mount();
    const box = el.querySelector("textarea")!;
    expect(box.placeholder).toContain("Why send it back?");
    box.value = "Add search";
    el.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    await tick();
    expect(box.value).toBe("Add search");
    expect(el.querySelector("[data-msg]")?.textContent).toBe("no Founder");
  });

  it("says in plain words when Node is missing", async () => {
    eng.view = state();
    eng.giveJob.mockRejectedValueOnce(new Error("ByteHive needs Node.js to run the Company."));
    const el = mount();
    el.querySelector("textarea")!.value = "Add search";
    el.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    await tick();
    expect(el.querySelector("[data-msg]")?.textContent).toBe("ByteHive needs Node.js to run the Company.");
  });
});
