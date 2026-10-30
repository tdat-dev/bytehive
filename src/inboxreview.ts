// Review drawer for the Agent Inbox: what an agent changed, and the three ways
// to end the job, as the design has them. Merge takes an agent's own branch
// into the project folder's branch and removes its worktree (review_land);
// Send back types your note (and, after a conflict, what to resolve) into the
// agent; Discard branch throws the branch away. Agents that work in the project
// folder itself have no branch of their own: they get the diff and Send back.
// A line number in the diff takes a comment (Shift for a run of lines); the
// comments wait per agent until Send back types them all in one message.

import { parseDiff, type DiffFile } from "./diff";
import { confirmModal } from "./confirmmodal";
import { topNote } from "./hint";
import { killPty, repoDiff, reposUnder, reviewBranchDiff, reviewDiscardBranch, reviewLand, sendMessage, type LandResult } from "./ipc";
import { markMerged, taskOf } from "./tasks";
import { promptShown } from "./working";
import { clearComments, commentWhere, commentsOf, diffTotals, dropComment, numberHunk, putComment, rankFiles, sendBackText, type Conflict, type LineComment } from "./reviewtext";
import type { Pane } from "./panetypes";
import { describeDone, describeSecrets, scanDoneDiff, scanSecrets, testRuns, testSummary, type DoneFinding, type SecretFinding } from "./donecheck";
import { chatFor, itemsFor } from "./gridcards";
import { digestOf, planProgress } from "./convdigest";

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

/** The comment being written: the lines it is about, and its text so far.
 *  `id` when it edits a saved one. */
interface Editing {
  id?: string;
  side: "new" | "old";
  start: number;
  end: number;
  text: string;
}

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
  let doneFindings: DoneFinding[] = [];
  let secretFindings: SecretFinding[] = [];
  let busy = false;
  let msg: { text: string; kind: "" | "err" | "busy" } = { text: "", kind: "" };
  /** The lines of the file on screen, by the index their number button carries. */
  let rows: Array<{ side: "new" | "old"; num: number; text: string }> = [];
  let editing: Editing | null = null;
  /** Where a Shift-click run of lines starts, and the one line number in the Tab order. */
  let anchor: number | null = null;
  let lnFocus = 0;

  function close(): void {
    el?.remove();
    el = null;
    gen++;
    const was = current;
    current = null;
    onClose();
    if (was) was.term.focus();
  }

  function open(pane: Pane): void {
    el?.remove();
    current = pane;
    const my = ++gen;
    files = null; loadError = ""; fileIdx = 0; showAll = false;
    own = !!pane.spec.worktree; base = ""; branch = pane.spec.branch ?? "";
    check = null; conflict = null; busy = false; msg = { text: "", kind: "" }; doneFindings = []; secretFindings = [];
    rows = []; editing = null; anchor = null; lnFocus = 0;
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
      files = rankFiles(parseDiff(raw));
    } catch (e) {
      if (my !== gen) return;
      files = [];
      loadError = `Couldn't read the changes: ${String(e)}`;
    }
    doneFindings = scanDoneDiff(files ?? []);
    secretFindings = scanSecrets(files ?? []);
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
    const seen = testSummary(testRuns(itemsFor(pane.id)));
    if (seen) chip += `<span class="check"><span class="${seen.ok ? "a" : "d"}" aria-hidden="true">${seen.ok ? "✓" : "!"}</span>${esc(seen.text)}</span>`;
    const digest = digestOf(chatFor(pane.id));
    const prog = planProgress(digest.todos);
    const words = digest.lastWords.replace(/\s+/g, " ").trim().slice(-160);
    const digestLine = prog || words ? `<p class="rsaid" title="What ${esc(pane.spec.name)} last said">${esc(prog && words ? `${prog}: ${words}` : prog || words)}</p>` : "";
    return `<div class="rhead">
      <button class="hbtn ir-close" type="button" data-act="close" title="Close (Esc)">Close</button>
      <p class="lbl">${own ? "Changes on this branch" : "Changes in the project folder"}</p>
      <h2>${esc(count)}${files?.length ? ` <span class="num a">+${t.add}</span><span class="num d">−${t.del}</span>` : ""}</h2>
      ${where ? `<p class="br" title="${esc(where)}">${esc(where)}</p>` : ""}
      ${digestLine}
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

  /** A saved comment under its last line: where, what you said, Edit and Remove. */
  function noteHtml(c: LineComment): string {
    const where = esc(commentWhere(c));
    return `<div class="rc-note" data-cid="${esc(c.id)}">
      <p class="rc-where">${where}</p>
      <p class="rc-text">${esc(c.text)}</p>
      <div class="rc-acts"><button type="button" class="hbtn" data-act="rc-edit" data-cid="${esc(c.id)}" aria-label="Edit comment on ${where}">Edit</button><button type="button" class="hbtn" data-act="rc-del" data-cid="${esc(c.id)}" aria-label="Remove comment on ${where}">Remove</button></div>
    </div>`;
  }

  /** The box you write a comment in, under the line (or last line of the run) it is about. */
  function boxHtml(path: string, e: Editing): string {
    const where = esc(commentWhere({ path, side: e.side, start: e.start, end: e.end }));
    return `<div class="rc-box">
      <label class="rc-where" for="rcText">Comment on ${where}</label>
      <textarea id="rcText" class="note" placeholder="What should change here?">${esc(e.text)}</textarea>
      <div class="rc-acts"><span class="rc-hint"><kbd>Ctrl Enter</kbd> saves, <kbd>Esc</kbd> cancels</span><button type="button" class="hbtn" data-act="rc-cancel">Cancel</button><button type="button" class="btn primary" data-act="rc-save">${e.id ? "Save" : "Comment"}</button></div>
    </div>`;
  }

  function diffHtml(pane: Pane): string {
    const f = files![fileIdx];
    rows = [];
    if (!f) return "";
    const mine = commentsOf(pane.id).filter((c) => c.path === f.path);
    let html = "";
    let drawn = 0;
    if (f.binary) html = `<p class="rempty">Binary file, not shown.</p>`;
    for (const h of f.hunks) {
      if (!showAll && drawn >= MAX_LINES) break;
      html += `<div class="hunk">${esc(h.header)}</div>`;
      for (const l of numberHunk(h.header, h.lines)) {
        if (!showAll && drawn++ >= MAX_LINES) break;
        // A removed line is commented by its old number, every other by its new one.
        const side = l.kind === "del" ? "old" : "new";
        const num = (side === "old" ? l.old : l.new) ?? 0;
        const k = rows.push({ side, num, text: l.text }) - 1;
        const on = mine.filter((c) => c.side === side && num >= c.start && num <= c.end);
        const sign = l.kind === "add" ? "+" : l.kind === "del" ? "-" : " ";
        const label = `${on.length ? "Commented. Add another comment" : "Comment"} on ${side === "old" ? `removed line ${num}` : `line ${num}`}`;
        html += `<div class="dl${l.kind === "ctx" ? "" : ` ${l.kind}`}${on.length ? " cm" : ""}"><button type="button" class="ln" data-ln="${k}" tabindex="${k === lnFocus ? 0 : -1}" aria-label="${label}">${l.new ?? ""}</button><span class="c">${esc(sign + l.text)}</span></div>`;
        for (const c of mine) if (c.side === side && c.end === num) html += editing?.id === c.id ? boxHtml(f.path, editing) : noteHtml(c);
        if (editing && !editing.id && editing.side === side && editing.end === num) html += boxHtml(f.path, editing);
      }
    }
    const total = f.hunks.reduce((k, h) => k + h.lines.length, 0);
    if (!showAll && total > MAX_LINES) html += `<button type="button" class="hbtn ir-more" data-act="all">Show all ${total} lines</button>`;
    return `<div class="diff" aria-label="Diff of ${esc(f.path)}. Arrow keys move between lines; Enter comments on one, Shift+Enter on a run of lines.">${html}</div>`;
  }

  function footHtml(pane: Pane): string {
    const name = esc(pane.spec.name);
    const into = esc(base || "main");
    const note = notes.get(pane.id) ?? "";
    let line = msg.text ? `<p class="ir-msg ${msg.kind}" role="status">${esc(msg.text)}</p>` : "";
    const flagText = [describeDone(doneFindings), describeSecrets(secretFindings)].filter(Boolean).join("; ");
    if (!msg.text && !conflict && (doneFindings.length || secretFindings.length)) {
      line = `<p class="ir-msg warn" role="status">This diff adds ${esc(flagText)}. Merge asks once more; Send back asks ${name} to finish them first.</p>`;
    }
    if (!msg.text && conflict) {
      line = `<p class="ir-msg warn" role="status">Merging into ${esc(conflict.base)} would conflict in ${esc(fileList(conflict.files))}. Nothing was merged. Send back asks ${name} to merge ${esc(conflict.base)} in and resolve them.</p>`;
    } else if (!msg.text && own && check?.kind === "error") {
      line = `<p class="ir-msg" role="status">${esc(check.message)}</p>`;
    } else if (!msg.text && !own) {
      line = `<p class="ir-msg" role="status">${name} works in the project folder, not on a branch of its own, so there is nothing to merge.</p>`;
    }
    const off = busy ? " disabled" : "";
    const n = commentsOf(pane.id).length;
    const back = `<button type="button" class="btn" data-act="back"${off}${n ? ` aria-label="Send back with ${n} comment${n === 1 ? "" : "s"}"` : ""}>Send back${n ? ` <span class="rc-n" aria-hidden="true">${n}</span>` : ""}</button>`;
    const acts = own
      ? `<button type="button" class="btn primary" data-act="merge"${off}>Merge into ${into} <kbd>M</kbd></button>
         ${back}
         <button type="button" class="btn danger" data-act="discard"${off}>Discard branch</button>`
      : back;
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
    const back = at && el.contains(at) ? (at.dataset.act ? `[data-act="${at.dataset.act}"]` : at.dataset.i ? `[data-i="${at.dataset.i}"]` : at.dataset.v ? `[data-v="${at.dataset.v}"]` : at.dataset.ln ? `[data-ln="${at.dataset.ln}"]` : at.id ? `#${at.id}` : at.classList.contains("diff") ? ".diff" : null) : null;
    const scrolls = [...el.querySelectorAll<HTMLElement>(".diff, .files")].map((x) => x.scrollTop);
    const sameFile = el.dataset.file === String(fileIdx);
    el.dataset.file = String(fileIdx);
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
      body = filesHtml(pane) + diffHtml(pane) + footHtml(pane);
    }
    el.innerHTML = headHtml(pane) + body;
    if (sameFile) el.querySelectorAll<HTMLElement>(".diff, .files").forEach((x, i) => { x.scrollTop = scrolls[i] ?? 0; });
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
    const flags = [describeDone(doneFindings), describeSecrets(secretFindings)].filter(Boolean).join("; ");
    if (doneFindings.length || secretFindings.length) {
      const gate = await confirmModal({
        title: `Merge with ${flags}?`,
        message: `This branch adds ${flags}. Merging anyway is one click; cancel and Send back to have ${pane.spec.name} finish them first.`,
        okLabel: "Merge anyway", danger: secretFindings.some((f) => f.kind === "secret"),
      });
      if (!gate.ok || pane !== current) { busy = false; say(""); return; }
    }
    say(`Merging into ${dry.base}…`, "busy");
    const wasRunning = pane.running;
    if (wasRunning) await killPty(pane.id).catch(() => {});
    const r = await reviewLand(wt, { title, expectBase: dry.base }).catch((e) => ({ kind: "error", message: String(e) }) as LandResult);
    if (r.kind !== "ok") {
      if (wasRunning) void pane.restart?.();
      if (pane === current) fail(r);
      return;
    }
    markMerged(pane.id, r.base, r.branch);
    if (r.removed) { delete pane.spec.worktree; delete pane.spec.branch; }
    notes.delete(pane.id);
    clearComments(pane.id);
    viewed.delete(pane.id);
    busy = false;
    if (pane === current) close();
    topNote(r.removed ? `Merged <b>${esc(r.branch)}</b> into <b>${esc(r.base)}</b>. Worktree removed.` : `Merged <b>${esc(r.branch)}</b> into <b>${esc(r.base)}</b>. ${esc(r.note ?? "")}`, 4200);
  }

  /** Type the note (and what to resolve, after a conflict) into the agent. */
  async function sendBack(): Promise<void> {
    const pane = current;
    if (!pane || !el || busy) return;
    // A comment still open in its box goes too, as you'd expect.
    if (editing && el.querySelector<HTMLTextAreaElement>("#rcText")?.value.trim()) saveComment(false);
    const box = el.querySelector<HTMLTextAreaElement>("#irNote");
    const comments = commentsOf(pane.id);
    const text = sendBackText({ note: box?.value ?? notes.get(pane.id), conflict, comments });
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
    // Only what was sent: a comment written while it went stays.
    for (const c of comments) dropComment(pane.id, c.id);
    viewed.delete(pane.id);
    busy = false;
    if (pane === current) close();
    pane.term.focus();
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
    clearComments(pane.id);
    viewed.delete(pane.id);
    busy = false;
    if (pane === current) close();
    // Free again: a running agent starts over on a fresh conversation.
    if (wasRunning) void pane.restart?.({ fresh: true });
    topNote(`Discarded <b>${esc(br)}</b>. ${esc(pane.spec.name)} is free.`);
  }

  /* ---------------- comments ---------------- */

  function focusLine(k: number): void {
    const b = el?.querySelector<HTMLElement>(`[data-ln="${k}"]`);
    if (!b) return;
    el?.querySelectorAll<HTMLElement>(".ln[tabindex='0']").forEach((x) => x.setAttribute("tabindex", "-1"));
    b.setAttribute("tabindex", "0");
    lnFocus = k;
    b.focus({ preventScroll: false });
  }

  /** The row index of a line, so the keyboard can go back to it. */
  function rowOf(side: "new" | "old", num: number): number {
    return Math.max(0, rows.findIndex((r) => r.side === side && r.num === num));
  }

  /** Open the box under line `k`; with Shift, for the run from the last line you picked. */
  function startComment(k: number, extend: boolean): void {
    const row = rows[k];
    if (!row) return;
    const from = extend && anchor !== null && rows[anchor]?.side === row.side ? rows[anchor].num : row.num;
    editing = { side: row.side, start: Math.min(from, row.num), end: Math.max(from, row.num), text: "" };
    if (!extend) anchor = k;
    lnFocus = k;
    render();
    el?.querySelector<HTMLTextAreaElement>("#rcText")?.focus();
  }

  function saveComment(refocus = true): void {
    const pane = current;
    const f = files?.[fileIdx];
    const box = el?.querySelector<HTMLTextAreaElement>("#rcText");
    if (!pane || !f || !editing || !box) return;
    const e = editing;
    const text = box.value.trim();
    editing = null;
    if (!text) {
      if (e.id) dropComment(pane.id, e.id); // emptied: it goes
    } else {
      const was = e.id ? commentsOf(pane.id).find((c) => c.id === e.id) : undefined;
      const code = was?.code ?? rows.filter((r) => r.side === e.side && r.num >= e.start && r.num <= e.end).map((r) => r.text);
      putComment(pane.id, { id: e.id, path: f.path, side: e.side, start: e.start, end: e.end, code, text });
    }
    render();
    if (refocus) focusLine(rowOf(e.side, e.end));
  }

  function cancelComment(): void {
    const e = editing;
    editing = null;
    render();
    if (e) focusLine(rowOf(e.side, e.end));
  }

  function onClick(e: MouseEvent): void {
    const t = e.target as HTMLElement;
    const pick = t.closest<HTMLElement>(".pick");
    if (pick) { fileIdx = Number(pick.dataset.i); showAll = false; editing = null; anchor = null; lnFocus = 0; render(); return; }
    const ln = t.closest<HTMLElement>(".ln[data-ln]");
    if (ln) { startComment(Number(ln.dataset.ln), e.shiftKey); return; }
    const cid = t.closest<HTMLElement>("[data-cid]")?.dataset.cid;
    switch (t.closest<HTMLElement>("[data-act]")?.dataset.act) {
      case "rc-save": saveComment(); return;
      case "rc-cancel": cancelComment(); return;
      case "rc-edit": {
        const c = current && commentsOf(current.id).find((x) => x.id === cid);
        if (c) { editing = { id: c.id, side: c.side, start: c.start, end: c.end, text: c.text }; render(); el?.querySelector<HTMLTextAreaElement>("#rcText")?.focus(); }
        return;
      }
      case "rc-del": {
        const c = current && commentsOf(current.id).find((x) => x.id === cid);
        if (current && c) { dropComment(current.id, c.id); render(); focusLine(rowOf(c.side, c.end)); }
        return;
      }
    }
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
    const target = e.target as HTMLElement;
    if (target.id === "rcText") {
      // Esc puts the comment away, not the whole drawer; Ctrl+Enter keeps it.
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); cancelComment(); return; }
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); e.stopPropagation(); saveComment(); return; }
      return;
    }
    if (e.key === "Escape") { e.stopPropagation(); close(); return; }
    // Up and down walk the line numbers, so the Tab order holds one of them, not hundreds.
    const ln = target.closest<HTMLElement>(".ln[data-ln]");
    if (ln && ["ArrowUp", "ArrowDown", "Home", "End"].includes(e.key)) {
      e.preventDefault();
      const k = Number(ln.dataset.ln);
      const next = e.key === "Home" ? 0 : e.key === "End" ? rows.length - 1 : k + (e.key === "ArrowDown" ? 1 : -1);
      if (next >= 0 && next < rows.length) focusLine(next);
      return;
    }
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
