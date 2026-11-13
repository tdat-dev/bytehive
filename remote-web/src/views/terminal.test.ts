import { describe, expect, it, vi } from "vitest";
import { Terminal } from "@xterm/xterm";
import { keyRow, silenceQueries, termEvents } from "./terminal";

describe("terminal events", () => {
  it("writes bytes of its agent only, and clears on a redraw", () => {
    const term = { write: vi.fn(), reset: vi.fn() };
    const on = termEvents(term, "m:1", "a1");
    on({ peer: "m:1", agent: "b2", bytes: new Uint8Array([1]), reset: false });
    on({ peer: "m:2", agent: "a1", bytes: new Uint8Array([1]), reset: false });
    expect(term.write).not.toHaveBeenCalled();
    on({ peer: "m:1", agent: "a1", bytes: new Uint8Array([1]), reset: false });
    expect(term.write).toHaveBeenCalledTimes(1);
    on({ peer: "m:1", agent: "a1", bytes: new Uint8Array(), reset: true });
    expect(term.reset).toHaveBeenCalledTimes(1);
    expect(term.write).toHaveBeenCalledTimes(1);
  });
});

describe("the row of keys a phone keyboard doesn't have", () => {
  const press = (row: HTMLElement, label: string) => [...row.querySelectorAll("button")].find((b) => b.textContent === label)!.click();

  it("sends the raw sequence of each key", () => {
    const sent: string[] = [];
    const row = keyRow((k) => sent.push(k), () => false);
    for (const b of row.querySelectorAll("button")) b.click();
    expect([...row.querySelectorAll("button")].map((b) => b.textContent)).toEqual(["Esc", "Tab", "Up", "Down", "Left", "Right", "Enter", "Ctrl+C"]);
    expect(sent).toEqual(["\x1b", "\t", "\x1b[A", "\x1b[B", "\x1b[D", "\x1b[C", "\r", "\x03"]);
  });

  it("sends the arrows the way the program asked for them, like a real keyboard", () => {
    const sent: string[] = [];
    const row = keyRow((k) => sent.push(k), () => true);
    for (const label of ["Up", "Down", "Left", "Right", "Esc", "Enter"]) press(row, label);
    expect(sent).toEqual(["\x1bOA", "\x1bOB", "\x1bOD", "\x1bOC", "\x1b", "\r"]);
  });

  it("leaves the keyboard's focus in the terminal when a key is pressed", () => {
    const row = keyRow(() => {}, () => false);
    const down = new MouseEvent("mousedown", { cancelable: true, bubbles: true });
    row.querySelector("button")!.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
  });
});

describe("a passive viewer", () => {
  // Every query xterm would answer on its own; the answer would land in the agent's PTY as typed input.
  const queries: [string, string][] = [
    ["primary DA", "\x1b[c"],
    ["secondary DA", "\x1b[>c"],
    ["CPR", "\x1b[6n"],
    ["DSR status", "\x1b[5n"],
    ["DEC CPR", "\x1b[?6n"],
    ["window report", "\x1b[18t"],
    ["DECRQM ansi", "\x1b[4$p"],
    ["DECRQM dec", "\x1b[?25$p"],
    ["DECRQSS", "\x1bP$qm\x1b\\"],
    ["OSC 4 colour", "\x1b]4;1;?\x07"],
    ["OSC 10 colour", "\x1b]10;?\x07"],
    ["OSC 11 colour", "\x1b]11;?\x1b\\"],
    ["OSC 12 colour", "\x1b]12;?\x07"],
  ];
  const feed = (term: Terminal, data: string) => new Promise<void>((done) => term.write(data, done));

  it("is answered by xterm when nothing is swallowed (the test sees the replies)", async () => {
    const term = new Terminal({ cols: 80, rows: 24 });
    const sent = vi.fn();
    term.onData(sent);
    await feed(term, "\x1b[c\x1b[6n");
    expect(sent).toHaveBeenCalled();
  });

  it.each(queries)("sends nothing back for %s", async (_name, query) => {
    const term = new Terminal({ cols: 80, rows: 24 });
    silenceQueries(term);
    const sent = vi.fn();
    term.onData(sent);
    await feed(term, query);
    expect(sent).not.toHaveBeenCalled();
  });

  it("still takes typing", () => {
    const term = new Terminal({ cols: 80, rows: 24 });
    silenceQueries(term);
    const sent = vi.fn();
    term.onData(sent);
    term.input("x");
    expect(sent).toHaveBeenCalledWith("x");
  });
});
