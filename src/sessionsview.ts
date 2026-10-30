// Sessions: every earlier agent conversation of a project (its agents'
// worktrees included), Claude and Muse alike, read back as a conversation
// (transcript.ts). Only to read: nothing here starts, resumes or types into an agent.

import { claudeSessionsEverywhere, claudeSessionsIn, claudeTranscript, museSessionList, museTranscript, type ClaudeSession } from "./ipc";
import { createChat, turnsOf, type Chat, type ChatItem } from "./chatmodel";
import { createMuseChat } from "./musemodel";
import { ago, drawTranscript, forgetImages, openImage, samePath } from "./transcript";
import { dayLabel, clock } from "./inboxhistory";

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const tail = (p: string, n = 2) => p.replace(/[\\/]+$/, "").split(/[\\/]/).slice(-n).join("/");

/** A conversation opens with this much of its end; "Read all of it" loads the rest. */
const TAIL_BYTES = 4 * 1024 * 1024;
/** Messages and steps drawn at once; "Show earlier" widens it. */
const WINDOW = 200;
/** Most slices one read takes (2 MB each): a runaway file stops somewhere. */
const MAX_SLICES = 600;

type Scope = "project" | "all";

/** One row of the list, whichever CLI ran it. Muse sessions have no folder. */
export interface SessionRow { id: string; title: string; cwd: string; modified_ms: number; messages: number; cli: "claude" | "muse" }

/** Where a session ran, said for the list: the project itself, an agent's
 *  worktree by its name, or the folder's last two parts. */
export function placeOf(cwd: string, root: string | null): string {
  if (!cwd) return "";
  if (root && samePath(cwd, root)) return "";
  const p = cwd.replace(/\\/g, "/");
  // Maestro's own: <drive>/.maestro-worktrees/<repo>-<hash>/<branch>-<hash>
  const mine = p.match(/\/\.maestro-worktrees\/[^/]+\/([^/]+?)(?:-[0-9a-z]{6})?\/?$/);
  if (mine) return `worktree ${mine[1]}`;
  const wt = p.match(/\/\.claude\/worktrees\/([^/]+)\/?$/);
  if (wt) return `worktree ${wt[1]}`;
  return tail(cwd);
}

/** The sessions whose title or folder has every word of `q`. */
export function matching<T extends { title: string; cwd: string }>(list: T[], q: string): T[] {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return list;
  return list.filter((s) => {
    const hay = `${s.title} ${s.cwd}`.toLowerCase();
    return words.every((w) => hay.includes(w));
  });
}

interface Reading {
  s: SessionRow;
  chat: Chat;
  /** Only the end of it was read. */
  partial: boolean;
  loading: boolean;
  failed?: string;
  window: number;
  open: Set<string>;
  expanded: Set<string>;
}

export function createSessionsView() {
  let back: HTMLElement | null = null;
  let root: string | null = null;
  let scope: Scope = "project";
  let list: SessionRow[] = [];
  let listing = false;
  let query = "";
  let reading: Reading | null = null;
  /** Bumped on every new read, so a slow one that finishes late is dropped. */
  let readNo = 0;
  let listNo = 0;
  let returnTo: HTMLElement | null = null;

  const $ = <T extends HTMLElement>(sel: string) => back?.querySelector<T>(sel) ?? null;

  function close(): void {
    if (!back) return;
    readNo += 1;
    if (reading) forgetImages(reading.chat.items);
    reading = null;
    back.remove();
    back = null;
    returnTo?.focus({ preventScroll: true });
  }

  async function loadList(): Promise<void> {
    const no = ++listNo;
    listing = true;
    drawList();
    let got: SessionRow[] = [];
    try {
      if (scope === "project" && root) {
        got = (await claudeSessionsIn(root, 300)).map((s) => ({ ...s, cli: "claude" as const }));
      } else {
        // Muse sessions aren't filed by folder: they join the "all" list only.
        const [claude, muse] = await Promise.all([
          claudeSessionsEverywhere(200).catch((): ClaudeSession[] => []),
          museSessionList(200).catch(() => []),
        ]);
        got = [
          ...claude.map((s) => ({ ...s, cli: "claude" as const })),
          ...muse.map((s) => ({ id: s.id, title: s.title, cwd: "", modified_ms: s.modified_ms, messages: s.messages, cli: "muse" as const })),
        ].sort((a, b) => b.modified_ms - a.modified_ms);
      }
    } catch {
      got = [];
    }
    if (no !== listNo || !back) return;
    list = got;
    listing = false;
    drawList();
  }

  function drawList(): void {
    const ul = $<HTMLElement>(".sv-list");
    const count = $<HTMLElement>(".sv-count");
    if (!ul) return;
    const shown = matching(list, query);
    if (count) count.textContent = listing ? "" : query ? `${shown.length} of ${list.length}` : `${list.length} session${list.length === 1 ? "" : "s"}`;
    if (listing && !list.length) { ul.innerHTML = `<p class="sv-empty">Looking for sessions…</p>`; return; }
    if (!shown.length) {
      ul.innerHTML = `<p class="sv-empty">${query ? "No session matches that." : scope === "project" ? "No sessions in this project yet." : "No sessions yet."}</p>`;
      return;
    }
    let day = "";
    ul.innerHTML = shown.map((s) => {
      const d = dayLabel(s.modified_ms);
      const head = d !== day ? `<h3 class="sv-day">${esc(d)}</h3>` : "";
      day = d;
      const place = placeOf(s.cwd, scope === "project" ? root : null);
      const on = reading?.s.id === s.id;
      return `${head}<button type="button" class="sv-item${on ? " on" : ""}" data-sid="${esc(s.id)}" aria-current="${on}">
        <span class="sv-t">${esc(s.title || "Untitled")}</span>
        <span class="sv-m"><time>${clock(s.modified_ms)}</time>${s.cli === "muse" ? `<span class="sv-cli">Muse</span>` : ""}<span>${s.messages} message${s.messages === 1 ? "" : "s"}</span>${place ? `<span class="sv-where">${esc(place)}</span>` : ""}</span>
      </button>`;
    }).join("");
  }

  function drawReader(): void {
    const box = $<HTMLElement>(".sv-read");
    if (!box) return;
    if (!reading) {
      box.innerHTML = `<p class="sv-pick">Pick a session to read it back.</p>`;
      return;
    }
    const r = reading;
    let head = box.querySelector<HTMLElement>(".sv-rhead");
    let thread = box.querySelector<HTMLElement>(".cv-thread");
    if (!head || !thread || box.dataset.sid !== r.s.id) {
      box.dataset.sid = r.s.id;
      box.innerHTML = `
        <header class="sv-rhead">
          <button type="button" class="sv-backto" data-sv-back aria-label="Back to the list">Sessions</button>
          <div class="sv-rt"><b></b><span class="sv-rm"></span></div>
          <button type="button" class="sv-copyid" data-sv-copyid title="Copy this session's id">Copy ID</button>
        </header>
        <div class="sv-scroll cv-scroll"><div class="cv-thread sv-thread" role="log" aria-live="off"></div></div>`;
      head = box.querySelector<HTMLElement>(".sv-rhead")!;
      thread = box.querySelector<HTMLElement>(".cv-thread")!;
    }
    head.querySelector("b")!.textContent = r.s.title || "Untitled";
    head.querySelector(".sv-rm")!.textContent = [
      r.s.cwd,
      `${new Date(r.s.modified_ms).toLocaleString()} (${ago(r.s.modified_ms)})`,
      `${r.s.messages} message${r.s.messages === 1 ? "" : "s"}`,
    ].filter(Boolean).join(" · ");
    const items = r.chat.items;
    const start = Math.max(0, items.length - r.window);
    const notes: string[] = [];
    if (r.failed) notes.push(`<p class="sv-note err">${esc(r.failed)}</p>`);
    if (r.loading && !items.length) notes.push(`<p class="sv-note">Reading the conversation…</p>`);
    if (!r.loading && !r.failed && !items.length) notes.push(`<p class="sv-note">Nothing to show in this session.</p>`);
    const earlier = r.partial && start === 0 && !r.loading
      ? `<button type="button" class="cv-earlier" data-sv-whole>This is the end of a long session. Read all of it</button>`
      : start > 0 ? `<button type="button" class="cv-earlier" data-sv-earlier>Show ${Math.min(start, WINDOW)} earlier messages</button>` : "";
    drawTranscript(thread, [
      { key: "notes", html: notes.join("") },
      { key: "earlier", html: earlier },
    ], items.slice(start), r.open, r.expanded);
  }

  /** Read a session: its end first, then on to its last line. `whole` reads it from the start. */
  async function read(s: SessionRow, whole = false): Promise<void> {
    const no = ++readNo;
    if (reading) forgetImages(reading.chat.items);
    reading = { s, chat: s.cli === "muse" ? createMuseChat() : createChat(), partial: false, loading: true, window: WINDOW, open: new Set(), expanded: new Set() };
    const r = reading;
    back?.classList.add("reading");
    drawList();
    drawReader();
    const scroller = () => $<HTMLElement>(".sv-scroll");
    try {
      if (s.cli !== "muse" && !s.cwd) throw new Error("This session doesn't say which folder it ran in, so it can't be found.");
      const pull = (off: number, tail?: number) =>
        s.cli === "muse" ? museTranscript(s.id, off, tail) : claudeTranscript(s.cwd, s.id, null, off, tail);
      let c = await pull(0, whole ? undefined : TAIL_BYTES);
      if (no !== readNo) return;
      if (!c.path) throw new Error("This session's file is gone.");
      r.partial = (c.from ?? 0) > 0;
      r.chat.feed(c.text);
      for (let k = 0; c.text && k < MAX_SLICES; k += 1) {
        c = await pull(c.next);
        if (no !== readNo) return;
        if (!c.text) break;
        r.chat.feed(c.text);
      }
    } catch (e) {
      if (no !== readNo) return;
      r.failed = e instanceof Error ? e.message : "Couldn't read this session.";
    }
    r.loading = false;
    drawReader();
    const sc = scroller();
    if (sc) sc.scrollTop = whole ? 0 : sc.scrollHeight;
  }

  function onClick(e: MouseEvent): void {
    const t = e.target as HTMLElement;
    if (t === back || t.closest("[data-sv-close]")) { close(); return; }
    const sc = t.closest<HTMLElement>("[data-scope]");
    if (sc) {
      const next = sc.dataset.scope as Scope;
      if (next === scope) return;
      scope = next;
      back?.querySelectorAll<HTMLElement>("[data-scope]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.scope === scope)));
      list = [];
      void loadList();
      return;
    }
    const item = t.closest<HTMLElement>("[data-sid]");
    if (item) {
      const s = list.find((x) => x.id === item.dataset.sid);
      if (s && reading?.s.id !== s.id) void read(s);
      else back?.classList.add("reading");
      return;
    }
    if (t.closest("[data-sv-back]")) { back?.classList.remove("reading"); $<HTMLElement>(".sv-item.on")?.focus(); return; }
    const r = reading;
    if (!r) return;
    if (t.closest("[data-sv-copyid]")) {
      const b = t.closest<HTMLElement>("[data-sv-copyid]")!;
      void navigator.clipboard?.writeText(r.s.id).then(() => { b.textContent = "Copied"; window.setTimeout(() => { b.textContent = "Copy ID"; }, 1400); }).catch(() => {});
      return;
    }
    if (t.closest("[data-sv-whole]")) { void read(r.s, true); return; }
    if (t.closest("[data-sv-earlier]")) {
      const scroller = $<HTMLElement>(".sv-scroll")!;
      const h = scroller.scrollHeight;
      r.window += WINDOW;
      drawReader();
      scroller.scrollTop += scroller.scrollHeight - h;
      return;
    }
    const codeCopy = t.closest<HTMLElement>("[data-copy-code]");
    if (codeCopy) {
      const code = codeCopy.closest("pre")?.querySelector("code")?.textContent ?? "";
      copied(codeCopy, code);
      return;
    }
    const msg = t.closest<HTMLElement>("[data-copy-msg]");
    if (msg) {
      const said = r.chat.items.find((i) => i.id === msg.dataset.copyMsg);
      copied(msg, said?.kind === "user" ? said.text : "");
      return;
    }
    const copy = t.closest<HTMLElement>("[data-copy]");
    if (copy) {
      const turn = turnsOf(r.chat.items).find((x) => x.items.some((i) => i.id === copy.dataset.copy));
      copied(copy, turn?.items.filter((i) => i.kind === "text").map((i) => ("text" in i ? i.text : "")).join("\n\n") ?? "");
      return;
    }
    const shot = t.closest<HTMLElement>("[data-shot]");
    if (shot) {
      const owner: ChatItem | undefined = r.chat.items.find((i) => i.id === shot.dataset.shot);
      const pics = owner?.kind === "step" ? owner.images : owner?.kind === "user" ? owner.pics : undefined;
      if (pics?.length) openImage(pics, Number(shot.dataset.k) || 0, owner?.kind === "step" ? `${owner.verb} ${owner.target}`.trim() : "You sent");
      return;
    }
    const more = t.closest<HTMLElement>("[data-expand]");
    if (more) { r.expanded.add(more.dataset.expand!); drawReader(); return; }
    const fold = t.closest<HTMLElement>("[data-fold]");
    if (fold) { r.expanded.delete(fold.dataset.fold!); drawReader(); return; }
    const step = t.closest<HTMLElement>("button.cv-sh")?.closest<HTMLElement>(".cv-step");
    if (step) {
      const id = step.dataset.id!;
      const todo = r.chat.items.some((i) => i.id === id && i.kind === "step" && i.tool === "TodoWrite");
      const openNow = step.classList.contains("open");
      // A plan shows open by default: closing it is remembered as "!id".
      if (todo) { if (openNow) { r.open.delete(id); r.open.add(`!${id}`); } else { r.open.delete(`!${id}`); r.open.add(id); } }
      else if (openNow) r.open.delete(id);
      else r.open.add(id);
      drawReader();
    }
  }

  function copied(b: HTMLElement, text: string): void {
    const was = b.textContent;
    if (!text) { b.textContent = "Nothing to copy"; window.setTimeout(() => { b.textContent = was; }, 1400); return; }
    void navigator.clipboard?.writeText(text).then(() => { b.textContent = "Copied"; window.setTimeout(() => { b.textContent = was; }, 1400); }).catch(() => {});
  }

  function onKey(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      // A picture opened from here closes first.
      if (document.querySelector(".cv-lb")) return;
      e.preventDefault();
      e.stopPropagation();
      if (back?.classList.contains("reading") && back.matches(".narrow")) back.classList.remove("reading");
      else close();
      return;
    }
    const t = e.target as HTMLElement;
    if ((e.key === "ArrowDown" || e.key === "ArrowUp") && (t.matches(".sv-item") || t.matches(".sv-q"))) {
      const items = [...(back?.querySelectorAll<HTMLElement>(".sv-item") ?? [])];
      if (!items.length) return;
      e.preventDefault();
      const i = items.indexOf(t);
      const next = t.matches(".sv-q") ? items[0] : items[Math.max(0, Math.min(items.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))];
      next?.focus();
      return;
    }
    if (e.key === "Enter" && t.matches(".sv-q")) { e.preventDefault(); back?.querySelector<HTMLElement>(".sv-item")?.click(); return; }
    // Plain keys stay here, away from the agent's terminal underneath.
    if (!e.ctrlKey && !e.altKey && !e.metaKey) e.stopPropagation();
  }

  /** Open the sessions of the project at `dir` (or of every folder, without one). */
  function open(dir: string | null, name?: string): void {
    if (back) { $<HTMLInputElement>(".sv-q")?.focus(); return; }
    returnTo = document.activeElement as HTMLElement | null;
    root = dir;
    scope = dir ? "project" : "all";
    list = [];
    query = "";
    reading = null;
    back = document.createElement("div");
    back.className = "inbox-modal-back sv-back";
    back.innerHTML = `
      <div class="sv" role="dialog" aria-modal="true" aria-label="Sessions">
        <aside class="sv-side">
          <header class="sv-head">
            <div class="sv-ht"><h2>Sessions</h2><span class="sv-count" aria-live="polite"></span></div>
            <button type="button" class="ir-close" data-sv-close aria-label="Close sessions" title="Close (Esc)">Close</button>
          </header>
          <div class="sv-scope" role="group" aria-label="Which sessions">
            <button type="button" data-scope="project" aria-pressed="${scope === "project"}" ${dir ? "" : "disabled"} title="${esc(dir ?? "")}">${esc(name ?? "This project")}</button>
            <button type="button" data-scope="all" aria-pressed="${scope === "all"}">All folders</button>
          </div>
          <input type="search" class="sv-q" placeholder="Search by title or folder" aria-label="Search sessions" spellcheck="false">
          <div class="sv-list" role="list"></div>
          <p class="sv-foot">Past agent sessions, read only. Nothing here starts or resumes an agent.</p>
        </aside>
        <section class="sv-read" aria-label="Session"></section>
      </div>`;
    document.body.appendChild(back);
    const box = back.querySelector<HTMLElement>(".sv")!;
    const fit = () => back?.classList.toggle("narrow", box.clientWidth < 760);
    new ResizeObserver(fit).observe(box);
    fit();
    back.addEventListener("click", onClick);
    back.addEventListener("keydown", onKey);
    const q = back.querySelector<HTMLInputElement>(".sv-q")!;
    q.addEventListener("input", () => { query = q.value; drawList(); });
    q.focus();
    drawReader();
    void loadList();
  }

  return { open, close, get isOpen() { return !!back; } };
}
