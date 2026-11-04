/* Project memory for maestro-mcp: <workspace>/.maestro/memory.json, the facts
 * every agent in the project should know. The shapes and rules live in the
 * app (src/projectmemory.ts) so Maestro's editor and the agents agree. Every
 * call re-reads the file and writes atomically, like the board. */

import fs from "node:fs";
import path from "node:path";
import { addFact, deleteFact, memoryText, parseMemory, serializeMemory, updateFact, MEMORY_REL, type MemoryEntry, type ProjectMemory } from "../../src/projectmemory";

export const memoryPath = (dir: string): string => path.join(dir, ...MEMORY_REL.split("/"));

export function loadMemory(dir: string): ProjectMemory {
  let raw: string;
  try {
    raw = fs.readFileSync(memoryPath(dir), "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return parseMemory("");
    throw e;
  }
  try {
    return parseMemory(raw);
  } catch {
    // Maestro's own write is not atomic: give a torn read one short retry.
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 50);
    return parseMemory(fs.readFileSync(memoryPath(dir), "utf8"));
  }
}

export function saveMemory(dir: string, m: ProjectMemory): void {
  const p = memoryPath(dir);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  const tmp = `${p}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, serializeMemory(m), "utf8");
  fs.renameSync(tmp, p);
}

/** What the agent gets back after a change: the entry, then the whole list. */
const after = (verb: string, e: MemoryEntry, m: ProjectMemory) => `${verb} [${e.id}].\n\n${memoryText(m)}`;

export function memoryGet(dir: string): string {
  return memoryText(loadMemory(dir));
}

export function memorySave(dir: string, fact: string, by: string, now: number): string {
  const m = loadMemory(dir);
  const had = m.entries.length;
  const e = addFact(m, fact, by, now);
  if (m.entries.length === had) return after("Already in memory as", e, m);
  saveMemory(dir, m);
  return after("Saved", e, m);
}

export function memoryUpdate(dir: string, id: string, fact: string, by: string, now: number): string {
  const m = loadMemory(dir);
  const e = updateFact(m, id, fact, by, now);
  saveMemory(dir, m);
  return after("Updated", e, m);
}

export function memoryDelete(dir: string, id: string): string {
  const m = loadMemory(dir);
  const e = deleteFact(m, id);
  saveMemory(dir, m);
  return `Deleted [${e.id}].\n\n${memoryText(m)}`;
}

/** The memory, for the server's instructions at start: the agent knows the
 *  project before its first tool call. Empty when there is none yet. */
export function memoryForInstructions(dir: string): string {
  try {
    const m = loadMemory(dir);
    if (!m.entries.length) return "";
    return `\n\nPROJECT MEMORY (what every agent here should know; ids for memory_update):\n${memoryText(m)}`;
  } catch {
    return "";
  }
}
