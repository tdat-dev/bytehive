// Who changed what in a folder agents share. git can say what changed, not who
// did, and an agent without a worktree has nothing of its own to compare. So
// each look at the folder is held against the one before: a file that moved in
// between belongs to the agents that were working then. One agent working: it is
// that agent's. Several: nobody can tell, so each keeps it and it is marked
// shared. None: it was you, or something else, and no agent claims it.
//
// Kept in memory: after a restart an agent starts with a clean slate.

import type { FolderState } from "./ipc";

export interface OwnWork {
  /** The commit this agent's changes are measured from. */
  from: string | null;
  /** The files it changed, in git's form. */
  paths: string[];
  /** Of those, how many another agent may have changed instead. */
  shared: number;
}

export function createFolderWork() {
  let last: FolderState | null = null;
  const owners = new Map<string, Set<string>>();
  const starts = new Map<string, string | null>();

  function claim(path: string, working: string[], from: string | null): void {
    if (!working.length) return;
    const who = owners.get(path) ?? new Set<string>();
    for (const id of working) {
      who.add(id);
      if (!starts.has(id)) starts.set(id, from);
    }
    owners.set(path, who);
  }

  return {
    /** A look at the folder has been taken. */
    seen: () => last !== null,
    head: () => last?.head ?? null,
    /** Compare a new look with the last one. `working` are the agents that
     *  printed anything since then. The first look only sets the mark: what is
     *  dirty by then is nobody's. */
    see(now: FolderState, working: string[]): void {
      const was = last;
      last = now;
      if (!was) return;
      const from = was.head;
      const paths = new Set([...Object.keys(was.files), ...Object.keys(now.files)]);
      for (const p of paths) if (was.files[p] !== now.files[p]) claim(p, working, from);
      for (const p of now.committed) {
        // Committed with no agent at work: that is you, taking it.
        if (working.length) claim(p, working, from);
        else owners.delete(p);
      }
    },
    /** What one agent changed; no files yet is an empty list. */
    of(id: string): OwnWork {
      const paths: string[] = [];
      let shared = 0;
      for (const [p, who] of owners) {
        if (!who.has(id)) continue;
        paths.push(p);
        if (who.size > 1) shared++;
      }
      return { from: starts.get(id) ?? null, paths: paths.sort(), shared };
    },
    /** You have looked at this agent's work, or it is gone: it claims nothing now. */
    settle(id: string): void {
      starts.delete(id);
      for (const [p, who] of owners) {
        who.delete(id);
        if (!who.size) owners.delete(p);
      }
    },
  };
}

export type FolderWork = ReturnType<typeof createFolderWork>;
