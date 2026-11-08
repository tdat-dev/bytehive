// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { showDelegationToast } from "./delegation";

describe("showDelegationToast", () => {
  it("shows no letter tile for a sender that is not an agent", () => {
    showDelegationToast("You", ["Bob"], "held back “thanks”");
    const av = document.querySelector<HTMLElement>(".deleg-toast-av");
    expect(av?.hidden).toBe(true);
    expect(document.querySelector(".deleg-toast-text")?.textContent).toContain("held back “thanks”");
  });

  it("carries one button that runs once and closes the toast", () => {
    const run = vi.fn();
    showDelegationToast("You", ["Bob"], "held back “thanks”", { label: "Send anyway", run });
    const act = document.querySelector<HTMLButtonElement>(".deleg-toast-act");
    expect(act?.hidden).toBe(false);
    expect(act?.textContent).toBe("Send anyway");
    act?.click();
    expect(run).toHaveBeenCalledTimes(1);
    expect(document.querySelector(".deleg-toast")?.classList.contains("on")).toBe(false);
  });

  it("drops the button when the next toast has none", () => {
    showDelegationToast("You", ["Bob"], "held back “thanks”", { label: "Send anyway", run: () => {} });
    showDelegationToast("You", ["Bob"], "sent.");
    expect(document.querySelector<HTMLButtonElement>(".deleg-toast-act")?.hidden).toBe(true);
  });
});
