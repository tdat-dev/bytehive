/* The one file the app carries: its members run it with plain node, far from any
 * node_modules. Written only when it changed: tauri-build watches its resources,
 * so a new timestamp would rebuild the whole app on every `tauri dev`. */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const here = path.dirname(fileURLToPath(import.meta.url));
// The native app's installer passes where it stages the file: `npm run bundle -- <file>`.
const out = process.argv[2] ? path.resolve(process.argv[2]) : path.join(here, "..", "src-tauri", "resources", "mcp", "bytehive-mcp.mjs");

const { outputFiles: [file] } = await build({
  entryPoints: [path.join(here, "src", "index.ts")],
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node22",
  outfile: out,
  write: false,
  // ws (the standalone browser hub) is CommonJS and requires node built-ins,
  // which an ESM bundle can only do through a real require.
  banner: { js: 'import { createRequire as __bhRequire } from "node:module"; const require = __bhRequire(import.meta.url);' },
});

const before = fs.existsSync(out) ? fs.readFileSync(out) : null;
if (before?.equals(file.contents)) console.log(`bytehive-mcp.mjs unchanged, ${file.contents.length} bytes`);
else {
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, file.contents);
  console.log(`bytehive-mcp.mjs written, ${file.contents.length} bytes`);
}
