// @vitest-environment happy-dom
import { describe, it, expect, afterEach } from "vitest";
import { hiddenGate } from "./hiddengate";

const setHidden = (h: boolean) => Object.defineProperty(document, "hidden", { configurable: true, get: () => h });

describe("hiddenGate", () => {
  afterEach(() => setHidden(false));

  it("lets every tick through while shown, every nth while hidden, all again once back", () => {
    const gate = hiddenGate(3);
    setHidden(false);
    expect([gate(), gate()]).toEqual([true, true]);
    setHidden(true);
    expect([gate(), gate(), gate(), gate(), gate(), gate()]).toEqual([false, false, true, false, false, true]);
    setHidden(false);
    expect(gate()).toBe(true);
  });
});
