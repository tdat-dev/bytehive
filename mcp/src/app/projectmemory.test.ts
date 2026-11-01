import { describe, expect, it } from "vitest";
import { addFact, deleteFact, emptyMemory, looksSecret, memoryText, parseMemory, serializeMemory, updateFact, MAX_ENTRIES } from "./projectmemory";

describe("project memory", () => {
  it("adds, edits and deletes facts with short ids", () => {
    const m = emptyMemory();
    const a = addFact(m, "Tests run with npx vitest run.", "Ana", 1);
    const b = addFact(m, "The board lives in .maestro/board.json", "you", 2);
    expect([a.id, b.id]).toEqual(["m1", "m2"]);
    updateFact(m, "m1", "Tests: npx vitest run, typecheck: npx tsc --noEmit", "Bob", 3);
    expect(m.entries[0]).toMatchObject({ text: "Tests: npx vitest run, typecheck: npx tsc --noEmit", by: "Bob", at: 3 });
    deleteFact(m, "m1");
    expect(addFact(m, "Another fact", "you", 4).id).toBe("m3");
    expect(() => deleteFact(m, "m9")).toThrow(/No memory entry/);
  });

  it("keeps the same fact once", () => {
    const m = emptyMemory();
    addFact(m, "Use pnpm, not npm.", "Ana", 1);
    addFact(m, "use pnpm,  not npm", "Bob", 2);
    expect(m.entries).toHaveLength(1);
  });

  it("refuses secrets, empty facts, long ones and a full memory", () => {
    const m = emptyMemory();
    expect(looksSecret("the key is sk-ant-api03-abcdefghijklmnopqrstuv")).toBe(true);
    expect(looksSecret("password = hunter2hunter2")).toBe(true);
    expect(looksSecret("Login goes through the password reset page")).toBe(false);
    expect(() => addFact(m, "token: ghp_abcdefghijklmnopqrstuvwxyz0123", "Ana", 1)).toThrow(/secrets/);
    expect(() => addFact(m, "   ", "Ana", 1)).toThrow(/empty/);
    expect(() => addFact(m, "x".repeat(700), "Ana", 1)).toThrow(/under/);
    for (let i = 0; i < MAX_ENTRIES; i++) addFact(m, `fact ${i}`, "Ana", i);
    expect(() => addFact(m, "one more", "Ana", 99)).toThrow(/full/);
  });

  it("reads its file forgivingly and round-trips it", () => {
    expect(parseMemory("").entries).toEqual([]);
    expect(parseMemory('{"entries":[{"text":"  a  "},{"text":""},{"id":"m7","text":"b","at":5,"by":"Ana"}]}').entries)
      .toEqual([{ id: "m1", text: "a", at: 0, by: "" }, { id: "m7", text: "b", at: 5, by: "Ana" }]);
    expect(() => parseMemory("{oops")).toThrow(/not valid JSON/);
    const m = emptyMemory();
    addFact(m, "a fact", "you", 1);
    expect(parseMemory(serializeMemory(m))).toEqual(m);
  });

  it("gives agents one line per fact with its id", () => {
    const m = emptyMemory();
    addFact(m, "line one\nline two", "you", 1);
    expect(memoryText(m)).toBe("[m1] line one line two");
    expect(memoryText(emptyMemory())).toMatch(/empty/);
  });
});
