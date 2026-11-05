// src/company/panel.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";

const eng = vi.hoisted(() => ({
  view: null as unknown,
  error: null as string | null,
  approve: vi.fn(),
  giveJob: vi.fn(),
}));
vi.mock("./engine", () => ({
  companyView: () => eng.view,
  companyError: () => eng.error,
  companyAlerts: () => [],
  onCompanyChange: () => {},
  openPerson: () => {},
  approve: eng.approve,
  giveJob: eng.giveJob,
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
  beforeEach(() => { document.body.innerHTML = ""; eng.view = null; eng.error = null; eng.approve.mockReset(); eng.giveJob.mockReset(); });

  it("says it is starting, then shows a load failure", () => {
    const el = mount();
    expect(el.textContent).toContain("Starting...");
    expect((el.querySelector("[data-send]") as HTMLButtonElement).disabled).toBe(true);
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
    expect(btn().disabled).toBe(false);
  });

  it("ignores a second click while the first is still running", async () => {
    eng.view = state();
    let done!: () => void;
    eng.approve.mockReturnValue(new Promise<void>((r) => { done = r; }));
    const el = mount();
    const btn = el.querySelector<HTMLButtonElement>('[data-act="approve"]')!;
    btn.click();
    el.querySelector<HTMLButtonElement>('[data-act="approve"]')!.click();
    expect(eng.approve).toHaveBeenCalledTimes(1);
    expect(el.querySelector<HTMLButtonElement>('[data-act="approve"]')!.disabled).toBe(true);
    done();
    await tick();
    expect(el.querySelector<HTMLButtonElement>('[data-act="approve"]')!.disabled).toBe(false);
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
});
