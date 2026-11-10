// Win+Alt+N, the one key that crosses into the app from a focused terminal: the
// OS has it (hotkey.rs), so no CLI loses a key. The first press brings the window
// forward with the keyboard on the next one waiting; the next press hands the
// keys back to the terminal you were in.

import { allTasks } from "./tasks";
import { workspaces } from "./appstate";
import { getPref, onPrefs } from "./prefs";
import { hotkeySet, onHotkey } from "./ipc";
import { focusStage } from "./inbox";
import { topNote } from "./hint";
import type { Pane } from "./panetypes";

/** The terminal to go back to, while the keyboard is where the key put it. */
let back: { paneId: string | null } | null = null;

const shown = (el: HTMLElement | null) => (el && el.getClientRects().length ? el : null);

function terminalInFocus(): Pane | null {
  const at = document.activeElement as HTMLElement | null;
  if (!at?.closest(".xterm")) return null;
  for (const ws of workspaces.values()) for (const p of ws.panes.values()) if (p.el.contains(at)) return p;
  return null;
}

/** The next one waiting: Needs you while it is on screen (the office's rail, or the
 *  column the Inbox gets when the office cannot draw), else the agents' list. */
function waitingRow(): HTMLElement | null {
  const inOffice = [...document.querySelectorAll<HTMLElement>("[data-needs] button")].find((b) => shown(b));
  if (inOffice) return inOffice;
  const t = allTasks().find((x) => x.status.state === "needs");
  return t ? shown(document.querySelector<HTMLElement>(`.iq-row[data-id="${CSS.escape(t.paneId)}"]`)) : null;
}

/** With nobody waiting, the key still takes the keyboard out of the terminal. */
function restingRow(): HTMLElement | null {
  return shown(document.querySelector<HTMLElement>('.iq-row[aria-current="true"]'))
    ?? shown(document.querySelector<HTMLElement>(".iq-row"))
    ?? shown(document.querySelector<HTMLElement>(".of [data-leave]"));
}

function goBack(paneId: string | null): void {
  for (const ws of workspaces.values()) {
    const p = paneId ? ws.panes.get(paneId) : undefined;
    if (p) { p.term.focus(); return; }
  }
  focusStage();
}

/** The key was pressed; Rust has already brought the window forward. */
export function hotkeyPressed(): void {
  const stillThere = (document.activeElement as HTMLElement | null)?.closest(".iq-row, .of");
  if (back && stillThere) {
    const { paneId } = back;
    back = null;
    goBack(paneId);
    return;
  }
  const paneId = terminalInFocus()?.id ?? null;
  const row = waitingRow();
  if (!row) topNote("Nothing is waiting for you");
  const to = row ?? restingRow();
  if (!to) return;
  to.focus();
  back = { paneId };
}

/** Why the key isn't working, in words for Settings; empty when it is. */
let failure = "";
export const hotkeyFailure = (): string => failure;

let onFailure: () => void = () => {};
/** Be told when registering fails or starts working. */
export function onHotkeyFailure(fn: () => void): void { onFailure = fn; }

/** Register what Settings says now. */
export async function applyHotkey(): Promise<void> {
  const combo = getPref("hotkeyOn") ? getPref("hotkey") : null;
  try {
    await hotkeySet(combo);
    failure = "";
  } catch (e) {
    failure = (e as { Failed?: string } | null)?.Failed ?? String(e);
  }
  onFailure();
}

/** Main window only: the tray and the OS key belong to it. */
export function initHotkey(): void {
  void onHotkey(hotkeyPressed).catch(() => {});
  let was = "";
  const sync = () => {
    const now = `${getPref("hotkeyOn")}|${getPref("hotkey")}`;
    if (now === was) return;
    was = now;
    void applyHotkey();
  };
  onPrefs(sync);
  sync();
}
