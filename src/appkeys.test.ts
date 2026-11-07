import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { onAppKey, terminalHasKeys } from "./appkeys";

const press = (key: string, init: KeyboardEventInit = {}, on: Element = document.body) => {
  const e = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
  on.dispatchEvent(e);
  return e;
};

describe("onAppKey", () => {
  let off: () => void = () => {};
  let terminal: HTMLTextAreaElement;
  let field: HTMLInputElement;

  beforeEach(() => {
    document.body.innerHTML = `<div class="xterm"><textarea class="xterm-helper-textarea"></textarea></div><input>`;
    terminal = document.querySelector("textarea")!;
    field = document.querySelector("input")!;
  });
  afterEach(() => { off(); off = () => {}; });

  it("gives no key to the app while a terminal has focus", () => {
    const seen = vi.fn();
    off = onAppKey(seen);
    terminal.focus();
    expect(terminalHasKeys()).toBe(true);
    for (const [key, init] of [["k", { ctrlKey: true }], [",", { ctrlKey: true }], ["Tab", { ctrlKey: true }], ["s", { altKey: true }], ["=", { ctrlKey: true }]] as const) press(key, init, terminal);
    expect(seen).not.toHaveBeenCalled();
  });

  it("leaves the key alive on its way to the terminal", () => {
    off = onAppKey(() => {});
    terminal.focus();
    const reached = vi.fn();
    terminal.addEventListener("keydown", reached);
    const e = press("k", { ctrlKey: true }, terminal);
    expect(reached).toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false);
  });

  it("gives every key to the app with focus anywhere else", () => {
    const seen = vi.fn();
    off = onAppKey(seen);
    field.focus();
    expect(terminalHasKeys()).toBe(false);
    press("k", { ctrlKey: true }, field);
    field.blur();
    press("k", { ctrlKey: true });
    expect(seen).toHaveBeenCalledTimes(2);
  });

  it("gives the keys back to the app on a click outside the terminal", () => {
    off = onAppKey(() => {});
    terminal.focus();
    terminal.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true }));
    expect(terminalHasKeys()).toBe(true);
    document.body.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true }));
    expect(terminalHasKeys()).toBe(false);
  });

  it("stops at the first handler that takes the key", () => {
    const later = vi.fn();
    const offFirst = onAppKey((e) => e.preventDefault());
    off = onAppKey(later);
    press("k", { ctrlKey: true });
    offFirst();
    expect(later).not.toHaveBeenCalled();
    press("k", { ctrlKey: true });
    expect(later).toHaveBeenCalledTimes(1);
  });
});
