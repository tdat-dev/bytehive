// The session store: sealed copies of a machine's sessions, kept so any device of
// the account can read them while the machine is off. Everything here is sealed
// by the machine with a key the relay never sees; the relay only checks shapes
// and sizes, and keeps who wrote what.

import { STORE } from "./limits";

/** A session's id on the relay: an HMAC the machine makes, 32 hex characters. */
export const SID = /^[0-9a-f]{32}$/;
/** A sealed blob: base64url, nothing else. */
const SEALED = /^[A-Za-z0-9_-]+$/;

export interface Vault { salt: string; rounds: number; check: string }
export interface Segment { n: number; sealed: string }
export interface Put { sid: string; meta: string; updated: number; segments: Segment[] }
export interface Listed { sid: string; machine: string; meta: string; updated: number; segments: number }

export function initStore(sql: SqlStorage): void {
  sql.exec(`
    CREATE TABLE IF NOT EXISTS vault (id INTEGER PRIMARY KEY CHECK (id = 1), salt TEXT, rounds INTEGER, chk TEXT);
    CREATE TABLE IF NOT EXISTS sessions (sid TEXT PRIMARY KEY, machine TEXT, meta TEXT, updated INTEGER, bytes INTEGER);
    CREATE TABLE IF NOT EXISTS segments (sid TEXT, n INTEGER, sealed TEXT, PRIMARY KEY (sid, n));
  `);
}

const sealed = (s: unknown, max: number): s is string => typeof s === "string" && s.length > 0 && s.length <= max && SEALED.test(s);

export function getVault(sql: SqlStorage): Vault | null {
  const r = sql.exec<{ salt: string; rounds: number; chk: string }>("SELECT salt, rounds, chk FROM vault WHERE id = 1").toArray()[0];
  return r ? { salt: r.salt, rounds: r.rounds, check: r.chk } : null;
}

/** A new vault makes every stored session unreadable, so replacing one drops them all. */
export function setVault(sql: SqlStorage, v: unknown, replace: boolean): "ok" | "exists" | "bad" {
  const x = v as Partial<Vault> | null;
  if (!x || !sealed(x.salt, 64) || !sealed(x.check, 256) || !Number.isInteger(x.rounds) || x.rounds! < 100_000 || x.rounds! > 10_000_000) return "bad";
  if (getVault(sql) && !replace) return "exists";
  sql.exec("DELETE FROM segments");
  sql.exec("DELETE FROM sessions");
  sql.exec("INSERT OR REPLACE INTO vault (id, salt, rounds, chk) VALUES (1, ?, ?, ?)", x.salt, x.rounds, x.check);
  return "ok";
}

function total(sql: SqlStorage): number {
  return sql.exec<{ t: number | null }>("SELECT SUM(bytes) AS t FROM sessions").toArray()[0]?.t ?? 0;
}

/** Store a session's header and the segments that changed, for the machine that owns it. */
export function put(sql: SqlStorage, machine: string, p: unknown): "ok" | "bad" | "quota" | "taken" {
  const x = p as Partial<Put> | null;
  if (!x || typeof x.sid !== "string" || !SID.test(x.sid) || !sealed(x.meta, STORE.metaBytes)) return "bad";
  if (!Number.isFinite(x.updated) || !Array.isArray(x.segments) || x.segments.length > STORE.segmentsPerPut) return "bad";
  for (const s of x.segments) {
    if (!s || !Number.isInteger(s.n) || s.n < 0 || s.n > STORE.maxSegment || !sealed(s.sealed, STORE.segmentBytes)) return "bad";
  }
  const owner = sql.exec<{ machine: string; bytes: number }>("SELECT machine, bytes FROM sessions WHERE sid = ?", x.sid).toArray()[0];
  if (owner && owner.machine !== machine) return "taken";
  // The session's size after this put: its new header, the segments kept, the segments sent.
  const sizes = new Map(sql.exec<{ n: number; l: number }>("SELECT n, LENGTH(sealed) AS l FROM segments WHERE sid = ?", x.sid).toArray().map((r) => [r.n, r.l]));
  for (const s of x.segments) sizes.set(s.n, s.sealed.length);
  const bytes = x.meta.length + [...sizes.values()].reduce((a, b) => a + b, 0);
  if (total(sql) - (owner?.bytes ?? 0) + bytes > STORE.totalBytes) return "quota";
  for (const s of x.segments) sql.exec("INSERT OR REPLACE INTO segments (sid, n, sealed) VALUES (?, ?, ?)", x.sid, s.n, s.sealed);
  sql.exec("INSERT OR REPLACE INTO sessions (sid, machine, meta, updated, bytes) VALUES (?, ?, ?, ?, ?)", x.sid, machine, x.meta, Math.floor(x.updated!), bytes);
  return "ok";
}

export function drop(sql: SqlStorage, machine: string, sid: unknown): void {
  if (typeof sid !== "string" || !SID.test(sid)) return;
  if (sql.exec("SELECT 1 FROM sessions WHERE sid = ? AND machine = ?", sid, machine).toArray().length === 0) return;
  sql.exec("DELETE FROM segments WHERE sid = ?", sid);
  sql.exec("DELETE FROM sessions WHERE sid = ?", sid);
}

export function list(sql: SqlStorage): Listed[] {
  return sql
    .exec<{ sid: string; machine: string; meta: string; updated: number; segments: number }>(
      "SELECT s.sid, s.machine, s.meta, s.updated, (SELECT COUNT(*) FROM segments g WHERE g.sid = s.sid) AS segments FROM sessions s ORDER BY s.updated DESC",
    )
    .toArray();
}

/** Segments from `from` on, as many as fit in one page. */
export function segments(sql: SqlStorage, sid: string, from: number): { segments: Segment[]; more: boolean } {
  if (!SID.test(sid) || !Number.isInteger(from) || from < 0) return { segments: [], more: false };
  const out: Segment[] = [];
  let bytes = 0;
  for (const r of sql.exec<{ n: number; sealed: string }>("SELECT n, sealed FROM segments WHERE sid = ? AND n >= ? ORDER BY n", sid, from)) {
    if (out.length && bytes + r.sealed.length > STORE.pageBytes) return { segments: out, more: true };
    out.push({ n: r.n, sealed: r.sealed });
    bytes += r.sealed.length;
  }
  return { segments: out, more: false };
}

/** Everything a revoked machine wrote goes with it. */
export function dropMachine(sql: SqlStorage, machine: string): void {
  sql.exec("DELETE FROM segments WHERE sid IN (SELECT sid FROM sessions WHERE machine = ?)", machine);
  sql.exec("DELETE FROM sessions WHERE machine = ?", machine);
}
