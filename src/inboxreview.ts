// Review drawer for the Agent Inbox: what an agent changed, and the three ways
// to end the job, as the design has them. Merge takes an agent's own branch
// into the project folder's branch and removes its worktree (review_land);
// Send back types your note (and, after a conflict, what to resolve) into the
// agent; Discard branch throws the branch away. Agents that work in the project
// folder itself have no branch of their own: they get the diff and Send back.

import { parseDiff, type DiffFile } from "./diff";
import { focusAgent } from "./chatview";
import { confirmModal } from "./confirmmodal";
import { topNote } from "./hint";
import { killPty, repoDiff, reposUnder, reviewBranchDiff, reviewDiscardBranch, reviewLand, sendMessage, type LandResult } from "./ipc";
import { markMerged, taskOf } from "./tasks";
import { promptShown } from "./working";
import { diffTotals, numberHunk, sendBackText, type Conflict } from "./reviewtext";
import type { Pane } from "./panetypes";

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

/** Lines drawn for one file before the rest waits behind "Show all". */
const MAX_LINES = 1500;
/** How long a stopped agent gets to start again before Send back gives up. */
const WAKE_MS = 30_000;

/** The folder whose changes belong to this agent: its worktree, else where it runs. */
export function reviewDir(pane: Pane): string | null {
  return pane.spec.worktree || pane.spec.cwd || null;
}

/** Kept per agent while Maestro runs: the note you were writing and the files
 *  you ticked as viewed, so switching agents or screens loses neither. */
const notes = new Map<string, string>();
const viewed = new Map<string, Set<string>>();

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** Start a stopped agent again and wait for its prompt; true once it takes input. */
async function wake(pane: Pane): Promise<boolean> {
  if (pane.running) return true;
  await pane.restart?.();
  const until = Date.now() + WAKE_MS;
  while (Date.now() < until) {
    if (pane.running && promptShown(pane.term.snapshot?.(40) ?? "")) { await sleep(600); return true; }
    await sleep(300);
  }
  return pane.running;
}

/** "a.ts", "a.ts and b.ts", "a.ts, b.ts and 3 more". */
function fileList(files: string[]): string {
  if (files.length <= 1) return files[0] ?? "some files";
  if (files.length === 2) return `${files[0]} and ${files[1]}`;
  if (files.length === 3) return `${files[0]}, ${files[1]} and ${files[2]}`;
  return `${files[0]}, ${files[1]} and ${files.length - 2} more`;
}

export function createReviewDrawer(host: HTMLElement, onClose: () => void) {
  let el: HTMLElement | null = null;
  let current: Pane | null = null;
  /** Bumped on every open, so a slow read for the previous agent is dropped. */
  let gen = 0;

  let files: DiffFile[] | null = null;
  let loadError = "";
  let fileIdx = 0;
  let showAll = false;
  /** The agent has a worktree and branch of its own (Merge and Discard apply). */
  let own = false;
  let base = "";
  let branch = "";
  /** The dry run's answer: merges cleanly, conflicts, or can't merge (why). */
  let check: LandResult | null = null;
  let conflict: Conflict | null = null;
  let busy = false;
  let msg: { text: string; kind: "" | "err" | "busy" } = { text: "", kind: "" };

  function close(): void {
    el?.remove();
    el = null;
    gen++;
    const was = current;
    current = null;
    onClose();
    if (was) focusAgent(was);
  }

  function open(pane: Pane): void {
    el?.remove();
    current = pane;
    const my = ++gen;
    files = null; loadError = ""; fileIdx = 0; showAll = false;
    own = !!pane.spec.worktree; base = ""; branch = pane.spec.branch ?? "";
    check = null; conflict = null; busy = false; msg = { text: "", kind: "" };
    el = document.createElement("aside");
    el.className = "inbox-review";
    el.setAttribute("role", "dialog");
    el.setAttribute("aria-label", `${pane.spec.name}'s changes`);
    host.appendChild(el);
    el.addEventListener("click", onClick);
    el.addEventListener("change", onChange);
    el.addEventListener("input", onInput);
    el.addEventListener("keydown", onKey);
    render();
    el.querySelector<HTMLElement>(".ir-close")?.focus({ preventScroll: true });
    void load(pane, my);
  }

  async function load(pane: Pane, my: number): Promise<void> {
    const wt = pane.spec.worktree;
    const title = taskOf(pane.id)?.title ?? pane.spec.title ?? null;
    try {
      let raw: string;
      if (wt) {
        // Everything on its branch (its commits too), and whether it would merge.
        const [diff, dry] = await Promise.all([
          reviewBranchDiff(wt).catch(() => null),
          reviewLand(wt, { title, dryRun: true }).catch((e) => ({ kind: "error", message: String(e) }) as LandResult),
        ]);
        if (my !== gen) return;
        check = dry;
        if (dry.kind !== "error") { base = dry.base; branch = dry.branch; }
        if (dry.kind === "conflict") conflict = { base: dry.base, branch: dry.branch, files: dry.files };
        // No branch of its own after all (or git can't tell): its folder's plain diff.
        if (diff === null) own = false;
        raw = diff ?? (await repoDiff(wt));
      } else {
        const dir = reviewDir(pane);
        const repo = dir ? (await reposUnder(dir))[0] : undefined;
        raw = repo ? await repoDiff(repo.path) : "";
        if (!repo) loadError = dir ? "This folder is not a git repository, so there is no diff to show." : "This agent has no folder.";
      }
      if (my !== gen) return;
      files = parseDiff(raw);
    } catch (e) {
      if (my !== gen) return;
      files = [];
      loadError = `Couldn't read the changes: ${String(e)}`;
    }
    render();
  }

  /* ---------------- drawing ---------------- */

  function headHtml(pane: Pane): string {
    const list = files ?? [];
    const t = diffTotals(list);
    const count = files === null ? "Reading the changes…" : `${list.length} file${list.length === 1 ? "" : "s"} changed`;
    const where = own ? `${branch || pane.spec.branch || ""}${base ? ` → ${base}` : ""}` : reviewDir(pane) ?? "";
    let chip = "";
    if (own && check?.kind === "ok") chip = `<span class="check"><span class="a" aria-hidden="true">✓</span>Merges cleanly into ${esc(check.base)}</span>`;
    else if (own && conflict) chip = `<span class="check"><span class="d" aria-hidden="true">!</span>Conflicts with ${esc(conflict.base)} in ${conflict.files.length} file${conflict.files.length === 1 ? "" : "s"}</span>`;
    return `<div class="rhead">
      <button class="hbtn ir-close" type="button" data-act="close" title="Close (Esc)">Close</button>
      <p class="lbl">${own ? "Changes on this branch" : "Changes in the project folder"}</p>
      <h2>${esc(count)}${files?.length ? ` <span class="num a">+${t.add}</span><span class="num d">−${t.del}</span>` : ""}</h2>
      ${where ? `<p class="br" title="${esc(where)}">${esc(where)}</p>` : ""}
      ${chip ? `<div class="checks">${chip}</div>` : ""}
    </div>`;
  }

  function filesHtml(pane: Pane): string {
    const list = files!;
    const seen = viewed.get(pane.id) ?? new Set<string>();
    const n = list.filter((f) => seen.has(f.path)).length;
    return `<div class="fbar"><span><span class="num">${n} / ${list.length}</span> files viewed</span><span class="progress" aria-hidden="true"><i style="width:${Math.round((n / list.length) * 100)}%"></i></span></div>
    <div class="files">${list.map((f, i) => `<div class="file${i === fileIdx ? " cur" : ""}${seen.has(f.path) ? " seen" : ""}">
        <button type="button" class="pick" data-i="${i}" aria-current="${i === fileIdx}" title="${esc(f.path)}"><span class="p">${esc(f.path)}</span><span class="a">+${f.additions}</span><span class="d">−${f.deletions}</span></button>
        <label><input type="checkbox" data-v="${i}"${seen.has(f.path) ? " checked" : ""}> Viewed</label></div>`).join("")}</div>`;
  }

  function diffHtml(): string {
    const f = files![fileIdx];
    if (!f) return "";
    let html = "";
    let drawn = 0;
    if (f.binary) html = `<p class="rempty">Binary file, not shown.</p>`;
    for (const h of f.hunks) {
      if (!showAll && drawn >= MAX_LINES) break;
      html += `<div class="hunk">${esc(h.header)}</div>`;
      for (const l of numberHunk(h.header, h.lines)) {
        if (!showAll && drawn++ >= MAX_LINES) break;
        const sign = l.kind === "add" ? "+" : l.kind === "del" ? "-" : " ";
        html += `<div class="dl${l.kind === "ctx" ? "" : ` ${l.kind}`}"><span class="ln">${l.new ?? ""}</span><span class="c">${esc(sign + l.text)}</span></div>`;
      }
    }
    const total = f.hunks.reduce((k, h) => k + h.lines.length, 0);
    if (!showAll && total > MAX_LINES) html += `<button type="button" class="hbtn ir-more" data-act="all">Show all ${total} lines</button>`;
    return `<div class="diff" tabindex="0" aria-label="Diff of ${esc(f.path)}">${html}</div>`;
  }

  function footHtml(pane: Pane): string {
    const name = esc(pane.spec.name);
    const into = esc(base || "main");
    const note = notes.get(pane.id) ?? "";
    let line = msg.text ? `<p class="ir-msg ${msg.kind}" role="status">${esc(msg.text)}</p>` : "";
    if (!msg.text && conflict) {
      line = `<p class="ir-msg warn" role="status">Merging into ${esc(conflict.base)} would conflict in ${esc(fileList(conflict.files))}. Nothing was merged. Send back asks ${name} to merge ${esc(conflict.base)} in and resolve them.</p>`;
    } else if (!msg.text && own && check?.kind === "error") {
      line = `<p class="ir-msg" role="status">${esc(check.message)}</p>`;
    } else if (!msg.text && !own) {
      line = `<p class="ir-msg" role="status">${name} works in the project folder, not on a branch of its own, so there is nothing to merge.</p>`;
    }
    const off = busy ? " disabled" : "";
    const acts = own
      ? `<button type="button" class="btn primary" data-act="merge"${off}>Merge into ${into} <kbd>M</kbd></button>
         <button type="button" class="btn" data-act="back"${off}>Send back</button>
         <button type="button" class="btn danger" data-act="discard"${off}>Discard branch</button>`
      : `<button type="button" class="btn" data-act="back"${off}>Send back</button>`;
    return `<div class="rfoot">${line}
      <label class="ia-sr" for="irNote">Note for ${name}</label>
      <textarea class="note" id="irNote" placeholder="What should ${name} change? Needed only for Send back.">${esc(note)}</textarea>
      <div class="row2">${acts}</div>
    </div>`;
  }

  function render(): void {
    const pane = current;
    if (!el || !pane) return;
    // Keep the keyboard where it was across a redraw.
    const at = document.activeElement as HTMLElement | null;
    const back = at && el.contains(at) ? (at.dataset.act ? `[data-act="${at.dataset.act}"]` : at.dataset.i ? `[data-i="${at.dataset.i}"]` : at.dataset.v ? `[data-v="${at.dataset.v}"]` : at.id ? `#${at.id}` : at.classList.contains("diff") ? ".diff" : null) : null;
    const t = taskOf(pane.id);
    let body: string;
    if (t?.status.state === "merged") {
      body = `<div class="rempty"><strong>Merged into ${esc(t.mergedInto ?? "main")}</strong>Its worktree and branch are removed. Clear it from the list, or give ${esc(pane.spec.name)} a new task.
        <button type="button" class="btn" data-act="clear">Clear from list</button></div>`;
    } else if (files === null) {
      body = `<div class="rempty" role="status">Reading the changes…</div>`;
    } else if (!files.length) {
      const working = t?.status.state === "working";
      body = `<div class="rempty"><strong>${working ? "Still working" : "Nothing to review"}</strong>${esc(loadError || (working ? `The diff lands here when ${pane.spec.name} stops.` : own ? "No changes on this branch." : "No uncommitted changes in this folder."))}</div>` +
        (conflict ? footHtml(pane) : "");
    } else {
      body = filesHtml(pane) + diffHtml() + footHtml(pane);
    }
    el.innerHTML = headHtml(pane) + body;
    if (back) el.querySelector<HTMLElement>(back)?.focus({ preventScroll: true });
  }

  /* ---------------- acting ---------------- */

  function say(text: string, kind: "" | "err" | "busy" = ""): void {
    msg = { text, kind };
    render();
  }

  /** Merge its branch in and remove the worktree. The agent is stopped first
   *  (Windows won't delete a folder a process runs in), and only once a dry
   *  run says the merge is clean; if the real one fails anyway it starts again. */
  async function merge(): Promise<void> {
    const pane = current;
    const wt = pane?.spec.worktree;
    if (!pane || !wt || !own || busy) return;
    const title = taskOf(pane.id)?.title ?? pane.spec.title ?? null;
    busy = true;
    say("Checking the merge…", "busy");
    const fail = (r: LandResult) => {
      busy = false;
      if (r.kind === "conflict") { conflict = { base: r.base, branch: r.branch, files: r.files }; check = r; say(""); }
      else if (r.kind === "error") say(r.message, "err");
    };
    const dry = await reviewLand(wt, { title, dryRun: true }).catch((e) => ({ kind: "error", message: String(e) }) as LandResult);
    if (pane !== current) return;
    if (dry.kind !== "ok") return fail(dry);
    say(`Merging into ${dry.base}…`, "busy");
    const wasRunning = pane.running;
    if (wasRunning) await killPty(pane.id).catch(() => {});
    const r = await reviewLand(wt, { title }).catch((e) => ({ kind: "error", message: String(e) }) as LandResult);
    if (r.kind !== "ok") {
      if (wasRunning) void pane.restart?.();
      if (pane === current) fail(r);
      return;
    }
    markMerged(pane.id, r.base, r.branch);
    if (r.removed) { delete pane.spec.worktree; delete pane.spec.branch; }
    notes.delete(pane.id);
    viewed.delete(pane.id);
    busy = false;
    if (pane === current) close();
    topNote(r.removed ? `Merged <b>${esc(r.branch)}</b> into <b>${esc(r.base)}</b>. Worktree removed.` : `Merged <b>${esc(r.branch)}</b> into <b>${esc(r.base)}</b>. ${esc(r.note ?? "")}`, 4200);
  }

  /** Type the note (and what to resolve, after a conflict) into the agent. */
  async function sendBack(): Promise<void> {
    const pane = current;
    if (!pane || !el || busy) return;
    const box = el.querySelector<HTMLTextAreaElement>("#irNote");
    const text = sendBackText({ note: box?.value ?? notes.get(pane.id), conflict });
    if (!text) {
      if (box) { box.placeholder = "Write what should change first, then press Send back."; box.focus(); }
      return;
    }
    busy = true;
    if (!pane.running) {
      say(`Starting ${pane.spec.name} again…`, "busy");
      if (!(await wake(pane))) { busy = false; if (pane === current) say(`${pane.spec.name} didn't start, so nothing was sent.`, "err"); return; }
    }
    await sendMessage(pane.id, text);
    notes.delete(pane.id);
    viewed.delete(pane.id);
    busy = false;
    if (pane === current) close();
    focusAgent(pane);
    topNote(`Sent back to <b>${esc(pane.spec.name)}</b>`);
  }

  /** Throw the branch away, after asking: the worktree, its uncommitted work and the branch. */
  async function discard(): Promise<void> {
    const pane = current;
    const wt = pane?.spec.worktree;
    if (!pane || !wt || !own || busy) return;
    const br = branch || pane.spec.branch || "its branch";
    const ok = await confirmModal({
      title: `Discard ${br}?`,
      message: `${pane.spec.name}'s worktree and the branch ${br} are deleted, with every change on them. This cannot be undone.`,
      okLabel: "Discard branch",
      danger: true,
    });
    if (!ok.ok || pane !== current) return;
    busy = true;
    say("Discarding…", "busy");
    const wasRunning = pane.running;
    if (wasRunning) await killPty(pane.id).catch(() => {});
    try {
      await reviewDiscardBranch(wt);
    } catch (e) {
      busy = false;
      if (wasRunning) void pane.restart?.();
      if (pane === current) say(String(e), "err");
      return;
    }
    delete pane.spec.worktree;
    delete pane.spec.branch;
    notes.delete(pane.id);
    viewed.delete(pane.id);
    busy = false;
    if (pane === current) close();
    // Free again: a running agent starts over on a fresh conversation.
    if (wasRunning) void pane.restart?.({ fresh: true });
    topNote(`Discarded <b>${esc(br)}</b>. ${esc(pane.spec.name)} is free.`);
  }

  function onClick(e: MouseEvent): void {
    const t = e.target as HTMLElement;
    const pick = t.closest<HTMLElement>(".pick");
    if (pick) { fileIdx = Number(pick.dataset.i); showAll = false; render(); return; }
    switch (t.closest<HTMLElement>("[data-act]")?.dataset.act) {
      case "close": close(); break;
      case "merge": void merge(); break;
      case "back": void sendBack(); break;
      case "discard": void discard(); break;
      case "all": showAll = true; render(); break;
      case "clear": { const p = current; close(); p?.el.querySelector<HTMLElement>("[data-kill]")?.click(); break; }
    }
  }

  function onChange(e: Event): void {
    const box = (e.target as HTMLElement).closest<HTMLInputElement>("input[data-v]");
    if (!box || !current || !files) return;
    const f = files[Number(box.dataset.v)];
    const set = viewed.get(current.id) ?? new Set<string>();
    if (box.checked) set.add(f.path); else set.delete(f.path);
    viewed.set(current.id, set);
    render();
  }

  function onInput(e: Event): void {
    const box = e.target as HTMLTextAreaElement;
    if (box.id === "irNote" && current) notes.set(current.id, box.value);
  }

  function onKey(e: KeyboardEvent): void {
    if (e.key === "Escape") { e.stopPropagation(); close(); return; }
    const typing = (e.target as HTMLElement).closest("input, textarea, select, [contenteditable]");
    // M merges, as the button says; not while you type.
    if (!typing && !e.altKey && !e.ctrlKey && !e.metaKey && e.key.toLowerCase() === "m" && own && files?.length) {
      e.preventDefault();
      e.stopPropagation();
      void merge();
    }
  }

  return {
    open,
    close,
    merge,
    /** The agent being reviewed, if the drawer is open. */
    get paneId(): string | null { return current?.id ?? null; },
  };
}
