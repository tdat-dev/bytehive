import { describe, it, expect } from "vitest";
import { createFolderWork } from "./folderwork";

const look = (files: Record<string, string> = {}, head = "h1", committed: string[] = []) => ({ head, files, committed });

describe("folder work", () => {
  it("what was dirty before the first look is nobody's", () => {
    const w = createFolderWork();
    w.see(look({ "old.txt": "1" }), ["ana"]);
    w.see(look({ "old.txt": "1" }), ["ana"]);
    expect(w.of("ana").paths).toEqual([]);
  });

  it("a file that moved while one agent worked is that agent's", () => {
    const w = createFolderWork();
    w.see(look(), []);
    w.see(look({ "a.txt": "1" }), ["ana"]);
    expect(w.of("ana")).toEqual({ from: "h1", paths: ["a.txt"], shared: 0 });
    expect(w.of("bo").paths).toEqual([]);
  });

  it("a file that moved while nobody worked is nobody's", () => {
    const w = createFolderWork();
    w.see(look(), []);
    w.see(look({ "a.txt": "1" }), []);
    expect(w.of("ana").paths).toEqual([]);
  });

  it("two agents at work both keep the file, and it is marked shared", () => {
    const w = createFolderWork();
    w.see(look(), []);
    w.see(look({ "a.txt": "1" }), ["ana", "bo"]);
    w.see(look({ "a.txt": "1", "b.txt": "1" }), ["bo"]);
    expect(w.of("ana")).toMatchObject({ paths: ["a.txt"], shared: 1 });
    expect(w.of("bo")).toMatchObject({ paths: ["a.txt", "b.txt"], shared: 1 });
  });

  it("an agent's own commit stays its work, measured from before it", () => {
    const w = createFolderWork();
    w.see(look({}, "h1"), []);
    w.see(look({ "a.txt": "1" }, "h1"), ["ana"]);
    w.see(look({}, "h2", ["a.txt", "b.txt"]), ["ana"]);
    expect(w.of("ana")).toEqual({ from: "h1", paths: ["a.txt", "b.txt"], shared: 0 });
  });

  it("a commit with no agent at work is you taking the work", () => {
    const w = createFolderWork();
    w.see(look({}, "h1"), []);
    w.see(look({ "a.txt": "1", "b.txt": "1" }, "h1"), ["ana"]);
    w.see(look({ "b.txt": "1" }, "h2", ["a.txt"]), []);
    expect(w.of("ana").paths).toEqual(["b.txt"]);
  });

  it("settling an agent clears it and leaves the other's claim alone", () => {
    const w = createFolderWork();
    w.see(look(), []);
    w.see(look({ "a.txt": "1" }), ["ana", "bo"]);
    w.settle("ana");
    expect(w.of("ana").paths).toEqual([]);
    expect(w.of("bo")).toMatchObject({ paths: ["a.txt"], shared: 0 });
  });

  it("a later job is measured from where the last one was settled", () => {
    const w = createFolderWork();
    w.see(look({}, "h1"), []);
    w.see(look({ "a.txt": "1" }, "h1"), ["ana"]);
    w.settle("ana");
    w.see(look({ "a.txt": "1" }, "h2"), []);
    w.see(look({ "a.txt": "2" }, "h2"), ["ana"]);
    expect(w.of("ana")).toEqual({ from: "h2", paths: ["a.txt"], shared: 0 });
  });
});
