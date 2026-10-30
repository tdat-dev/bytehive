// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
vi.mock("./terminal", () => ({ mountTerminal: () => ({}) }));
import { updateStoppedOverlay } from "./pane";
import type { Pane } from "./panetypes";

function fakePane(): Pane {
  const el = document.createElement("section");
  el.className = "pane";
  el.innerHTML = '<div class="pane-bar"></div><div class="term-host" data-host></div>';
  return {
    id: "a", el, color: "", running: false, spawnedAt: null, lastOutputAt: 0,
    lastInputAt: 0, attention: false, attentionClearedAt: 0, attentionNotified: false,
    spec: { name: "Ana", badge: "muse", cwd: "D:/app", program: "muse", args: [], color: "", mono: "" },
    term: {},
  } as unknown as Pane;
}

describe("updateStoppedOverlay", () => {
  it("covers a stopped terminal with a Resume card naming the exit", () => {
    const p = fakePane();
    p.exitCode = 1;
    p.el.classList.add("stopped");
    updateStoppedOverlay(p);
    const veil = p.el.querySelector(".pane-veil") as HTMLElement;
    expect(veil).not.toBeNull();
    expect(veil.querySelector(".pane-veil-name")!.textContent).toBe("Ana stopped");
    expect(veil.querySelector(".pane-veil-sub")!.textContent).toContain("exit code 1");
    expect(veil.querySelector("button")!.getAttribute("aria-label")).toBe("Resume Ana");
  });

  it("shows no exit code for a parked pane that never ran", () => {
    const p = fakePane();
    p.el.classList.add("stopped");
    updateStoppedOverlay(p);
    expect(p.el.querySelector(".pane-veil-sub")!.textContent).toBe("Resume to carry on");
  });

  it("clicking Resume restarts the agent in place", () => {
    const p = fakePane();
    p.restart = vi.fn(async () => {});
    p.el.classList.add("stopped");
    updateStoppedOverlay(p);
    (p.el.querySelector(".pane-resume") as HTMLButtonElement).click();
    expect(p.restart).toHaveBeenCalledOnce();
  });

  it("removes the veil once the pane runs again", () => {
    const p = fakePane();
    p.el.classList.add("stopped");
    updateStoppedOverlay(p);
    expect(p.el.querySelector(".pane-veil")).not.toBeNull();
    p.el.classList.remove("stopped");
    updateStoppedOverlay(p);
    expect(p.el.querySelector(".pane-veil")).toBeNull();
  });
});
