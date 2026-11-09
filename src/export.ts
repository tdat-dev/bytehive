// The hand-over to the native app (export.rs): every "maestro.*" entry this
// app keeps in the WebView's storage, written to ~/.bytehive/export/web-state.json
// at start, whenever it changed, and on the way out. The native app reads the
// fleet, settings and recent folders from it once, on its first start.

import { invoke } from "@tauri-apps/api/core";

/** The file's shape; the native app checks it before reading. */
export const EXPORT_FORMAT = 1;
const CHECK_MS = 30_000;

/** The stored entries to hand over: ours only, as stored (strings). */
export function collectState(storage: Pick<Storage, "length" | "key" | "getItem">): Record<string, string> {
  const out: Record<string, string> = {};
  for (let i = 0; i < storage.length; i++) {
    const k = storage.key(i);
    if (!k || !k.startsWith("maestro.")) continue;
    const v = storage.getItem(k);
    if (v !== null) out[k] = v;
  }
  return out;
}

let last = "";

async function exportNow(version: string): Promise<void> {
  const storage = collectState(localStorage);
  const body = JSON.stringify(storage);
  if (body === last) return;
  last = body;
  const state = JSON.stringify({ format: EXPORT_FORMAT, app: "ByteHive", version, exportedAt: Date.now(), storage });
  await invoke("export_web_state", { state }).catch((e) => {
    last = "";
    console.error(`[bytehive] export for the native app failed: ${e}`);
  });
}

/** Start handing over. Called once, after the session is restored. */
export function startExport(version: string): void {
  void exportNow(version);
  window.setInterval(() => void exportNow(version), CHECK_MS);
  window.addEventListener("beforeunload", () => void exportNow(version));
}
