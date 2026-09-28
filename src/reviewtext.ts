// What Review types into an agent when it sends the work back, and the line
// numbers its diff shows. Pure (no DOM, no IPC) so it stays unit-testable.

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

/** The one message Send back types: the conflict to resolve, then your note.
 *  Empty when there is nothing to send. */
export function sendBackText(o: { note?: string; conflict?: Conflict | null }): string {
  const parts: string[] = [];
  if (o.conflict) parts.push(conflictText(o.conflict));
  const note = o.note?.trim();
  if (note) parts.push(note);
  return parts.join("\n\n");
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
