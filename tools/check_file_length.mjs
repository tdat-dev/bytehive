// File-length gate: no source file grows past LIMIT lines.
//
// Why: a file heading past ~500 lines is doing more than one job. Splitting
// by responsibility (OOP/SOLID, one module one reason to change) keeps every
// file reviewable in one sitting and cheap to maintain.
//
// Enforcement is a ratchet, not a flag day: files already over the limit are
// listed in file_length_allowlist.json with their current size. A listed file
// fails only if it GREW past its recorded size; anything else over the limit
// fails outright. The allowlist only ever shrinks: split a listed file and
// drop it (or lower its number) in the same change.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const LIMIT = 500;
const ALLOWLIST = join(dirname(fileURLToPath(import.meta.url)), "file_length_allowlist.json");
const ROOTS = ["crates", "mcp/src"];
const EXTS = new Set([".ts", ".rs"]);
const SKIP = new Set(["node_modules", "target", "dist"]);

function walk(dir, out) {
  for (const name of readdirSync(dir)) {
    if (SKIP.has(name)) continue;
    const full = join(dir, name);
    if (statSync(full).isDirectory()) {
      walk(full, out);
    } else if ([...EXTS].some((e) => name.endsWith(e))) {
      out.push(full);
    }
  }
  return out;
}

function lineCount(text) {
  if (!text) return 0;
  const n = text.split("\n").length;
  return text.endsWith("\n") ? n - 1 : n;
}

const allow = JSON.parse(readFileSync(ALLOWLIST, "utf8"));
const files = ROOTS.flatMap((r) => walk(join(ROOT, r), []));
let failed = false;

for (const full of files) {
  const rel = relative(ROOT, full).replace(/\\/g, "/");
  const n = lineCount(readFileSync(full, "utf8"));
  const max = allow[rel];
  if (n > LIMIT && max === undefined) {
    console.error(`TOO LONG  ${n} lines  ${rel}  (limit ${LIMIT}; split it, do not extend the allowlist)`);
    failed = true;
  } else if (n > LIMIT && n > max) {
    console.error(`GREW      ${n} lines  ${rel}  (allowlisted at ${max}; shrink it, do not grow it)`);
    failed = true;
  } else if (max !== undefined && n <= LIMIT) {
    console.log(`SHRUNK    ${n} lines  ${rel}  (under the limit now; remove it from the allowlist)`);
  }
}

for (const rel of Object.keys(allow)) {
  try {
    statSync(join(ROOT, rel));
  } catch {
    console.error(`STALE     allowlist names a file that no longer exists: ${rel}`);
    failed = true;
  }
}

if (failed) process.exit(1);
console.log(`ok: ${files.length} files within the length gate (limit ${LIMIT})`);
