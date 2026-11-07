// Who gets a key press. The owner's rule: a focused terminal is the CLI's, so
// no app shortcut sits over it. Every shortcut registers here and the one
// listener below returns at once while focus is inside a terminal. It never
// stops the event: xterm reads it further down. What a terminal keeps for
// itself (Windows Terminal's list) lives in termkeys.ts.

type AppKey = (e: KeyboardEvent) => void;

const handlers: AppKey[] = [];
let listening = false;

/** Something stands over the app: no shortcut acts behind it. */
export function dialogUp(): boolean {
  return !!document.querySelector(".backdrop.open, .inbox-modal-back, .inbox-pal, .sm-pop, .cm-menu");
}

/** Ctrl+Shift+K deletes the line in the code editor, and L selects the matches:
 *  shortcuts run first, so they step aside there. */
export function editorKeeps(e: KeyboardEvent): boolean {
  const k = e.key.toLowerCase();
  return (k === "k" || k === "l") && !!document.activeElement?.closest(".cm-editor");
}

/** Focus is inside a terminal, so every key is its CLI's. */
export function terminalHasKeys(): boolean {
  return !!document.activeElement?.closest(".xterm");
}

function route(e: KeyboardEvent): void {
  if (terminalHasKeys()) return;
  for (const fn of [...handlers]) {
    fn(e);
    if (e.defaultPrevented) return;
  }
}

/** The mouse is the plain way out: a click outside the terminal gives the keys
 *  back to the app, even on something that keeps focus from moving. */
function leaveTerminal(e: PointerEvent): void {
  if (!terminalHasKeys() || (e.target as Element | null)?.closest?.(".xterm")) return;
  (document.activeElement as HTMLElement).blur();
}

/** Run `fn` for keys pressed outside a terminal, until the returned function is
 *  called. The first handler to preventDefault takes the key. */
export function onAppKey(fn: AppKey): () => void {
  if (!listening) {
    listening = true;
    window.addEventListener("keydown", route, true);
    window.addEventListener("pointerdown", leaveTerminal, true);
  }
  handlers.push(fn);
  return () => {
    const i = handlers.indexOf(fn);
    if (i >= 0) handlers.splice(i, 1);
  };
}
