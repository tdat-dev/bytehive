# AGENTS.md — rules for working in Maestro

## File length (hard gate)

No source file may exceed **500 lines** (`src/**/*.ts`, `src-tauri/src/**/*.rs`,
`crates/*/src/**/*.rs`). A file heading past that is doing more than one job:
split it by responsibility (one module, one reason to change) instead of
growing it.

- Run `npm run check:length` (also a CI job). It fails on any file over the
  limit and on any allowlisted file that grew.
- `tools/file_length_allowlist.json` grandfathers the files that were already
  over the limit, with their size at the time. The allowlist only ever
  shrinks: never add to it for new code, and when you touch a listed file,
  split it (or part of it) and lower or drop its entry in the same change.
- Tests count too. Test helpers shared by several files go in a
  `*.shared.test.ts` / helper module, not in one ever-growing test file.
- Generated code (`target/`, `node_modules/`, `dist/`) is excluded.

When a file approaches the limit, split first, then add the feature.
