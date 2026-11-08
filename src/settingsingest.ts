// Settings > Ingest: the switch and the classifier key. Everything else about
// the classifier is decided in Rust. The key is written once and never read
// back, so the field always opens empty and says whether one is stored.
import { failureOf } from "./ingest/provider";
import { ingestProviderGet, ingestProviderSave } from "./ipc";
import { getPrefs, setPref } from "./prefs";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T | null;
const field = (id: string) => $<HTMLInputElement>(id);

const PURPOSE = "For messages the rules can't judge.";
const NO_KEY = `${PURPOSE} Without one, only the rules run.`;

function note(text: string, kind: "" | "ok" | "err" = ""): void {
  const el = $("ingNote");
  if (!el) return;
  el.textContent = text;
  el.className = "ing-note" + (kind ? " " + kind : "");
}

/** What a failed check means, in words that say what to do about it. */
export function keyProblem(e: unknown): string {
  const error = failureOf(e);
  if (/http_40[13]/.test(error)) return "That key was refused. Nothing was saved.";
  if (/timeout|network/.test(error)) return "Couldn't reach the classifier.";
  return "The classifier didn't answer.";
}

/** The saved line names where the key goes, so nobody has to guess. */
export const savedLine = (host: string): string => (host ? `${PURPOSE} Saved, sent only to ${host}.` : `${PURPOSE} Saved.`);

function showKey(p: { hasKey: boolean; host: string }): void {
  const key = field("ingKey");
  if (key) {
    key.value = "";
    key.placeholder = p.hasKey ? "Paste a new key to replace it" : "";
  }
  note(p.hasKey ? savedLine(p.host) : NO_KEY);
}

export function syncIngestView(): void {
  const on = field("prefIngest");
  if (on) on.checked = getPrefs().ingest;
  void ingestProviderGet()
    .then(showKey)
    .catch(() => note(NO_KEY));
}

/** A new key is tried, then kept, so there is nothing else to press. Rust
 *  keeps it only if the provider does not refuse it. */
async function saveKey(): Promise<void> {
  const key = field("ingKey");
  const apiKey = key?.value.trim();
  if (!key || !apiKey) return;
  key.disabled = true;
  note("Checking...");
  try {
    const p = await ingestProviderSave({ apiKey });
    key.placeholder = "Paste a new key to replace it";
    note(p.checked ? "Working." : "Saved, but the classifier couldn't be reached to check it.", p.checked ? "ok" : "");
  } catch (e) {
    note(keyProblem(e), "err");
  } finally {
    key.value = "";
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
