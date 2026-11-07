// The few keys a terminal keeps for itself, the ones Windows Terminal keeps.
// Everything else goes to the CLI (appkeys.ts holds the app back).

export type LocalKey = "copy" | "paste" | "find";

export function localTermKey(e: KeyboardEvent, hasSelection: boolean): LocalKey | null {
  // AltGr arrives as Ctrl+Alt: it types text.
  if (e.type !== "keydown" || !e.ctrlKey || e.altKey || e.metaKey) return null;
  const k = e.key.toLowerCase();
  if (k === "v") return "paste";
  if (k === "c") return e.shiftKey || hasSelection ? "copy" : null;
  return e.shiftKey && k === "f" ? "find" : null;
}
