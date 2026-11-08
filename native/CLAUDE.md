# ByteHive native (Rust + GPUI): rules for this crate

## Keep files small

- One feature per file. A view grows into a folder of parts (`src/inbox/queue.rs`, `stage.rs`, `sheet.rs`, `palette.rs`...), never one file that keeps growing.
- Aim under 400 lines per file. Past 500, split before adding more.
- Split by what a reader looks for (a surface, a panel, a modal, the background work), not by kind of code.
- Shared look (colours, rings, shadows, icons, tooltips) lives in one `style.rs`; words the UI says in one `words.rs`.
- A new surface starts in its own file from its first line.

## Port, don't invent

- Every surface is ported from today's Tauri source (src/styles/*.css, src/*.ts) with its exact values and words. Compare zoomed screenshots side by side before calling it done.
- Owner-approved differences are listed in the port plan, docs/superpowers/plans/2026-11-08-native-gpui-port.md.

## Terminal

- Keys reach the CLI whole through `keys.rs` (raw Win32 messages into win32-input-mode). Never a key table. Only the few keys today's app takes over a terminal stay local.
