import { describe, it, expect } from "vitest";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { branchFiles, evidenceDir, workFiles } from "./git.js";

describe("evidenceDir", () => {
  it("reads the agent's run folder when ByteHive gave one", () => {
    expect(evidenceDir({ BYTEHIVE_RUN_DIR: "D:\\wt\\ana" }, "D:\\app")).toBe("D:\\wt\\ana");
  });

  it("falls back to the board's folder", () => {
    expect(evidenceDir({}, "D:\\app")).toBe("D:\\app");
    expect(evidenceDir({ BYTEHIVE_RUN_DIR: "  " }, "D:\\app")).toBe("D:\\app");
  });
});

function repo(): { main: string; wt: string; git: (cwd: string, ...a: string[]) => void } {
  const main = mkdtempSync(join(tmpdir(), "bh-main-"));
  const git = (cwd: string, ...a: string[]) => { execFileSync("git", a, { cwd, stdio: "pipe" }); };
  git(main, "init", "-q", "-b", "main");
  git(main, "config", "user.email", "t@t.t");
  git(main, "config", "user.name", "t");
  writeFileSync(join(main, "a.txt"), "a");
  git(main, "add", ".");
  git(main, "commit", "-qm", "a");
  const wt = join(main, "..", `${basename(main)}-wt`);
  git(main, "worktree", "add", "-q", "-b", "ana", wt);
  return { main, wt, git };
}

describe("what a worker changed", () => {
  it("includes what they committed on their branch", () => {
    const { main, wt, git } = repo();
    writeFileSync(join(wt, "b.txt"), "b");
    git(wt, "add", ".");
    git(wt, "commit", "-qm", "b");
    expect(branchFiles(wt, main)).toEqual(["b.txt"]);
    expect(workFiles(wt, main)).toEqual(["b.txt"]);
  });
  it("adds what is not committed yet, once", () => {
    const { main, wt, git } = repo();
    writeFileSync(join(wt, "b.txt"), "b");
    git(wt, "add", ".");
    git(wt, "commit", "-qm", "b");
    writeFileSync(join(wt, "b.txt"), "changed");
    writeFileSync(join(wt, "c.txt"), "c");
    expect(workFiles(wt, main).sort()).toEqual(["b.txt", "c.txt"]);
  });
  it("leaves out what hooks write", () => {
    const { main, wt } = repo();
    mkdirSync(join(wt, ".claude"));
    writeFileSync(join(wt, ".claude", "x.json"), "{}");
    mkdirSync(join(wt, "logs"));
    writeFileSync(join(wt, "logs", "y.log"), "y");
    writeFileSync(join(wt, "real.txt"), "r");
    expect(workFiles(wt, main)).toEqual(["real.txt"]);
  });
  it("is what git status says for an agent in the project folder", () => {
    const { main } = repo();
    writeFileSync(join(main, "z.txt"), "z");
    expect(workFiles(main, main)).toEqual(["z.txt"]);
  });
  it("is nothing outside a repo", () => {
    const dir = mkdtempSync(join(tmpdir(), "bh-none-"));
    expect(workFiles(dir, dir)).toEqual([]);
  });
});
