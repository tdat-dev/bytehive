// Settings > Ingest: the switch and the classifier key. Everything else about
// the classifier is decided in Rust. The key is written once and never read
// back, so the field always opens empty and says whether one is stored.
import { failureOf } from "./ingest/provider";
import { ingestClassify, ingestProviderGet, ingestProviderSave } from "./ipc";
import { getPrefs, setPref } from "./prefs";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T | null;
const field = (id: string) => $<HTMLInputElement>(id);

const NO_KEY = "Without one, only the built-in rules run. It stays on this computer.";

function note(text: string, kind: "" | "ok" | "err" = ""): void {
  const el = $("ingNote");
  if (!el) return;
  el.textContent = text;
  el.className = "ing-note" + (kind ? " " + kind : "");
}

/** What a failed check means, in words that say what to do about it. */
export function keyProblem(e: unknown): string {
  const error = failureOf(e);
  if (/http_40[13]/.test(error)) return "That key was refused.";
  if (/timeout|network/.test(error)) return "Couldn't reach the classifier.";
  return "The classifier didn't answer.";
}

function showKey(hasKey: boolean): void {
  const key = field("ingKey");
  if (key) {
    key.value = "";
    key.placeholder = hasKey ? "Saved" : "";
  }
  note(hasKey ? "Saved on this computer." : NO_KEY);
}

export function syncIngestView(): void {
  const on = field("prefIngest");
  if (on) on.checked = getPrefs().ingest;
  void ingestProviderGet()
    .then((p) => showKey(p.hasKey))
    .catch(() => note(NO_KEY));
}

/** A new key is saved and tried at once, so there is nothing else to press. */
async function saveKey(): Promise<void> {
  const key = field("ingKey");
  const apiKey = key?.value.trim();
  if (!key || !apiKey) return;
  key.disabled = true;
  note("Checking...");
  try {
    await ingestProviderSave({ apiKey });
    key.value = "";
    key.placeholder = "Saved";
    await ingestClassify("cảm ơn nhé");
    note("Working.", "ok");
  } catch (e) {
    note(keyProblem(e), "err");
  } finally {
    key.disabled = false;
  }
}

/** Wire the section once. Call at startup. */
export function initIngestView(): void {
  field("prefIngest")?.addEventListener("change", (e) => {
    setPref("ingest", (e.target as HTMLInputElement).checked);
  });
  const key = field("ingKey");
  key?.addEventListener("change", () => void saveKey());
  key?.addEventListener("keydown", (e) => {
    if (e.key === "Enter") key.blur();
  });
}
