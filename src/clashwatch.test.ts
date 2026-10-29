import { describe, expect, it } from "vitest";
import { clashHistory, clashNotes, clashShort, type ClashInput } from "./clashwatch";

let n = 0;
const agent = (over: Partial<ClashInput> = {}): ClashInput => ({
  paneId: `p${++n}`, name: `Agent${n}`, wsId: "ws-1", state: "working", running: true,
  title: null, race: null, paths: [], ...over,
});

describe("file clashes", () => {
  it("warns every agent that shares a file in one project", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", paths: ["src/app.ts"] }),
      agent({ paneId: "b", name: "Bo", paths: ["src/app.ts", "src/lib.ts"] }),
    ]);
    expect(notes.get("a")).toMatchObject({ kind: "file", with: ["Bo"], files: ["src/app.ts"] });
    expect(notes.get("b")).toMatchObject({ kind: "file", with: ["Ana"], files: ["src/app.ts"] });
  });

  it("compares paths case-insensitively across separators", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", paths: ["SRC\\App.ts"] }),
      agent({ paneId: "b", name: "Bo", paths: ["src/app.ts"] }),
    ]);
    expect(notes.get("a")?.kind).toBe("file");
    expect(notes.get("b")?.kind).toBe("file");
  });

  it("ignores agents in different projects", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", paths: ["src/app.ts"] }),
      agent({ paneId: "b", name: "Bo", wsId: "ws-2", paths: ["src/app.ts"] }),
    ]);
    expect(notes.size).toBe(0);
  });

  it("ignores agents that stopped or merged", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", paths: ["src/app.ts"] }),
      agent({ paneId: "b", name: "Bo", running: false, paths: ["src/app.ts"] }),
      agent({ paneId: "c", name: "Cy", state: "merged", paths: ["src/app.ts"] }),
    ]);
    expect(notes.size).toBe(0);
  });

  it("keeps a stable key for the same overlap", () => {
    const inputs = () => [
      agent({ paneId: "a", name: "Ana", paths: ["src/app.ts", "src/b.ts"] }),
      agent({ paneId: "b", name: "Bo", paths: ["src/b.ts", "src/app.ts"] }),
    ];
    expect(clashNotes(inputs()).get("a")?.key).toBe(clashNotes(inputs()).get("a")?.key);
  });
});

describe("task dupes", () => {
  it("warns agents running the same task", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", title: "Fix login" }),
      agent({ paneId: "b", name: "Bo", title: "  fix   LOGIN " }),
    ]);
    expect(notes.get("a")).toMatchObject({ kind: "task", with: ["Bo"] });
    expect(notes.get("b")).toMatchObject({ kind: "task", with: ["Ana"] });
  });

  it("skips empty titles", () => {
    const notes = clashNotes([agent({ paneId: "a" }), agent({ paneId: "b" })]);
    expect(notes.size).toBe(0);
  });

  it("a race is on purpose, not a dupe", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", title: "Fix login", race: "r1" }),
      agent({ paneId: "b", name: "Bo", title: "Fix login", race: "r1" }),
      agent({ paneId: "c", name: "Cy", title: "Fix login", race: "r2" }),
    ]);
    expect(notes.get("a")?.with).toEqual(["Cy"]);
    expect(notes.get("b")?.with).toEqual(["Cy"]);
    expect(notes.get("c")?.with).toEqual(["Ana", "Bo"]);
  });

  it("a file clash wins over a task dupe on one agent", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", title: "Fix login", paths: ["src/app.ts"] }),
      agent({ paneId: "b", name: "Bo", title: "Fix login", paths: ["src/app.ts"] }),
    ]);
    expect(notes.get("a")?.kind).toBe("file");
  });
});

describe("wording", () => {
  it("names who and what, capped", () => {
    const notes = clashNotes([
      agent({ paneId: "a", name: "Ana", paths: ["a.ts", "b.ts", "c.ts"] }),
      agent({ paneId: "b", name: "Bo", paths: ["a.ts", "b.ts", "c.ts"] }),
      agent({ paneId: "c", name: "Cy", paths: ["a.ts", "b.ts", "c.ts"] }),
      agent({ paneId: "d", name: "Dee", paths: ["a.ts", "b.ts", "c.ts"] }),
    ]);
    const note = notes.get("a")!;
    expect(clashShort(note)).toBe("overlaps Bo, Cy +1 more in a.ts, b.ts +1 more");
    expect(clashHistory(note)).toBe("Overlaps Bo, Cy +1 more in a.ts, b.ts +1 more");
    const dupe = clashNotes([
      agent({ paneId: "x", name: "Xia", title: "T" }),
      agent({ paneId: "y", name: "Yara", title: "T" }),
    ]).get("x")!;
    expect(clashShort(dupe)).toBe("same task as Yara");
  });
});
