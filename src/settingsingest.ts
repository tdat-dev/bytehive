// Settings > Ingest: the classifier endpoint and the gate's limits. The key is
// written once through Rust and never read back, so the field always opens
// empty and says whether one is stored.
import { ingestClassify, ingestProviderGet, ingestProviderSave, type IngestProvider } from "./ipc";
import { getPrefs, setPref } from "./prefs";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T | null;
const field = (id: string) => $<HTMLInputElement>(id);
const select = (id: string) => $<HTMLSelectElement>(id);
const button = (id: string) => $<HTMLButtonElement>(id);

function note(text: string, kind: "" | "ok" | "err" = ""): void {
  const el = $("ingNote");
  if (!el) return;
  el.textContent = text;
  el.className = "ing-note" + (kind ? " " + kind : "");
}

function show(p: IngestProvider): void {
  const base = field("ingBaseUrl");
  if (base) base.value = p.baseUrl;
  const model = field("ingModel");
  if (model) model.value = p.model;
  const mode = select("ingJsonMode");
  if (mode) mode.value = p.jsonMode;
  const key = field("ingKey");
  if (key) {
    key.value = "";
    key.placeholder = p.hasKey ? "Saved (leave empty to keep)" : "API key";
  }
}

function setNumber(id: string, value: number): void {
  const el = field(id);
  if (el && document.activeElement !== el) el.value = String(value);
}

function showLimits(): void {
  const p = getPrefs();
  setNumber("ingTtlDays", p.ingestTtlDays);
  setNumber("ingDailyCalls", p.ingestDailyCalls);
  setNumber("ingDailyTokens", p.ingestDailyTokens);
  setNumber("ingLoopMin", p.ingestLoopMin);
  const on = field("prefIngest");
  if (on) on.checked = p.ingest;
}

export function syncIngestView(): void {
  showLimits();
  void ingestProviderGet()
    .then(show)
    .catch((e) => note(String(e), "err"));
}

async function save(): Promise<boolean> {
  const btn = button("ingSave");
  if (btn) btn.disabled = true;
  try {
    const provider = await ingestProviderSave({
      baseUrl: field("ingBaseUrl")?.value.trim() ?? "",
      model: field("ingModel")?.value.trim() ?? "",
      apiKey: field("ingKey")?.value.trim() || undefined,
      jsonMode: (select("ingJsonMode")?.value as "schema" | "prompt" | undefined) ?? "schema",
    });
    show(provider);
    note("Saved.", "ok");
    return true;
  } catch (e) {
    note(String(e), "err");
    return false;
  } finally {
    if (btn) btn.disabled = false;
  }
}

async function test(): Promise<void> {
  const btn = button("ingTest");
  if (!btn) return;
  btn.disabled = true;
  note("Saving and testing...");
  try {
    if (!(await save())) return;
    const reply = await ingestClassify("cảm ơn nhé");
    note(`Answer: ${reply.content} (${reply.usage.in} in, ${reply.usage.out} out)`, "ok");
  } catch (e) {
    note(String(e), "err");
  } finally {
    btn.disabled = false;
  }
}

/** Wire the section once. Call at startup. */
export function initIngestView(): void {
  field("prefIngest")?.addEventListener("change", (e) => {
    setPref("ingest", (e.target as HTMLInputElement).checked);
  });
  const limits: Array<["ingTtlDays" | "ingDailyCalls" | "ingDailyTokens" | "ingLoopMin", "ingestTtlDays" | "ingestDailyCalls" | "ingestDailyTokens" | "ingestLoopMin"]> = [
    ["ingTtlDays", "ingestTtlDays"],
    ["ingDailyCalls", "ingestDailyCalls"],
    ["ingDailyTokens", "ingestDailyTokens"],
    ["ingLoopMin", "ingestLoopMin"],
  ];
  for (const [id, key] of limits) {
    field(id)?.addEventListener("change", () => {
      setPref(key, Number(field(id)?.value));
      showLimits();
    });
  }
  button("ingSave")?.addEventListener("click", () => void save());
  button("ingTest")?.addEventListener("click", () => void test());
}
