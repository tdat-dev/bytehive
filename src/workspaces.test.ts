import { describe, it, expect } from "vitest";
import { basename, nextWorkspaceName, pickNextActive, needsCloseConfirm, needsWorktree, trustsFolder } from "./workspaces";

describe("trustsFolder", () => {
  it("answers Claude's folder question for a member: the owner approved the hire", () => {
    expect(trustsFolder({ personId: "p1", badge: "claude" })).toBe(true);
  });
  it("leaves it to you for your own agents", () => {
    expect(trustsFolder({ badge: "claude" })).toBe(false);
  });
  it("only Claude asks it", () => {
    expect(trustsFolder({ personId: "p1", badge: "codex" })).toBe(false);
  });
});

describe("needsWorktree", () => {
  const repo = { isolated: false, repoRoot: "D:\\app" };
  it("gives a member one on a git project, whatever the project's setting", () => {
    expect(needsWorktree(repo, { personId: "p1" })).toBe(true);
  });
  it("leaves other agents to the project's setting", () => {
    expect(needsWorktree(repo, {})).toBe(false);
    expect(needsWorktree({ ...repo, isolated: true }, {})).toBe(true);
  });
  it("gives nobody one outside a git project", () => {
    expect(needsWorktree({ isolated: false, repoRoot: null }, { personId: "p1" })).toBe(false);
    expect(needsWorktree({ isolated: true, repoRoot: null }, {})).toBe(false);
  });
  it("gives nobody a second one", () => {
    expect(needsWorktree(repo, { personId: "p1", worktree: "D:\\wt" })).toBe(false);
  });
});

describe("basename", () => {
  it("takes the last path segment, slash or backslash", () => {
    expect(basename("D:\\WhaleloSource\\app")).toBe("app");
    expect(basename("/home/me/proj/")).toBe("proj");
  });
});

describe("nextWorkspaceName", () => {
  it("uses the directory basename when a dir is given", () => {
    expect(nextWorkspaceName("D:\\projects\\api", [])).toBe("api");
  });
  it("falls back to the first free 'Project N'", () => {
    expect(nextWorkspaceName(null, [])).toBe("Project 1");
    expect(nextWorkspaceName(null, ["Project 1", "Project 2"])).toBe("Project 3");
  });
});

describe("pickNextActive", () => {
  it("activates the neighbour after the closed tab", () => {
    expect(pickNextActive(["a", "b", "c"], "b")).toBe("c");
  });
  it("activates the last when the closed tab was last", () => {
    expect(pickNextActive(["a", "b", "c"], "c")).toBe("b");
  });
  it("returns null when closing the only tab", () => {
    expect(pickNextActive(["a"], "a")).toBeNull();
  });
});

describe("needsCloseConfirm", () => {
  it("is true only when terminals are running", () => {
    expect(needsCloseConfirm(0)).toBe(false);
    expect(needsCloseConfirm(3)).toBe(true);
  });
});

import { runDir, sameFolder, agentEnv } from "./workspaces";
describe("runDir", () => {
  it("starts an agent in its worktree, else the project, else home, without a trailing slash", () => {
    expect(runDir({ worktree: "D:\\wt\\a", cwd: "D:\\app" }, "C:\\Users\\me")).toBe("D:\\wt\\a");
    expect(runDir({ cwd: "D:\\app\\" }, "C:\\Users\\me")).toBe("D:\\app");
    expect(runDir({ cwd: null }, "C:\\Users\\me\\")).toBe("C:\\Users\\me");
    expect(runDir({ cwd: "C:\\" }, "")).toBe("C:\\");
    expect(runDir({ cwd: "/home/me/" }, "")).toBe("/home/me");
  });
});

describe("sameFolder", () => {
  it("matches a folder whatever its case, slashes or trailing separator", () => {
    expect(sameFolder("D:\\App\\", "d:/app")).toBe(true);
    expect(sameFolder("D:\\app", "D:\\app2")).toBe(false);
    expect(sameFolder(null, "D:\\app")).toBe(false);
  });
});

describe("agentEnv", () => {
  it("pins the workspace to the project even when the agent runs in a worktree", () => {
    const env = agentEnv({ name: "Ana", cwd: "D:\\app" }, "D:\\.bytehive-worktrees\\app-1\\ana");
    expect(env).toContainEqual(["BYTEHIVE_WORKSPACE", "D:\\app"]);
    expect(env).toContainEqual(["BYTEHIVE_AGENT", "Ana"]);
  });

  it("falls back to the run folder for an agent with no project", () => {
    expect(agentEnv({ name: "Ana", cwd: null }, "C:\\Users\\me")).toContainEqual(["BYTEHIVE_WORKSPACE", "C:\\Users\\me"]);
  });

  it("adds the spec's own variables", () => {
    const env = agentEnv({ name: "Ana", cwd: "D:\\app", env: [["BYTEHIVE_AGENT_ID", "p1"]] }, null);
    expect(env).toContainEqual(["BYTEHIVE_AGENT_ID", "p1"]);
  });

  it("names the folder the agent really works in, apart from the project", () => {
    const env = agentEnv({ name: "Ana", cwd: "D:\\app" }, "D:\\.bytehive-worktrees\\app-1\\ana");
    expect(env).toContainEqual(["BYTEHIVE_RUN_DIR", "D:\\.bytehive-worktrees\\app-1\\ana"]);
  });

  it("works in the project when there is no separate run folder", () => {
    expect(agentEnv({ name: "Ana", cwd: "D:\\app" }, null)).toContainEqual(["BYTEHIVE_RUN_DIR", "D:\\app"]);
  });
});
