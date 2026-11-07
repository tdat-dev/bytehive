// New agent (Agent Inbox): one short form instead of the crew picker. Say what
// the agent should do, pick the CLI, and how many agents take the same job
// (two or three is a race: compare their diffs, keep the best one).

import { CLI_PRESETS, CUSTOM_PREFIX, type CliPreset } from "./crew";
import { roomNote } from "./planmeter";
import { topNote } from "./hint";
import { activeWs } from "./appstate";
import { loadCrew, presetAvailable, refreshCliAvailability, saveTemplate, spawnAgents } from "./spawnmodal";
import { ICON_CLOSE } from "./icons";
import { getPref } from "./prefs";

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const LAST_CLI = "maestro.inbox.lastCli";
const LAST_CUSTOM = "maestro.inbox.lastCustom";
/** Radio value for the free-text command row (an AI CLI not listed above). */
const CUSTOM = "custom";

/** The coding CLIs worth offering here (shells stay in the old crew picker). */
export function agentPresets(): CliPreset[] {
  return CLI_PRESETS.filter((p) => !p.shell);
}

/** The button's words: "Start agent", "Start 3 agents". */
export function startLabel(count: number): string {
  return count > 1 ? `Start ${count} agents` : "Start agent";
}

let el: HTMLElement | null = null;
let returnTo: HTMLElement | null = null;

/** The CLI New agent starts on: the one set in Settings, or the last one used. */
function lastCli(): string {
  const set = getPref("defaultCli");
  if (set !== "last") return set;
  try { return localStorage.getItem(LAST_CLI) || "claude"; } catch { return "claude"; }
}

export function closeNewAgent(): void {
  el?.remove();
  el = null;
  returnTo?.focus();
  returnTo = null;
}

export function openNewAgent(opts: { count?: number } = {}): void {
  const preset = Math.min(3, Math.max(1, opts.count ?? getPref("defaultCount")));
  const ws = activeWs;
  if (!ws) {
    // With nothing open the owner stands on Start or in the office, and neither has a Home to press.
    const office = document.getElementById("office");
    topNote(office && !office.hidden ? "Open a project first: <b>Open project</b>, at the top of the office" : "Open a project first: pick a folder here");
    return;
  }
  el?.remove();
  returnTo = document.activeElement as HTMLElement | null;
  const presets = agentPresets();
  const last = lastCli();
  const chosen = last.startsWith(CUSTOM_PREFIX) ? CUSTOM : presets.some((p) => p.id === last) ? last : "claude";
  let lastCustom = "";
  try {
    lastCustom = last.startsWith(CUSTOM_PREFIX) ? last.slice(CUSTOM_PREFIX.length) : localStorage.getItem(LAST_CUSTOM) || "";
  } catch { /* private window */ }
  el = document.createElement("div");
  el.className = "inbox-modal-back";
  el.innerHTML = `
    <form class="inbox-modal na" role="dialog" aria-modal="true" aria-labelledby="naTitle">
      <header class="im-head">
        <div><h2 id="naTitle">New agent</h2><p class="im-sub">In ${esc(ws.name)}${ws.dir ? ` · <span class="im-mono">${esc(ws.dir)}</span>` : ""}</p></div>
        <button type="button" class="btn quiet icon im-x" data-close aria-label="Close">${ICON_CLOSE}</button>
      </header>
      <label class="im-label" for="naTask">What should it do?</label>
      <textarea id="naTask" rows="4" placeholder="Fix the flaky upload test and add a regression test. Leave empty to give it a job later."></textarea>
      <fieldset class="im-field">
        <legend class="im-label">Agent</legend>
        <div class="im-choices">${presets.map((p) => {
          const missing = !presetAvailable(p.program);
          return `<label class="im-choice${missing ? " missing" : ""}" title="${missing ? `${esc(p.program)} is not installed` : esc(p.program)}">
            <input type="radio" name="naCli" value="${esc(p.id)}"${p.id === chosen ? " checked" : ""}${missing ? " disabled" : ""}><span>${esc(p.label)}</span></label>`;
        }).join("")}<label class="im-choice" title="Any other AI command on your PATH"><input type="radio" name="naCli" value="custom"${chosen === CUSTOM ? " checked" : ""}><span>Custom…</span></label></div>
        <input id="naCustom" class="im-custom" placeholder="Command, e.g. ollama run llama3.1" value="${esc(lastCustom)}" aria-label="Custom command"${chosen === CUSTOM ? "" : " hidden"}>
      </fieldset>
      <p class="im-hint im-room" data-room hidden></p>
      <div class="im-row">
        <fieldset class="im-field">
          <legend class="im-label">How many</legend>
          <div class="im-seg">${[1, 2, 3].map((n) => `<label><input type="radio" name="naCount" value="${n}"${n === preset ? " checked" : ""}><span>${n}</span></label>`).join("")}</div>
        </fieldset>
        <p class="im-hint" data-race${preset > 1 ? "" : " hidden"}>They all get the same job. Compare their changes in Review and keep the best.</p>
      </div>
      <label class="im-check"><input type="checkbox" id="naSkip"${loadCrew().skipPerms ? " checked" : ""}> Don't ask before running commands or editing files</label>
      <footer class="im-foot">
        <button type="button" class="btn quiet" data-save title="Keep this setup so Settings → Sessions → Scheduled agents can start it at a set time">Save as preset</button>
        <span class="im-sp"></span>
        <button type="button" class="btn secondary" data-close>Cancel</button>
        <button type="submit" class="btn primary" data-start title="Start (Ctrl+Enter)" aria-keyshortcuts="Control+Enter">${startLabel(preset)}</button>
      </footer>
    </form>`;
  document.body.appendChild(el);
  const form = el.querySelector("form")!;
  const task = el.querySelector<HTMLTextAreaElement>("#naTask")!;
  const start = el.querySelector<HTMLButtonElement>("[data-start]")!;
  const count = () => Number(form.querySelector<HTMLInputElement>('input[name="naCount"]:checked')?.value ?? 1);
  // how much of the chosen CLI's plan is left, when little
  const room = () => {
    const cli = presets.find((p) => p.id === form.querySelector<HTMLInputElement>('input[name="naCli"]:checked')?.value);
    const note = cli ? roomNote(cli.badge, presets.filter((p) => presetAvailable(p.program))) : "";
    const n = el?.querySelector<HTMLElement>("[data-room]");
    if (n) { n.textContent = note; n.hidden = !note; }
  };
  room();
  form.addEventListener("change", (e) => {
    room();
    start.textContent = startLabel(count());
    el?.querySelector<HTMLElement>("[data-race]")?.toggleAttribute("hidden", count() < 2);
    const isCustom = form.querySelector<HTMLInputElement>('input[name="naCli"]:checked')?.value === CUSTOM;
    el?.querySelector<HTMLElement>("#naCustom")?.toggleAttribute("hidden", !isCustom);
    if (isCustom && (e.target as HTMLElement).closest?.('input[name="naCli"]')) {
      el?.querySelector<HTMLInputElement>("#naCustom")?.focus();
    }
  });
  el.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t === el || t.closest("[data-close]")) { closeNewAgent(); return; }
    if (t.closest("[data-save]")) {
      const picked = form.querySelector<HTMLInputElement>('input[name="naCli"]:checked')?.value;
      if (!picked) return;
      const n = count();
      const skip = el?.querySelector<HTMLInputElement>("#naSkip")?.checked ?? false;
      if (picked === CUSTOM) {
        const cmd = el?.querySelector<HTMLInputElement>("#naCustom")?.value.trim() ?? "";
        if (!cmd) { topNote("Type the command to run, e.g. <b>ollama run llama3.1</b>"); return; }
        saveTemplate(`${n > 1 ? `${n}× ` : ""}${cmd} · ${ws.name}`, {}, ws.dir ?? "", skip, cmd, n);
      } else {
        const label = agentPresets().find((p) => p.id === picked)?.label ?? picked;
        saveTemplate(`${n > 1 ? `${n}× ` : ""}${label} · ${ws.name}`, { [picked]: n }, ws.dir ?? "", skip);
      }
      const b = t.closest<HTMLButtonElement>("[data-save]")!;
      b.textContent = "Saved";
      b.disabled = true;
    }
  });
  el.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { e.stopPropagation(); closeNewAgent(); }
    // Ctrl+Enter starts from inside the text box.
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); form.requestSubmit(); }
  });
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    let cli = form.querySelector<HTMLInputElement>('input[name="naCli"]:checked')?.value;
    if (!cli) return;
    if (cli === CUSTOM) {
      const cmd = el?.querySelector<HTMLInputElement>("#naCustom")?.value.trim() ?? "";
      if (!cmd) { topNote("Type the command to run, e.g. <b>ollama run llama3.1</b>"); return; }
      cli = CUSTOM_PREFIX + cmd;
      try { localStorage.setItem(LAST_CUSTOM, cmd); } catch { /* private window */ }
    }
    try { localStorage.setItem(LAST_CLI, cli); } catch { /* private window */ }
    const job = task.value.trim() || null;
    const skip = el?.querySelector<HTMLInputElement>("#naSkip")?.checked ?? false;
    const n = count();
    returnTo = null; // the new agent takes the focus, not the dock button
    closeNewAgent();
    void spawnAgents(ws, cli, n, job, skip);
  });
  task.focus();
  // Grey out CLIs that aren't installed once the probe answers.
  void refreshCliAvailability().then(() => {
    el?.querySelectorAll<HTMLInputElement>('input[name="naCli"]').forEach((input) => {
      const p = presets.find((x) => x.id === input.value);
      const missing = !!p && !presetAvailable(p.program);
      input.disabled = missing;
      input.closest(".im-choice")?.classList.toggle("missing", missing);
      if (missing && input.checked) {
        input.checked = false;
        el?.querySelector<HTMLInputElement>('input[name="naCli"]:not(:disabled)')?.click();
      }
    });
  });
}
