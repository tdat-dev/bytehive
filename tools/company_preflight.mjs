// tools/company_preflight.mjs
/* Run before any dev app. Says where it would write and whether it is safe.
 *   node tools/company_preflight.mjs              port and lock report
 *   node tools/company_preflight.mjs --hash       hash of the owner's company files
 *   node tools/company_preflight.mjs --hold-lock <root>   a fresh lock held by this process, for 5 minutes
 */
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const real = path.join(os.homedir(), ".bytehive", "company");
const arg = process.argv[2];

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) return e.name === "people" ? [] : walk(p);
    return e.name === "engine.lock" ? [] : [p];
  });
}

if (arg === "--hash") {
  const h = createHash("sha256");
  for (const f of walk(real).sort()) { h.update(f); h.update(fs.readFileSync(f)); }
  console.log(h.digest("hex"));
  process.exit(0);
}

if (arg === "--hold-lock") {
  const root = process.argv[3];
  if (!root || path.resolve(root) === path.resolve(real)) { console.error("refused: a sandbox root only"); process.exit(1); }
  fs.mkdirSync(root, { recursive: true });
  const beat = () => fs.writeFileSync(path.join(root, "engine.lock"), JSON.stringify({ pid: process.pid, at: Date.now() }));
  beat();
  const t = setInterval(beat, 1500);
  setTimeout(() => { clearInterval(t); process.exit(0); }, 300_000);
} else {
  const lock = (() => { try { return JSON.parse(fs.readFileSync(path.join(real, "engine.lock"), "utf8")); } catch { return null; } })();
  const age = lock ? Math.round((Date.now() - lock.at) / 1000) : null;
  console.log(`real company lock: ${lock ? `pid ${lock.pid}, ${age}s old` : "none"}`);
  const netstat = execFileSync("netstat", ["-ano", "-p", "TCP"], { encoding: "utf8" });
  const held = netstat.split("\n").find((l) => /:17630\s/.test(l) && /LISTENING/.test(l));
  console.log(held ? `port 17630 is taken: ${held.trim()}. Wait for it, never kill it.` : "port 17630 is free");
  process.exit(held ? 1 : 0);
}
