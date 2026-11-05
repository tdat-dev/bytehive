#!/usr/bin/env node
/* bytehive-mcp entry: stdio MCP server for the ByteHive kanban board.
 * Workspace resolution, most-specific first:
 *   1. argv[2]            — explicit override (`bytehive-mcp <dir>`)
 *   2. BYTEHIVE_WORKSPACE  — the project folder, also for an agent running in
 *      a worktree; set on every ByteHive-spawned terminal. Pins the board to it
 *      even if the agent's cwd drifts (a `cd`, a subdir, a git worktree), so an
 *      agent can never write its cards into a DIFFERENT project's board.
 *   3. process.cwd()      — fallback for agents launched outside ByteHive. */

import fs from "node:fs";
import path from "node:path";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { createServer } from "./server.js";

const dir = path.resolve(
  process.argv[2] ?? (process.env.BYTEHIVE_WORKSPACE?.trim() || process.env.MAESTRO_WORKSPACE?.trim() || process.cwd()),
);
if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
  console.error(`bytehive-mcp: workspace directory not found: ${dir}`);
  process.exit(1);
}
// A project from before the rename (Maestro) keeps its board, memory and
// fleet files in .maestro: it becomes .bytehive, once.
const old = path.join(dir, ".maestro");
const now = path.join(dir, ".bytehive");
if (fs.existsSync(old) && !fs.existsSync(now)) {
  try { fs.renameSync(old, now); } catch { /* in use: try again next start */ }
}

const server = createServer(dir);
await server.connect(new StdioServerTransport());
