// Bundle one gold entry with the office's TypeScript and three from here.
// build.ts keeps most of its parts to itself: they are exported on the way in,
// the file on disk untouched. Vite's import.meta.glob (CLI logos) is stubbed.
import { build } from "esbuild";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
const here = path.dirname(fileURLToPath(import.meta.url));
const exposeBuild = {
  name: "expose-build",
  setup(b) {
    b.onLoad({ filter: /build\.ts$/ }, async (args) => {
      const norm = args.path.split(path.sep).join("/");
      if (!norm.endsWith("/company/office/build.ts")) return undefined;
      const text = await readFile(args.path, "utf8");
      const more = "\nexport { shellGeometry, deskGeometry, chairGeometry, chairBackGeometry, boardGeometry, wallGrid, hexFloor, deskThing, noteParts, stringLights, liftCar };\n";
      return { contents: text + more, loader: "ts" };
    });
  },
};
await build({
  entryPoints: [process.argv[2]],
  bundle: true,
  platform: "node",
  format: "esm",
  outfile: process.argv[3],
  nodePaths: [path.join(here, "node_modules")],
  logLevel: "error",
  plugins: [exposeBuild],
  define: { "import.meta.glob": "__viteGlob" },
  banner: { js: "const __viteGlob = () => ({});" },
});
console.log("built", process.argv[3]);
