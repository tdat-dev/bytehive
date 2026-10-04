import { describe, expect, it } from "vitest";
import { matching, placeOf } from "./sessionsview";
import type { ClaudeSession } from "./ipc";

const s = (id: string, title: string, cwd: string): ClaudeSession => ({ id, title, cwd, messages: 1, modified_ms: 0 });
const APP = String.raw`D:\app`;

describe("placeOf", () => {
  it("says nothing for the project itself, the worktree's name for an agent's", () => {
    expect(placeOf(APP, APP)).toBe("");
    expect(placeOf("D:\\app\\", APP)).toBe("");
    expect(placeOf(String.raw`D:\app\.claude\worktrees\fix-login`, APP)).toBe("worktree fix-login");
    expect(placeOf(String.raw`D:\.maestro-worktrees\app-o703vx\maestro-bob-nryjdb-iqnrko`, APP)).toBe("worktree maestro-bob-nryjdb");
    expect(placeOf("D:/code/other", null)).toBe("code/other");
    expect(placeOf("", APP)).toBe("");
  });
});

describe("matching", () => {
  const list = [s("1", "Fix the login page", APP), s("2", "Release notes", String.raw`D:\app\.claude\worktrees\docs`)];
  it("keeps sessions with every word in the title or folder", () => {
    expect(matching(list, "").length).toBe(2);
    expect(matching(list, "LOGIN fix").map((x) => x.id)).toEqual(["1"]);
    expect(matching(list, "docs").map((x) => x.id)).toEqual(["2"]);
    expect(matching(list, "login docs")).toEqual([]);
  });
});
