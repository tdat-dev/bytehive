// Resolved Tauri config for the Supervisor edition: deep-merge the base
// tauri.conf.json with the small tauri.supervisor.conf.json overlay, so the
// overlay can override nested keys (updater endpoints) without depending on
// how `tauri build --config` merges, and without a full config copy drifting
// from the base (version included).
//
//   node tools/supervisor-config.mjs --write [--out <path>]
//
// Objects merge recursively; arrays and scalars replace. Default output is
// src-tauri/tauri.supervisor.resolved.json (deleted after the build).

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export function merge(base, over) {
  if (Array.isArray(over)) return over;
  if (over && typeof over === "object" && base && typeof base === "object" && !Array.isArray(base)) {
    const out = { ...base };
    for (const [k, v] of Object.entries(over)) out[k] = k in out ? merge(out[k], v) : v;
    return out;
  }
  return over;
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
export const RESOLVED_REL = "src-tauri/tauri.supervisor.resolved.json";

export function resolvedPath(out) {
  return out ?? join(root, RESOLVED_REL);
}

export function writeResolved(out) {
  const base = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"));
  const over = JSON.parse(readFileSync(join(root, "src-tauri/tauri.supervisor.conf.json"), "utf8"));
  const dest = resolvedPath(out);
  writeFileSync(dest, JSON.stringify(merge(base, over), null, 2) + "\n");
  return dest;
}

const args = process.argv.slice(2);
if (args.includes("--write")) {
  const i = args.indexOf("--out");
  const dest = writeResolved(i >= 0 ? args[i + 1] : undefined);
  console.log("wrote " + dest);
}
