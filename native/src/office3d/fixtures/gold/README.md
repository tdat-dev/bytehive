# Goldens from the office's own TypeScript

The 3D office in `native/src/office3d` is a port of the Tauri office
(`src/company/office` on feat/office-3d). Its goldens are made by running that
TypeScript itself:

1. In a scratch folder: `npm install three@0.170.0 esbuild@0.24.0`, and copy
   `build.mjs` there.
2. Copy a `*-gold.ts` there and replace `OFFICE` in its imports with the path
   to the office's `src/company/office`.
3. `node build.mjs light-gold.ts light-gold.mjs`, then
   `node light-gold.mjs <this folder>/../light.json`.

`prims.mjs` beside this folder makes `prims.json` from three alone.
