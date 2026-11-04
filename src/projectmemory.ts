/* Project memory: what every agent in a project should know about it
 * (conventions, decisions, where things are, gotchas), one fact per entry, in
 * <project>/.bytehive/memory.json. Agents read and save it through the bytehive
 * MCP tools (mcp/src/memory.ts); you read and edit it in ByteHive
 * (memoryview.ts). Pure: no file access here, so both sides share it. */

export interface MemoryEntry {
  id: string;
  text: string;
  /** ms when it was written or last changed */
  at: number;
  /** who wrote it: an agent's name, or "you" */
  by: string;
}

export interface ProjectMemory {
  version: 1;
  entries: MemoryEntry[];
}

export const MEMORY_REL = ".bytehive/memory.json";
/** A fact is a sentence or two, not a document. */
export const MAX_FACT = 600;
/** Enough for what a project needs every agent to know, small enough to read
 *  at the start of every session. */
export const MAX_ENTRIES = 80;

export class MemoryError extends Error {}

export const emptyMemory = (): ProjectMemory => ({ version: 1, entries: [] });

/** Reads memory.json, forgiving: a missing, empty or unknown file is an empty
 *  memory, and entries without text are dropped. */
export function parseMemory(raw: string | null | undefined): ProjectMemory {
  if (!raw || !raw.trim()) return emptyMemory();
  let data: unknown;
  try { data = JSON.parse(raw); } catch { throw new MemoryError("The project memory file (.bytehive/memory.json) is not valid JSON."); }
  const list = Array.isArray((data as ProjectMemory)?.entries) ? (data as ProjectMemory).entries : [];
  const entries: MemoryEntry[] = [];
  for (const e of list) {
    if (!e || typeof e.text !== "string" || !e.text.trim()) continue;
    entries.push({
      id: typeof e.id === "string" && e.id ? e.id : newId(entries),
      text: e.text.trim(),
      at: typeof e.at === "number" ? e.at : 0,
      by: typeof e.by === "string" ? e.by : "",
    });
  }
  return { version: 1, entries };
}

export const serializeMemory = (m: ProjectMemory): string => JSON.stringify(m, null, 2) + "\n";

/** Short ids an agent can type back: m1, m2, … never reused while the entry lives. */
function newId(entries: MemoryEntry[]): string {
  let n = 0;
  for (const e of entries) { const k = /^m(\d+)$/.exec(e.id); if (k) n = Math.max(n, Number(k[1])); }
  return `m${n + 1}`;
}

/** Things that look like a key, a token or a password. Memory is read by
 *  every agent and sits in a plain file, so they never go in. */
const SECRET = [
  /\bsk-[A-Za-z0-9_-]{16,}/, /\bgh[pousr]_[A-Za-z0-9]{20,}/, /\bgithub_pat_[A-Za-z0-9_]{20,}/,
  /\bAKIA[0-9A-Z]{16}\b/, /\bxox[abpr]-[A-Za-z0-9-]{10,}/, /\bAIza[0-9A-Za-z_-]{30,}/,
  /\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/, /-----BEGIN [A-Z ]*PRIVATE KEY-----/,
  /\b(password|passwd|pwd|secret|api[_-]?key|token)\s*[:=]\s*\S{6,}/i,
];
export const looksSecret = (text: string): boolean => SECRET.some((r) => r.test(text));

const norm = (s: string) => s.toLowerCase().replace(/\s+/g, " ").replace(/[.\s]+$/, "").trim();

function cleanFact(text: string): string {
  const t = text.trim().replace(/\s+\n/g, "\n");
  if (!t) throw new MemoryError("The fact is empty.");
  if (t.length > MAX_FACT) throw new MemoryError(`Keep a fact under ${MAX_FACT} characters: one fact per entry.`);
  if (looksSecret(t)) throw new MemoryError("That looks like a key, a token or a password. Project memory is a plain file every agent reads, so it never holds secrets.");
  return t;
}

/** Adds a fact. The same fact twice is kept once. */
export function addFact(m: ProjectMemory, text: string, by: string, now: number): MemoryEntry {
  const t = cleanFact(text);
  const same = m.entries.find((e) => norm(e.text) === norm(t));
  if (same) return same;
  if (m.entries.length >= MAX_ENTRIES) throw new MemoryError(`Project memory is full (${MAX_ENTRIES} facts). Update or delete an old one first.`);
  const e: MemoryEntry = { id: newId(m.entries), text: t, at: now, by };
  m.entries.push(e);
  return e;
}

function find(m: ProjectMemory, id: string): MemoryEntry {
  const e = m.entries.find((x) => x.id === id.trim());
  if (!e) throw new MemoryError(`No memory entry "${id}". Read the list with memory_get.`);
  return e;
}

export function updateFact(m: ProjectMemory, id: string, text: string, by: string, now: number): MemoryEntry {
  const e = find(m, id);
  e.text = cleanFact(text);
  e.at = now;
  e.by = by;
  return e;
}

export function deleteFact(m: ProjectMemory, id: string): MemoryEntry {
  const e = find(m, id);
  m.entries = m.entries.filter((x) => x !== e);
  return e;
}

/** Memory as an agent reads it: one line per fact with its id. */
export function memoryText(m: ProjectMemory): string {
  if (!m.entries.length) return "Project memory is empty. Save lasting facts about this project with memory_save.";
  return m.entries.map((e) => `[${e.id}] ${e.text.replace(/\n+/g, " ")}`).join("\n");
}
