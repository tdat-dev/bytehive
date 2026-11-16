// Bundle one gold entry with the office's TypeScript and three from here.
import { build } from "esbuild";
import { fileURLToPath } from "node:url";
import path from "node:path";
const here = path.dirname(fileURLToPath(import.meta.url));
await build({ entryPoints: [process.argv[2]], bundle: true, platform: "node", format: "esm", outfile: process.argv[3], nodePaths: [path.join(here, "node_modules")], logLevel: "error" });
console.log("built", process.argv[3]);
