import { describe, expect, it } from "vitest";
import { conflictText, diffTotals, numberHunk, sendBackText } from "./reviewtext";

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
