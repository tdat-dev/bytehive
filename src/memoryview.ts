// Project memory, in ByteHive: every fact the project's agents share
// (projectmemory.ts), to read, add, edit and delete by hand, and to send to a
// running agent of the project to check against the code and fix. Agents
// change the same file through the bytehive MCP tools; the panel follows them.

import { fsCreateDir, fsCreateFile, fsReadFile, fsStat, fsWriteFile } from "./ipc";
import { addFact, deleteFact, parseMemory, serializeMemory, updateFact, MemoryError, type MemoryEntry, type ProjectMemory } from "./projectmemory";

const FILE = ".bytehive\\memory.json";
/** How often an open panel looks for an agent's change. */
const FOLLOW_MS = 1500;

export interface MemoryAgent { id: string; name: string }
export interface MemoryViewOpts {
  dir: string;
  project: string;
  /** The project's running agents, the one on screen first. */
  agents: () => MemoryAgent[];
  /** Types a message into an agent's terminal and presses Enter. */
  send: (agentId: string, text: string) => Promise<boolean>;
  /** Every project with a memory, to switch between them from the panel. */
  projects?: { id: string; name: string }[];
  /** The project shown, by id (one of `projects`). */
  current?: string;
  /** Show another project's memory in place of this one. */
  switchTo?: (id: string) => void;
}

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const oneLine = (s: string) => s.replace(/\s+/g, " ").trim();

/** What an agent is asked to do with one fact, or with all of them. In
 *  English, like every hand-off, on one line so the CLI takes it whole. */
export function fixPrompt(entry: MemoryEntry | null, note: string): string {
  const said = oneLine(note) ? ` The user says: ${oneLine(note)}.` : "";
  if (entry) {
    return `Fix this entry of the project memory, [${entry.id}]: "${oneLine(entry.text)}".${said} Check it against the code, then call memory_update with id ${entry.id} and the corrected fact, or memory_delete if it no longer holds. Reply in one line with what you changed.`;
  }
  return `Review this project's shared memory: read it with memory_get.${said} Check each fact against the code. Fix wrong or stale facts with memory_update, delete ones that no longer hold with memory_delete, merge duplicates, and save important missing facts with memory_save. Reply with a short list of what you changed.`;
}

async function load(dir: string): Promise<{ mem: ProjectMemory; mtime: number | null }> {
  try {
    const f = await fsReadFile(dir, FILE);
    return { mem: parseMemory(f.content), mtime: f.mtime };
  } catch (e) {
    if (e instanceof MemoryError) throw e;
    return { mem: parseMemory(""), mtime: null }; // no file yet
  }
}

/** Read, change, write, without overwriting an agent's change made in
 *  between: a conflict reads again and redoes the change once. */
async function change<T>(dir: string, fn: (m: ProjectMemory) => T): Promise<{ result: T; mem: ProjectMemory; mtime: number }> {
  try { await fsCreateDir(dir, ".bytehive"); } catch { /* already there */ }
  for (let attempt = 0; ; attempt++) {
    const { mem, mtime } = await load(dir);
    const result = fn(mem);
    if (mtime === null) { try { await fsCreateFile(dir, FILE); } catch { /* already there */ } }
    try {
      const w = await fsWriteFile(dir, FILE, serializeMemory(mem), mtime);
      return { result, mem, mtime: w.mtime };
    } catch (e) {
      if (attempt >= 1 || !/conflict/i.test(String((e as Error)?.message ?? e))) throw e;
    }
  }
}

export function memoryOpen(): boolean {
  return !!document.querySelector(".mm-back");
}

export function openMemory(o: MemoryViewOpts): void {
  if (memoryOpen()) return;
  let mem: ProjectMemory = parseMemory("");
  let mtime: number | null = null;
  let editing: string | null = null;
  /** The fact being sent to an agent ("*" for all of them). */
  let asking: string | null = null;
  let status = "";
  let undo: MemoryEntry | null = null;
  let error = "";
  /** The field a form that just opened puts the keyboard in. */
  let focusNext: string | null = null;

  const back = document.createElement("div");
  back.className = "inbox-modal-back mm-back";
  back.innerHTML = `<div class="inbox-modal mm" role="dialog" aria-modal="true" aria-labelledby="mmTitle">
    <header class="im-head"><div><h2 id="mmTitle">Project memory</h2><p class="im-sub">What every agent in <b>${esc(o.project)}</b> knows when it starts. Agents add to it as they learn.</p></div>
      <button type="button" class="im-x" data-close aria-label="Close">✕</button></header>
    ${o.projects && o.projects.length > 1 ? `<nav class="mm-tabs" aria-label="Projects">${o.projects.map((p) =>
      `<button type="button" class="mm-tab" data-proj-tab="${esc(p.id)}"${p.id === o.current ? ' aria-current="true"' : ""}>${esc(p.name)}</button>`).join("")}</nav>` : ""}
    <form class="mm-add"><label class="mm-sr" for="mmNew">Add a fact</label><textarea id="mmNew" rows="2" placeholder="Add a fact every agent here should know: a convention, a decision, where something lives…"></textarea><button type="submit" class="im-btn primary">Add</button></form>
    <div class="mm-body" aria-live="polite"></div>
    <footer class="mm-foot"></footer>
  </div>`;
  document.body.appendChild(back);
  const body = back.querySelector<HTMLElement>(".mm-body")!;
  const foot = back.querySelector<HTMLElement>(".mm-foot")!;
  const addBox = back.querySelector<HTMLTextAreaElement>("#mmNew")!;
  const prev = document.activeElement as HTMLElement | null;

  const askForm = (id: string) => {
    const agents = o.agents();
    if (!agents.length) return `<div class="mm-ask"><p class="mm-note">Start an agent in ${esc(o.project)} first: it checks the code and fixes ${id === "*" ? "the memory" : "this fact"} for you.</p><div class="mm-acts"><button type="button" class="im-btn quiet" data-cancel>Close</button></div></div>`;
    return `<form class="mm-ask" data-ask="${esc(id)}">
      <label class="mm-sr" for="mmNote">What should change</label>
      <textarea id="mmNote" rows="2" placeholder="${id === "*" ? "Anything to look at? (optional)" : "What is wrong with it? (optional)"}"></textarea>
      <div class="mm-acts"><label class="mm-to">Send to <select id="mmTo">${agents.map((a) => `<option value="${esc(a.id)}">${esc(a.name)}</option>`).join("")}</select></label>
        <span class="im-sp"></span><button type="button" class="im-btn quiet" data-cancel>Cancel</button><button type="submit" class="im-btn primary">Send</button></div>
    </form>`;
  };

  const entryHtml = (e: MemoryEntry) => {
    const when = e.at ? new Date(e.at).toLocaleDateString(undefined, { month: "short", day: "numeric" }) : "";
    const meta = [e.by === "you" ? "You" : e.by, when].filter(Boolean).join(" · ");
    if (editing === e.id) {
      return `<li class="mm-e editing"><form class="mm-edit" data-edit="${esc(e.id)}"><label class="mm-sr" for="mmEdit">Edit ${esc(e.id)}</label><textarea id="mmEdit" rows="3">${esc(e.text)}</textarea>
        <div class="mm-acts"><span class="im-sp"></span><button type="button" class="im-btn quiet" data-cancel>Cancel</button><button type="submit" class="im-btn primary">Save</button></div></form></li>`;
    }
    return `<li class="mm-e" data-id="${esc(e.id)}"><div class="mm-t">${esc(e.text)}</div>
      <div class="mm-m"><span class="mm-id">${esc(e.id)}</span>${meta ? `<span>${esc(meta)}</span>` : ""}<span class="im-sp"></span>
        <button type="button" class="mm-b" data-act="ask" title="Send this fact to an agent to check against the code and fix">Ask an agent</button>
        <button type="button" class="mm-b" data-act="edit">Edit</button>
        <button type="button" class="mm-b danger" data-act="delete">Delete</button></div>
      ${asking === e.id ? askForm(e.id) : ""}</li>`;
  };

  const render = () => {
    const focusId = (document.activeElement as HTMLElement | null)?.id;
    body.innerHTML = (error ? `<p class="mm-err" role="alert">${esc(error)}</p>` : "") + (mem.entries.length
      ? `<ol class="mm-list">${mem.entries.map(entryHtml).join("")}</ol>`
      : `<p class="mm-empty">Nothing yet. Agents save what they learn about ${esc(o.project)} here; add your own above.</p>`);
    foot.innerHTML = `${status ? `<span class="mm-status">${esc(status)}${undo ? ` <button type="button" class="mm-b" data-act="undo">Undo</button>` : ""}</span>` : ""}<span class="im-sp"></span>
      ${mem.entries.length ? `<button type="button" class="im-btn" data-act="ask-all" title="An agent checks every fact against the code, fixes, merges and fills gaps">Ask an agent to check all of it</button>` : ""}`;
    // the form for all of it sits over the footer
    back.querySelector(".mm-askall")?.remove();
    if (asking === "*") foot.insertAdjacentHTML("beforebegin", `<div class="mm-askall">${askForm("*")}</div>`);
    // A form you just opened takes the keyboard; otherwise it stays where it was.
    const next = focusNext ?? (focusId ? `#${focusId}` : null);
    focusNext = null;
    if (next) back.querySelector<HTMLElement>(next)?.focus();
  };

  const run = async <T>(fn: (m: ProjectMemory) => T, done?: (r: T) => void) => {
    error = "";
    try {
      const r = await change(o.dir, fn);
      mem = r.mem; mtime = r.mtime;
      done?.(r.result);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
    render();
  };

  const refresh = async () => {
    try {
      const r = await load(o.dir);
      mem = r.mem; mtime = r.mtime; error = "";
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
    render();
  };

  // An agent's change shows up while the panel is open, unless you are typing.
  const follow = window.setInterval(async () => {
    if (editing || asking) return;
    try {
      const s = await fsStat(o.dir, FILE);
      if (s.mtime !== mtime) await refresh();
    } catch { /* no file yet */ }
  }, FOLLOW_MS);

  const close = () => { window.clearInterval(follow); back.remove(); prev?.focus?.(); };

  back.addEventListener("click", (ev) => {
    const t = ev.target as HTMLElement;
    if (t === back || t.closest("[data-close]")) { close(); return; }
    const tab = t.closest<HTMLElement>("[data-proj-tab]")?.dataset.projTab;
    if (tab) { if (tab !== o.current && o.switchTo) { close(); o.switchTo(tab); } return; }
    if (t.closest("[data-cancel]")) { editing = null; asking = null; render(); return; }
    const act = t.closest<HTMLElement>("[data-act]")?.dataset.act;
    const id = t.closest<HTMLElement>("[data-id]")?.dataset.id ?? "";
    if (act === "edit") { editing = id; asking = null; focusNext = "#mmEdit"; render(); }
    else if (act === "ask") { asking = asking === id ? null : id; editing = null; focusNext = "#mmNote"; render(); }
    else if (act === "ask-all") { asking = asking === "*" ? null : "*"; editing = null; focusNext = "#mmNote"; render(); }
    else if (act === "delete") {
      void run((m) => deleteFact(m, id), (gone) => { undo = gone; status = `Deleted ${gone.id}.`; });
    } else if (act === "undo" && undo) {
      const again = undo;
      undo = null;
      void run((m) => addFact(m, again.text, again.by || "you", Date.now()), () => { status = "Put back."; });
    }
  });

  back.addEventListener("submit", (ev) => {
    ev.preventDefault();
    const form = ev.target as HTMLFormElement;
    if (form.classList.contains("mm-add")) {
      const text = addBox.value;
      if (!text.trim()) return;
      void run((m) => addFact(m, text, "you", Date.now()), (e) => { addBox.value = ""; status = `Added ${e.id}.`; undo = null; });
    } else if (form.dataset.edit) {
      const id = form.dataset.edit;
      const text = form.querySelector("textarea")!.value;
      void run((m) => updateFact(m, id, text, "you", Date.now()), () => { editing = null; status = `Saved ${id}.`; undo = null; });
    } else if (form.dataset.ask) {
      const id = form.dataset.ask;
      const entry = id === "*" ? null : mem.entries.find((x) => x.id === id) ?? null;
      const note = form.querySelector("textarea")!.value;
      const to = form.querySelector<HTMLSelectElement>("#mmTo")!;
      const name = to.selectedOptions[0]?.textContent ?? "the agent";
      asking = null;
      status = `Sending to ${name}…`;
      undo = null;
      render();
      void o.send(to.value, fixPrompt(entry, note)).then((ok) => {
        status = ok ? `Sent to ${name}. Its changes show up here as it makes them.` : `${name} did not take the message: it may be busy. Try again when it is free.`;
        render();
      });
    }
  });

  back.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape") {
      ev.stopPropagation();
      if (editing || asking) { editing = null; asking = null; render(); } else close();
    } else if (ev.key === "Enter" && (ev.ctrlKey || ev.metaKey) && (ev.target as HTMLElement).tagName === "TEXTAREA") {
      ev.preventDefault();
      (ev.target as HTMLElement).closest("form")?.requestSubmit();
    }
  });

  void refresh().then(() => addBox.focus());
}
