import { describe, expect, it } from "vitest";
import { clearComments, commentWhere, commentsOf, conflictText, diffTotals, dropComment, numberHunk, putComment, rankFiles, riskOf, sendBackText, type LineComment } from "./reviewtext";

describe("send back", () => {
  const conflict = { base: "main", branch: "maestro/eli-1a2b3c", files: ["src/a.ts", "src/b.ts"] };

  it("lists the conflicting files and asks the agent to merge the base in and resolve them", () => {
    const t = conflictText(conflict);
    expect(t).toContain("Merging your branch maestro/eli-1a2b3c into main conflicts in:\n- src/a.ts\n- src/b.ts");
    expect(t).toContain("git merge main");
    expect(t).toContain("resolve the conflicts");
    expect(t).not.toContain("—");
  });

  it("types one message: the conflict first, then your note; nothing when both are empty", () => {
    expect(sendBackText({ note: "  add a test  " })).toBe("add a test");
    const both = sendBackText({ note: "and keep the old name", conflict });
    expect(both.startsWith("Merging your branch")).toBe(true);
    expect(both.endsWith("\n\nand keep the old name")).toBe(true);
    expect(sendBackText({ note: "   " })).toBe("");
    expect(sendBackText({})).toBe("");
  });
});

describe("diff line numbers", () => {
  it("numbers each side from the hunk header", () => {
    const lines = numberHunk("@@ -10,3 +10,4 @@ fn x()", [
      { kind: "ctx", text: "a" },
      { kind: "del", text: "b" },
      { kind: "add", text: "B" },
      { kind: "add", text: "C" },
      { kind: "ctx", text: "d" },
    ]);
    expect(lines.map((l) => [l.old, l.new])).toEqual([[10, 10], [11, null], [null, 11], [null, 12], [12, 13]]);
  });

  it("counts a new file from line 1 and totals a diff", () => {
    expect(numberHunk("@@ -0,0 +1 @@", [{ kind: "add", text: "x" }])[0].new).toBe(1);
    expect(diffTotals([{ path: "a", additions: 3, deletions: 1, hunks: [] }, { path: "b", additions: 2, deletions: 0, hunks: [] }])).toEqual({ add: 5, del: 1 });
  });
});

describe("comments on the diff", () => {
  const c = (over: Partial<LineComment>): LineComment => ({ id: "x", path: "src/a.ts", side: "new", start: 12, end: 12, code: ["const x = 1;"], text: "Name it", ...over });

  it("says where a comment is", () => {
    expect(commentWhere(c({}))).toBe("src/a.ts:12");
    expect(commentWhere(c({ start: 12, end: 14 }))).toBe("src/a.ts:12-14");
    expect(commentWhere(c({ side: "old", start: 7, end: 7 }))).toBe("src/a.ts:7 (removed)");
  });

  it("types every comment in one message: where, the quoted code, the comment, then what to do", () => {
    const t = sendBackText({
      note: "Also run the linter.",
      comments: [
        c({ path: "src/b.ts", start: 3, end: 4, code: ["if (a)", "  go();"], text: "Guard b too" }),
        c({ text: "Rename x to count" }),
        c({ text: "   " }), // an empty one is not sent
      ],
    });
    expect(t).toBe([
      "Review comments on your changes:",
      "",
      "src/a.ts:12\n> const x = 1;\nRename x to count",
      "",
      "src/b.ts:3-4\n> if (a)\n>   go();\nGuard b too",
      "",
      "Also run the linter.",
      "",
      "Address each comment, then tell me when it is done.",
    ].join("\n"));
    expect(t).not.toContain("\u2014");
  });

  it("keeps drafts per agent until they are cleared", () => {
    const a = putComment("ana", { path: "a.ts", side: "new", start: 1, end: 1, code: ["x"], text: "one" });
    putComment("ana", { path: "a.ts", side: "new", start: 2, end: 2, code: ["y"], text: "two" });
    putComment("bo", { path: "b.ts", side: "old", start: 5, end: 5, code: ["z"], text: "why?" });
    putComment("ana", { ...a, text: "one, edited" });
    expect(commentsOf("ana").map((x) => x.text)).toEqual(["one, edited", "two"]);
    dropComment("ana", a.id);
    expect(commentsOf("ana").map((x) => x.text)).toEqual(["two"]);
    clearComments("ana");
    expect(commentsOf("ana")).toEqual([]);
    expect(commentsOf("bo")).toHaveLength(1);
    clearComments("bo");
  });
});

describe("review risk order", () => {
  it("ranks secrets above auth above code above tests above docs", () => {
    const paths = ["docs/guide.md", "src/app.test.ts", "src/app.ts", "src/auth/login.ts", ".env"];
    const ranked = [...paths].sort((a, b) => riskOf(b) - riskOf(a));
    expect(ranked).toEqual([".env", "src/auth/login.ts", "src/app.ts", "src/app.test.ts", "docs/guide.md"]);
  });

  it("ranks manifests and infra with config, and keeps diff order inside a tier", () => {
    expect(riskOf("package.json")).toBeGreaterThan(riskOf("src/app.ts"));
    expect(riskOf("Dockerfile")).toBeGreaterThan(riskOf("src/app.ts"));
    const files = [{ path: "b.ts" }, { path: "a.ts" }];
    expect(rankFiles(files).map((f) => f.path)).toEqual(["b.ts", "a.ts"]);
  });
});
