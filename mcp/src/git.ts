/* "What changed" evidence for card_done: working-tree paths from git status.
 * Best-effort — no git, not a repo, or any git failure just yields []. */

import { execFileSync } from "node:child_process";

/** Without this git writes a non-ASCII name as a quoted octal escape, and the same file shows up twice. */
const PLAIN = ["-c", "core.quotepath=off"];

export function changedFiles(dir: string): string[] {
  let out: string;
  try {
    out = execFileSync("git", [...PLAIN, "status", "--porcelain"], {
      cwd: dir,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
  } catch {
    return [];
  }
  return out
    .split("\n")
    .filter((line) => line.length > 3)
    .map((line) => {
      const p = line.slice(3).trim();
      // rename lines look like "R  old -> new" — the new path is the evidence
      const arrow = p.indexOf(" -> ");
      return arrow >= 0 ? p.slice(arrow + 4) : p;
    })
    .map((p) => p.replace(/^"|"$/g, "")) // git quotes paths with special chars
    .filter(Boolean);
}

/** Where card_done looks for what the agent changed: the folder it runs in
 *  (BYTEHIVE_RUN_DIR, its worktree when isolated), else the board's folder. */
export function evidenceDir(env: Record<string, string | undefined>, boardDir: string): string {
  return env.BYTEHIVE_RUN_DIR?.trim() || boardDir;
}

const run = (cwd: string, ...args: string[]) =>
  execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();

/** What this branch has committed since it left the project's checkout. People
 *  commit before card_done, so git status alone shows nothing of their work. */
export function branchFiles(runDir: string, boardDir: string): string[] {
  try {
    const base = run(boardDir, "rev-parse", "HEAD");
    const fork = run(runDir, "merge-base", "HEAD", base);
    return run(runDir, ...PLAIN, "diff", "--name-only", fork, "HEAD").split("\n").filter(Boolean);
  } catch {
    return [];
  }
}

/** What the owner's hooks and ByteHive write into a working folder. */
const NOISE = /^(\.claude|logs|\.bytehive)\//;

/** Everything a worker changed: committed and not yet committed. */
export function workFiles(runDir: string, boardDir: string): string[] {
  return [...new Set([...branchFiles(runDir, boardDir), ...changedFiles(runDir)])].filter((f) => !NOISE.test(f));
}
