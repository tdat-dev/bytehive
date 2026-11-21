import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { memoryDelete, memoryForInstructions, memoryGet, memorySave, memoryUpdate } from "./memory";

let dir = "";
beforeEach(() => { dir = fs.mkdtempSync(path.join(os.tmpdir(), "mem-")); });
afterEach(() => { fs.rmSync(dir, { recursive: true, force: true }); });

describe("project memory tools", () => {
  it("saves, updates and deletes facts in .bytehive/memory.json", () => {
    expect(memoryGet(dir)).toMatch(/empty/);
    expect(memoryForInstructions(dir)).toBe("");
    expect(memorySave(dir, "Run tests with npx vitest run", "Ana", 1)).toMatch(/^Saved \[m1\]/);
    expect(memorySave(dir, "run tests with npx vitest run.", "Bob", 2)).toMatch(/^Already in memory as \[m1\]/);
    memorySave(dir, "The board lives in .bytehive", "Ana", 2);
    expect(memoryUpdate(dir, "m1", "Run tests with npm test", "Bob", 3)).toBe("Updated [m1] Run tests with npm test\n(2 facts in memory)");
    expect(memorySave(dir, "A third fact", "Ana", 4)).not.toContain("board lives");
    memoryDelete(dir, "m2");
    memoryDelete(dir, "m3");
    const file = JSON.parse(fs.readFileSync(path.join(dir, ".bytehive", "memory.json"), "utf8"));
    expect(file.entries[0]).toMatchObject({ id: "m1", by: "Bob", at: 3 });
    expect(memoryForInstructions(dir)).toContain("PROJECT MEMORY");
    expect(memoryDelete(dir, "m1")).toMatch(/^Deleted \[m1\]/);
    expect(memoryGet(dir)).toMatch(/empty/);
  });
});
