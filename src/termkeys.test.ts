import { describe, it, expect } from "vitest";
import { localTermKey } from "./termkeys";

const key = (k: string, mods: Partial<Pick<KeyboardEvent, "ctrlKey" | "shiftKey" | "altKey" | "metaKey">> = {}, type = "keydown") =>
  ({ type, key: k, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods }) as KeyboardEvent;

describe("localTermKey", () => {
  it("keeps what Windows Terminal keeps", () => {
    expect(localTermKey(key("c", { ctrlKey: true }), true)).toBe("copy");
    expect(localTermKey(key("C", { ctrlKey: true, shiftKey: true }), false)).toBe("copy");
    expect(localTermKey(key("V", { ctrlKey: true, shiftKey: true }), false)).toBe("paste");
    expect(localTermKey(key("v", { ctrlKey: true }), false)).toBe("paste");
    expect(localTermKey(key("F", { ctrlKey: true, shiftKey: true }), false)).toBe("find");
  });

  it("sends Ctrl+C to the CLI when nothing is selected", () => {
    expect(localTermKey(key("c", { ctrlKey: true }), false)).toBeNull();
  });

  it("keeps nothing else", () => {
    const combos: Array<[string, Parameters<typeof key>[1]]> = [
      ["k", { ctrlKey: true }], [",", { ctrlKey: true }], ["Tab", { ctrlKey: true }], ["Tab", { ctrlKey: true, shiftKey: true }],
      ["K", { ctrlKey: true, shiftKey: true }], ["J", { ctrlKey: true, shiftKey: true }], ["D", { ctrlKey: true, shiftKey: true }],
      ["O", { ctrlKey: true, shiftKey: true }], ["T", { ctrlKey: true, shiftKey: true }], ["B", { ctrlKey: true, shiftKey: true }],
      ["1", { altKey: true }], ["s", { altKey: true }], ["p", { altKey: true }], ["ArrowUp", { altKey: true }],
      ["=", { ctrlKey: true }], ["-", { ctrlKey: true }], ["0", { ctrlKey: true }], ["f", { ctrlKey: true }], ["Escape", {}], ["a", {}],
    ];
    for (const [k, mods] of combos) expect(localTermKey(key(k, mods), true)).toBeNull();
  });

  it("leaves AltGr, which Windows reports as Ctrl+Alt, as text", () => {
    expect(localTermKey(key("v", { ctrlKey: true, altKey: true }), false)).toBeNull();
    expect(localTermKey(key("c", { ctrlKey: true, altKey: true }), true)).toBeNull();
  });

  it("acts on the press alone", () => {
    expect(localTermKey(key("v", { ctrlKey: true }, "keyup"), false)).toBeNull();
  });
});
