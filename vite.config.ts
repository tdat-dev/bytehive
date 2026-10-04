/// <reference types="vitest/config" />
import { defineConfig } from "vite";

// Tauri expects a fixed dev-server port (matches devUrl in tauri.conf.json).
// 17630, not Tauri's default 1420: Windows winnat/Hyper-V reserves port blocks
// inside the dynamic port range and EACCES-blocks listening there. On this
// machine that range is 1024-15000 (`netsh int ipv4 show dynamicport tcp`),
// which is why 1430, 1630 and 3630 were each swallowed in turn. A port above
// the range is never reserved.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 17630,
    strictPort: true,
    // Agent worktrees live under .claude/worktrees and Rust builds under
    // src-tauri/target: every file an agent saved there reloaded the dev app,
    // over and over while one worked beside it.
    watch: { ignored: ["**/.claude/**", "**/src-tauri/target/**", "**/.maestro/**", "**/logs/**"] },
  },
  // Keep the test config here and nowhere else: a vitest.config.ts would be
  // loaded *instead of* this file, silently dropping the excludes below.
  test: {
    environment: "happy-dom",
    // mcp/ is its own package with its own vitest; .claude/ holds worktree copies.
    exclude: ["**/node_modules/**", "**/dist/**", "mcp/**", ".claude/**"],
  },
});
