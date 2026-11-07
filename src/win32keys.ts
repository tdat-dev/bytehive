// win32-input-mode. ConPTY opens each session asking for it (ESC[?9001h): from
// then on a key can go as the whole Windows key event, press and release,
//   ESC [ Vk ; Sc ; Uc ; Kd ; Cs ; Rc _
// Without it xterm has no bytes at all for Ctrl+Shift+K, Ctrl+Tab or Ctrl+comma,
// so a CLI could never bind them. Typed text still goes as text: what an IME or
// Unikey composes never was a key press, and a plain letter needs nothing more.

const LEFT_ALT = 0x02, LEFT_CTRL = 0x08, SHIFT = 0x10, NUMLOCK = 0x20, SCROLLLOCK = 0x40, CAPSLOCK = 0x80, ENHANCED = 0x100;

/** Keys Windows flags as extended. */
const ENHANCED_KEYS = new Set([
  "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown", "Insert", "Delete", "NumpadEnter", "NumpadDivide",
]);

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "AltGraph", "Meta", "CapsLock", "NumLock", "ScrollLock"]);

/** The character Windows gives a key that makes one, 0 for the others. */
function charOf(e: KeyboardEvent): number {
  const ctrl = e.ctrlKey;
  switch (e.key) {
    case "Enter": return ctrl ? 10 : 13;
    case "Backspace": return ctrl ? 127 : 8;
    case "Tab": return 9;
    case "Escape": return 27;
    case " ": return 32;
  }
  if ([...e.key].length !== 1) return 0;
  if (!ctrl) return e.key.codePointAt(0) ?? 0;
  // Ctrl makes a control character of a letter and of [ \ ], and nothing of the rest.
  const k = e.key.toUpperCase().charCodeAt(0);
  return k >= 0x40 && k <= 0x5f ? k - 0x40 : 0;
}

function stateOf(e: KeyboardEvent): number {
  return (e.altKey ? LEFT_ALT : 0) | (e.ctrlKey ? LEFT_CTRL : 0) | (e.shiftKey ? SHIFT : 0) |
    (e.getModifierState("NumLock") ? NUMLOCK : 0) | (e.getModifierState("ScrollLock") ? SCROLLLOCK : 0) | (e.getModifierState("CapsLock") ? CAPSLOCK : 0) |
    (ENHANCED_KEYS.has(e.code) ? ENHANCED : 0);
}

/** The sequence for this key, or null when it goes the usual way. No scan code:
 *  a browser doesn't give one, and conhost reads the virtual key. */
export function win32Key(e: KeyboardEvent, down: boolean): string | null {
  // An IME at work, and text a program injected (VK_PACKET).
  if (e.isComposing || e.keyCode === 229 || e.keyCode === 231) return null;
  if (MODIFIER_KEYS.has(e.key)) return null;
  const printable = [...e.key].length === 1;
  // Plain text, and AltGr, which Windows reports as Ctrl+Alt.
  if (printable && (!(e.ctrlKey || e.altKey) || (e.ctrlKey && e.altKey))) return null;
  return `\x1b[${e.keyCode};0;${charOf(e)};${down ? 1 : 0};${stateOf(e)};1_`;
}
