// Write the updater's latest.json for a native release:
//   node tools/latest_json.mjs <version> <installer-url> <installer.sig> <notes.md> <out.json>
// Every Windows key points at the NSIS installer, so the Tauri app's updater
// (which reads windows-x86_64-nsis, or windows-x86_64 for an MSI install)
// moves people onto the native app, and the native updater reads it too.
import { readFileSync, writeFileSync } from "node:fs";

const [version, url, sigFile, notesFile, out] = process.argv.slice(2);
if (!out) {
  console.error("usage: latest_json.mjs <version> <url> <sig> <notes> <out>");
  process.exit(1);
}
const signature = readFileSync(sigFile, "utf8").trim();
const notes = readFileSync(notesFile, "utf8").trim();
const entry = { signature, url };
const json = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: { "windows-x86_64": entry, "windows-x86_64-nsis": entry, "windows-x86_64-msi": entry },
};
writeFileSync(out, JSON.stringify(json, null, 2) + "\n");
console.log(`wrote ${out} for ${version}`);
