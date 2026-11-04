// What Review types into an agent when it sends the work back, the comments
// you leave on its diff until then, and the line numbers the diff shows. No DOM
// and no IPC, so it stays unit-testable.

import type { DiffFile, DiffLine } from "./diff";

/** A merge that would conflict: which branch, into what, in which files. */
export interface Conflict {
  base: string;
  branch: string;
  files: string[];
}

/** The prepared message for a conflict: the files, and what to do about them. */
export function conflictText(c: Conflict): string {
  const files = c.files.length ? c.files.map((f) => `- ${f}`).join("\n") : "- (git did not name the files)";
  return (
    `Merging your branch ${c.branch} into ${c.base} conflicts in:\n${files}\n\n` +
    `Nothing was merged; ${c.base} is unchanged. Merge ${c.base} into your branch (git merge ${c.base}), ` +
    `resolve the conflicts in those files, check that it still builds and the tests pass, and commit. ` +
    `Then tell me it is ready to merge again.`
  );
}

/** A comment on one line, or a run of lines, of the diff. `side` says which
 *  numbers `start`..`end` count: the new file's, or the old one's for lines
 *  the agent removed. `code` is the quoted line(s) as they were when you wrote it. */
export interface LineComment {
  id: string;
  path: string;
  side: "new" | "old";
  start: number;
  end: number;
  code: string[];
  text: string;
}

/** "src/a.ts:12", "src/a.ts:12-14", "src/a.ts:7 (removed)". */
export function commentWhere(c: Pick<LineComment, "path" | "side" | "start" | "end">): string {
  const at = c.start === c.end ? `${c.start}` : `${c.start}-${c.end}`;
  return `${c.path}:${at}${c.side === "old" ? " (removed)" : ""}`;
}

/** Each comment as the agent reads it: where, the quoted code, what to do. */
export function commentsText(list: LineComment[]): string {
  const sorted = [...list].sort((a, b) => a.path.localeCompare(b.path) || a.start - b.start);
  return `Review comments on your changes:\n\n` + sorted.map((c) =>
    `${commentWhere(c)}\n${c.code.map((l) => `> ${l}`).join("\n")}\n${c.text.trim()}`).join("\n\n");
}

/** The one message Send back types: the conflict to resolve, your comments on
 *  the diff, then your note. Empty when there is nothing to send. */
export function sendBackText(o: { note?: string; conflict?: Conflict | null; comments?: LineComment[] }): string {
  const parts: string[] = [];
  if (o.conflict) parts.push(conflictText(o.conflict));
  const comments = (o.comments ?? []).filter((c) => c.text.trim());
  if (comments.length) parts.push(commentsText(comments));
  const note = o.note?.trim();
  if (note) parts.push(note);
  if (comments.length) parts.push(`Address ${comments.length === 1 ? "this comment" : "each comment"}, then tell me when it is done.`);
  return parts.join("\n\n");
}

/* ---- drafts: kept per agent while ByteHive runs, until Send back delivers them ---- */

const drafts = new Map<string, LineComment[]>();
let nextId = 1;

/** The comments waiting to go to this agent, in the order written. */
export function commentsOf(agentId: string): LineComment[] {
  return drafts.get(agentId) ?? [];
}

/** Add a comment, or replace the one with the same id. Returns it. */
export function putComment(agentId: string, c: Omit<LineComment, "id"> & { id?: string }): LineComment {
  const list = [...commentsOf(agentId)];
  const saved: LineComment = { ...c, id: c.id ?? `c${nextId++}` };
  const i = list.findIndex((x) => x.id === saved.id);
  if (i >= 0) list[i] = saved; else list.push(saved);
  drafts.set(agentId, list);
  return saved;
}

export function dropComment(agentId: string, id: string): void {
  const list = commentsOf(agentId).filter((c) => c.id !== id);
  if (list.length) drafts.set(agentId, list); else drafts.delete(agentId);
}

export function clearComments(agentId: string): void {
  drafts.delete(agentId);
}

/** A diff line with the numbers it has on each side: `old` for a removed or
 *  unchanged line, `new` for an added or unchanged one. */
export interface NumberedLine extends DiffLine {
  old: number | null;
  new: number | null;
}

/** Number each hunk's lines from its `@@ -a,b +c,d @@` header. */
export function numberHunk(header: string, lines: DiffLine[]): NumberedLine[] {
  const m = header.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
  let o = m ? Number(m[1]) : 1;
  let n = m ? Number(m[2]) : 1;
  return lines.map((l) => {
    if (l.kind === "add") return { ...l, old: null, new: n++ };
    if (l.kind === "del") return { ...l, old: o++, new: null };
    return { ...l, old: o++, new: n++ };
  });
}

/** Lines added and removed across a diff. */
export function diffTotals(files: DiffFile[]): { add: number; del: number } {
  return files.reduce((t, f) => ({ add: t.add + f.additions, del: t.del + f.deletions }), { add: 0, del: 0 });
}
