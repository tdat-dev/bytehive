// @vitest-environment happy-dom
import { describe, it, expect, vi } from "vitest";
import { mountTerminal } from "./terminal";
import { setCalmForTest } from "./calm";

const bytes = (s: string) => new TextEncoder().encode(s);

function mount(hidden: boolean) {
  const div = document.createElement("div");
  if (hidden) div.style.display = "none";
  document.body.appendChild(div);
  return { div, term: mountTerminal(div, () => {}, () => {}, { webgl: false }) };
}

describe("terminal output and flow control", () => {
  it("calls back once the emulator has parsed a chunk", async () => {
    const { term } = mount(false);
    const parsed = vi.fn();
    term.write(bytes("hello\r\n"), parsed);
    expect(parsed).not.toHaveBeenCalled(); // xterm parses on its own schedule
    await vi.waitFor(() => expect(parsed).toHaveBeenCalledTimes(1));
    expect(term.snapshot(50)).toContain("hello");
    term.dispose();
  });

  it("drains a pane that is out of sight the same way, so its state never goes stale", async () => {
    const { term } = mount(true);
    const parsed = vi.fn();
    for (let i = 0; i < 20; i++) term.write(bytes(`line ${i}\r\n`), parsed);
    await vi.waitFor(() => expect(parsed).toHaveBeenCalledTimes(20));
    expect(term.snapshot(30)).toContain("line 19"); // what needs-you detection and task state read
    term.dispose();
  });

  it("still acks a chunk the filter swallowed whole", async () => {
    const { term } = mount(false);
    const parsed = vi.fn();
    term.write(new Uint8Array(0), parsed);
    await vi.waitFor(() => expect(parsed).toHaveBeenCalledTimes(1));
    term.dispose();
  });

  it("acks what the background pacer still held when the pane goes", () => {
    setCalmForTest(true); // the window is in the background: output waits for the next draw
    const { term } = mount(false);
    const parsed = vi.fn();
    term.write(bytes("held back"), parsed);
    term.dispose();
    setCalmForTest(false);
    expect(parsed).toHaveBeenCalledTimes(1);
  });

  it("does not leave the backend waiting when the terminal refuses the write", () => {
    const { term } = mount(false);
    term.dispose();
    const parsed = vi.fn();
    term.write(bytes("late"), parsed); // a chunk already in flight when the pane went
    expect(parsed).toHaveBeenCalled();
  });
});
