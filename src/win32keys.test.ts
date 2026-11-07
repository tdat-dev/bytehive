import { describe, it, expect } from "vitest";
import { win32Key } from "./win32keys";

const key = (k: string, keyCode: number, over: Partial<KeyboardEvent> & { locks?: string[] } = {}) => {
  const { locks = [], ...rest } = over;
  return { key: k, code: "", keyCode, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, isComposing: false, getModifierState: (m: string) => locks.includes(m), ...rest } as unknown as KeyboardEvent;
};

describe("win32Key", () => {
  it("writes the key event the way Windows Terminal does", () => {
    expect(win32Key(key("k", 75, { ctrlKey: true }), true)).toBe("\x1b[75;0;11;1;8;1_");
    expect(win32Key(key("k", 75, { ctrlKey: true }), false)).toBe("\x1b[75;0;11;0;8;1_");
  });
  it("gives keys xterm has no bytes for a whole event", () => {
    expect(win32Key(key("K", 75, { ctrlKey: true, shiftKey: true }), true)).toBe("\x1b[75;0;11;1;24;1_");
    expect(win32Key(key("Tab", 9, { ctrlKey: true }), true)).toBe("\x1b[9;0;9;1;8;1_");
    expect(win32Key(key(",", 188, { ctrlKey: true }), true)).toBe("\x1b[188;0;0;1;8;1_");
    expect(win32Key(key("=", 187, { ctrlKey: true }), true)).toBe("\x1b[187;0;0;1;8;1_");
  });
  it("makes Ctrl+C the character conhost turns into an interrupt", () => {
    expect(win32Key(key("c", 67, { ctrlKey: true }), true)).toBe("\x1b[67;0;3;1;8;1_");
  });
  it("tells Enter and Backspace from their Ctrl forms, and Shift+Enter from Enter", () => {
    expect(win32Key(key("Enter", 13), true)).toBe("\x1b[13;0;13;1;0;1_");
    expect(win32Key(key("Enter", 13, { shiftKey: true }), true)).toBe("\x1b[13;0;13;1;16;1_");
    expect(win32Key(key("Enter", 13, { ctrlKey: true }), true)).toBe("\x1b[13;0;10;1;8;1_");
    expect(win32Key(key("Backspace", 8, { ctrlKey: true }), true)).toBe("\x1b[8;0;127;1;8;1_");
  });
  it("keeps the letter on an Alt key", () => {
    expect(win32Key(key("p", 80, { altKey: true }), true)).toBe("\x1b[80;0;112;1;2;1_");
  });
  it("flags the extended keys and the locks", () => {
    expect(win32Key(key("ArrowUp", 38, { code: "ArrowUp", altKey: true }), true)).toBe("\x1b[38;0;0;1;258;1_");
    expect(win32Key(key("F5", 116, { locks: ["NumLock"] }), true)).toBe("\x1b[116;0;0;1;32;1_");
  });
  it("leaves typed text as text", () => {
    expect(win32Key(key("a", 65), true)).toBeNull();
    expect(win32Key(key("A", 65, { shiftKey: true }), true)).toBeNull();
    expect(win32Key(key("?", 191, { shiftKey: true }), true)).toBeNull();
    expect(win32Key(key("ă", 0), true)).toBeNull();
  });
  it("leaves an IME, injected text and AltGr to xterm", () => {
    expect(win32Key(key("Process", 229), true)).toBeNull();
    expect(win32Key(key("a", 65, { isComposing: true }), true)).toBeNull();
    expect(win32Key(key("ê", 231), true)).toBeNull();
    expect(win32Key(key("€", 69, { ctrlKey: true, altKey: true }), true)).toBeNull();
  });
  it("sends nothing for a modifier on its own", () => {
    for (const [k, c] of [["Control", 17], ["Shift", 16], ["Alt", 18]] as const) expect(win32Key(key(k, c), true)).toBeNull();
  });
});
