// Bundle one gold entry with the office's TypeScript and three from here.
// build.ts and setting.ts keep most of their parts to themselves: they are
// exported on the way in, the files on disk untouched. Vite's import.meta.glob
// (CLI logos) is stubbed.
import { build } from "esbuild";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
const here = path.dirname(fileURLToPath(import.meta.url));
// The parts each file keeps to itself, exported for the goldens.
const exposed = [
  ["/company/office/build.ts", "shellGeometry, deskGeometry, chairGeometry, chairBackGeometry, boardGeometry, wallGrid, hexFloor, deskThing, noteParts, stringLights, liftCar, roofSteps, finished"],
  ["/company/office/setting.ts", "starField, moonGeometry, tree, bench, streetLamp, shrub"],
];
const expose = {
  name: "expose",
  setup(b) {
    b.onLoad({ filter: /(build|setting)\.ts$/ }, async (args) => {
      const norm = args.path.split(path.sep).join("/");
      const more = exposed.find(([end]) => norm.endsWith(end));
      if (!more) return undefined;
      const text = await readFile(args.path, "utf8");
      return { contents: text + "\nexport { " + more[1] + " };\n", loader: "ts" };
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
  plugins: [expose],
  define: { "import.meta.glob": "__viteGlob" },
  banner: { js: "const __viteGlob = () => ({});" },
});
console.log("built", process.argv[3]);
