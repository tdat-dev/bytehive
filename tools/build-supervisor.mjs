// Build the side-by-side Supervisor edition: the same code as Maestro, but
// under its own name, app id, dashboard port and browser hub file, so it
// runs next to the regular app without touching it.
//
// No installer bundler (NSIS/WiX) is needed: --no-bundle emits the exe, which
// is renamed to maestro-supervisor.exe (the name the edition detection reads)
// and shipped with browser-extension/ beside it.
//
//   npm run build:supervisor   → dist-supervisor/maestro-supervisor.exe

import { spawnSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { RESOLVED_REL, writeResolved } from "./supervisor-config.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "dist-supervisor");

// Nested overrides (updater endpoints) ride a generated full config, deleted
// after the build so it never drifts or gets committed.
writeResolved();
let status = 0;
try {
  const build = spawnSync(`npx tauri build --config ${RESOLVED_REL} --no-bundle`, {
    cwd: root,
    stdio: "inherit",
    shell: true,
    env: { ...process.env, VITE_EDITION: "supervisor", VITE_DASH_PORT: "8577" },
  });
  status = build.status ?? 1;
} finally {
  rmSync(join(root, RESOLVED_REL), { force: true });
}
if (status !== 0) process.exit(status);

// Tauri names the exe from mainBinaryName even with --no-bundle; fall back to
// the Cargo binary name in case that ever changes.
const cands = ["maestro-supervisor.exe", "maestro-dev.exe"].map((n) =>
  join(root, "src-tauri/target/release", n),
);
const exe = cands.find((c) => existsSync(c));
if (!exe) {
  console.error("missing " + cands[0]);
  process.exit(1);
}
mkdirSync(out, { recursive: true });
copyFileSync(exe, join(out, "maestro-supervisor.exe"));
// The bundler would place resources beside the exe; do it by hand.
cpSync(join(root, "browser-extension"), join(out, "browser-extension"), { recursive: true });
console.log("Supervisor build at " + out);
