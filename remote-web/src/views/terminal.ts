// The real terminal, carried end to end. It opens at the agent's own size, so
// the screen on the computer isn't disturbed; "Fit to this screen" is a choice.

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import type { RemoteAgent } from "../../../src/remote/protocol";
import type { MachineSession } from "../session";
import { el } from "./ui";

// The same in both page themes: what agents draw in a terminal is made for a dark one.
const FONT = '"Geist Mono Variable", ui-monospace, "Cascadia Mono", Consolas, monospace';
const THEME = { background: "#0a0a0c", foreground: "#f4f4f5", cursor: "#f4f4f5", cursorAccent: "#0a0a0c", selectionBackground: "rgba(255,255,255,.22)" };

interface TermEvent { peer: string; agent: string; bytes: Uint8Array; reset: boolean }

/** What a terminal does with the events of the page: its own agent's bytes, and a redraw that clears it first. */
export function termEvents(term: { write(b: Uint8Array): void; reset(): void }, peer: string, agent: string) {
  return (e: TermEvent): void => {
    if (e.peer !== peer || e.agent !== agent) return;
    if (e.reset) term.reset();
    else term.write(e.bytes);
  };
}

/**
 * Keeps this viewer passive. The machine replays raw scrollback, and xterm answers the
 * queries in it (device attributes, cursor position, colours) through `onData`, which
 * would reach the agent's PTY as typed input. The real terminal on the computer already
 * answers them, so none is answered here. Setting a colour (OSC 4/10/11/12 with a value) still applies.
 */
export function silenceQueries(term: Pick<Terminal, "parser">): void {
  const { parser } = term;
  const swallow = () => true;
  parser.registerCsiHandler({ final: "c" }, swallow);
  parser.registerCsiHandler({ prefix: ">", final: "c" }, swallow);
  parser.registerCsiHandler({ final: "n" }, swallow);
  parser.registerCsiHandler({ prefix: "?", final: "n" }, swallow);
  parser.registerCsiHandler({ final: "t" }, swallow);
  parser.registerCsiHandler({ intermediates: "$", final: "p" }, swallow);
  parser.registerCsiHandler({ prefix: "?", intermediates: "$", final: "p" }, swallow);
  // DECRQSS, answered by xterm through its DCS handler.
  parser.registerDcsHandler({ intermediates: "$", final: "q" }, swallow);
  const isQuery = (data: string) => /(^|;)\?/.test(data);
  for (const osc of [4, 10, 11, 12]) parser.registerOscHandler(osc, isQuery);
}

/** The keys a phone keyboard doesn't have, and the raw sequence each one sends. */
const KEYS: [string, string][] = [
  ["Esc", "\x1b"], ["Tab", "\t"], ["Up", "\x1b[A"], ["Down", "\x1b[B"], ["Left", "\x1b[D"], ["Right", "\x1b[C"], ["Enter", "\r"], ["Ctrl+C", "\x03"],
];

/** A row of those keys. `appCursor`: the program asked for application cursor keys, so arrows go as a real keyboard's would. */
export function keyRow(send: (sequence: string) => void, appCursor: () => boolean): HTMLElement {
  const row = el("div", undefined, "term-keys");
  row.setAttribute("role", "toolbar");
  row.setAttribute("aria-label", "Keys for the terminal");
  for (const [label, sequence] of KEYS) {
    const b = el("button", label, "btn raised small");
    b.type = "button";
    // A press leaves the focus in the terminal, so a phone keeps its keyboard open.
    b.onmousedown = (e) => e.preventDefault();
    b.onclick = () => send(appCursor() ? sequence.replace(/^\x1b\[(?=[A-D]$)/, "\x1bO") : sequence);
    row.append(b);
  }
  return row;
}

/** Opens the agent's terminal in `root` and answers the function that closes it. */
export function openTerminal(root: HTMLElement, s: MachineSession, peer: string, a: RemoteAgent): () => void {
  const term = new Terminal({ cols: a.cols, rows: a.rows, cursorBlink: true, scrollback: 5000, fontFamily: FONT, fontSize: 13, theme: THEME });
  const fit = new FitAddon();
  term.loadAddon(fit);
  silenceQueries(term);

  const button = el("button", "Fit to this screen", "btn quiet small");
  button.type = "button";
  const size = el("span", undefined, "term-size");
  const strip = el("div", undefined, "term-strip");
  strip.append(size, button);
  const encoder = new TextEncoder();
  const row = keyRow((k) => s.sendKeys(a.id, encoder.encode(k)), () => term.modes.applicationCursorKeysMode);
  // A terminal at the agent's size can be wider than a phone: only this area scrolls sideways.
  const screen = el("div", undefined, "term-screen");
  root.append(strip, row, screen);
  term.open(screen);
  const sizeNow = () => {
    size.textContent = `${term.cols} × ${term.rows}${screen.scrollWidth > screen.clientWidth ? ", scroll sideways" : ""}`;
  };
  sizeNow();
  const resized = term.onResize(sizeNow);
  // A font that came later leaves the cells measured with the fallback: a change of font measures them again.
  let shown = true;
  void document.fonts?.load(`13px ${FONT.split(",")[0]}`).then(() => {
    if (!shown) return;
    term.options.fontFamily = "monospace";
    term.options.fontFamily = FONT;
  });

  const handle = termEvents(term, peer, a.id);
  const listener = (e: Event) => handle((e as CustomEvent<TermEvent>).detail);
  window.addEventListener("remote-term", listener);
  const keys = term.onData((d) => s.sendKeys(a.id, encoder.encode(d)));
  s.send({ type: "term.open", agent: a.id });

  // The only place the page asks the computer to resize the real terminal.
  button.onclick = () => {
    fit.fit();
    s.send({ type: "term.resize", agent: a.id, cols: term.cols, rows: term.rows });
  };

  return () => {
    shown = false;
    s.send({ type: "term.close", agent: a.id });
    window.removeEventListener("remote-term", listener);
    keys.dispose();
    resized.dispose();
    term.dispose();
    strip.remove();
    row.remove();
    screen.remove();
  };
}
