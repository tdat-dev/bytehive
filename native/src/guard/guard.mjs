// ByteHive's guards for Claude Code, given to every agent ByteHive starts through
// --settings (docs/superpowers/specs/2026-11-13-harness-research.md, section 6).
// Rules a model may skip when they are only words, kept here as a script:
//
//   node guard.mjs pre    PreToolUse on Bash and PowerShell: refuse commands that destroy work.
//   node guard.mjs stop   Stop: a turn that changed code and ran no check after the
//                         last change is not done yet. Asked once; a second stop goes.
//   node guard.mjs post   PostToolUse (Codex only): notes the turn's edits and checks.
//
// Claude Code and Codex speak the same hook shapes; Codex is wired in with -c.
//
// The hook event comes on stdin as JSON; the answer goes to stdout. A project
// tunes it in <project>/.bytehive/guards.json:
//   { "verify": true, "check": "cargo test", "block": ["regex", ...] }

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** Commands that throw work away, each with the words the agent is told. */
export const BLOCK = [
  [/\brm\s+(?:-[a-z]*r[a-z]*f[a-z]*|-[a-z]*f[a-z]*r[a-z]*|--recursive\s+--force|--force\s+--recursive)\s+(?:\/|~|\$HOME|[a-z]:[\\/]?)(?:\s|\*|$)/i, "it deletes a whole drive or home folder"],
  [/\b(?:Remove-Item|rm|del|rd|rmdir)\b[^\n;|]*-Recurse[^\n;|]*\s(?:[a-z]:\\?|~|\$HOME|\$env:USERPROFILE)(?:[\s*]|$)/i, "it deletes a whole drive or home folder"],
  [/\bgit\s+push\b[^\n;&|]*\s(?:--force(?!-with-lease)\b|-f\b)/, "a force push rewrites what others have"],
  [/\bgit\s+reset\s+(?:[^\n;&|]*\s)?--hard\b/, "it throws away uncommitted work"],
  [/\bgit\s+clean\s+-[a-z]*(?:f[a-z]*d|d[a-z]*f)/i, "it deletes untracked files for good"],
  [/\bgit\s+checkout\s+(?:--\s+)?\.(?:\s|$)|\bgit\s+restore\s+(?:--[\w-]+\s+)*\.(?:\s|$)/, "it throws away every uncommitted change"],
];

const EDITS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit"]);
/** The tools that run a command: Bash, and PowerShell on Windows. */
export const SHELLS = new Set(["Bash", "PowerShell"]);
/** A command that checks the work: tests, a build, a type check, a linter. */
// A test runner, a build or a linter actually run. A bare word like "check" is
// not enough: `git diff --check` would pass for one (seen with Codex).
const CHECK = /\b(?:cargo\s+(?:test|build|check|clippy|nextest)|(?:npm|pnpm|yarn|bun)\s+(?:test|t|run\s+[\w:.-]+|build)|npx\s+(?:vitest|jest|tsc|eslint|playwright|mocha)|node\s+--test|go\s+(?:test|build|vet)|deno\s+test|python\d?\s+-m\s+(?:pytest|unittest)|pytest|vitest|jest|mocha|tsc|eslint|ruff|mypy|make|gradle|mvn|dotnet\s+(?:test|build)|ctest|phpunit|rspec)\b|\b[\w./\\-]*test[\w-]*\.(?:sh|ps1|cmd|bat|mjs|js|py)\b/i;
/** Files whose change needs no check. */
const QUIET = /\.(?:md|mdx|txt|rst|adoc)$/i;

export function readConfig(dirs) {
  for (const d of dirs) {
    if (!d) continue;
    try {
      return JSON.parse(fs.readFileSync(path.join(d, ".bytehive", "guards.json"), "utf8"));
    } catch {}
  }
  return {};
}

/** Why `command` is refused, or null. */
export function refuse(command, config = {}) {
  for (const [re, why] of BLOCK) if (re.test(command)) return why;
  for (const extra of config.block ?? []) {
    try {
      if (new RegExp(extra).test(command)) return `the project's guards.json blocks /${extra}/`;
    } catch {}
  }
  return null;
}

/** The transcript's lines from the last thing the person typed on. */
export function lastTurn(lines) {
  const entries = [];
  for (const l of lines) {
    try {
      entries.push(JSON.parse(l));
    } catch {}
  }
  let start = 0;
  entries.forEach((e, i) => {
    const c = e?.message?.content;
    const typed = e?.type === "user" && !e.isMeta && (typeof c === "string" || (Array.isArray(c) && c.some((p) => p?.type === "text")));
    if (typed) start = i;
  });
  return entries.slice(start);
}

/** What the turn changed and whether a check ran after the last change. */
export function audit(turn, config = {}) {
  const changed = [];
  let lastEdit = -1;
  let lastCheck = -1;
  turn.forEach((e, i) => {
    if (e?.type !== "assistant" || !Array.isArray(e.message?.content)) return;
    for (const p of e.message.content) {
      if (p?.type !== "tool_use") continue;
      const file = p.input?.file_path ?? p.input?.notebook_path ?? "";
      if (EDITS.has(p.name) && !QUIET.test(file)) {
        lastEdit = i;
        changed.push(path.basename(String(file)));
      }
      const cmd = String(p.input?.command ?? "");
      if (SHELLS.has(p.name) && (CHECK.test(cmd) || (config.check && cmd.includes(config.check)))) lastCheck = i;
    }
  });
  return { changed: [...new Set(changed)], checked: lastCheck > lastEdit };
}

/** The Stop answer: null lets the agent stop. */
export function stopAnswer(event, lines, config = {}) {
  if (event.stop_hook_active || config.verify === false) return null;
  const { changed, checked } = audit(lastTurn(lines), config);
  return verdict(changed, checked, config);
}

/** Block the stop when code changed with no check after it. */
function verdict(changed, checked, config) {
  if (!changed.length || checked) return null;
  const how = config.check ? `\`${config.check}\`` : "the project's tests or build";
  const files = changed.slice(0, 5).join(", ") + (changed.length > 5 ? ` and ${changed.length - 5} more` : "");
  return {
    decision: "block",
    reason: `ByteHive: you changed ${files} and ran no check after the last change. Run ${how} and report what it printed before you finish. If this change needs no check, say why in one line.`,
  };
}

// Codex: its transcript is not a stable format (its docs say so), so the turn is
// followed as it happens instead. PostToolUse notes each edit and each check in a
// small file kept per session and turn; Stop reads it.

/** The files an apply_patch touches. */
export function patchFiles(patch) {
  return [...String(patch).matchAll(/^\*\*\* (?:Add|Update|Delete) File: (.+)$/gm)].map((m) => m[1].trim());
}

/** Files whose state differs between two worktree snapshots. */
export function moved(before, after) {
  const keys = new Set([...Object.keys(before ?? {}), ...Object.keys(after ?? {})]);
  return [...keys].filter((k) => (before ?? {})[k] !== (after ?? {})[k]);
}

/**
 * The turn's record after one more tool call. `before` and `after` are worktree
 * snapshots around the call (null outside git): Codex edits files with shell
 * commands as often as with apply_patch, so a change is whatever moved.
 */
export function note(state, event, config = {}, before = null, after = null) {
  const s = { changed: [], edit: -1, check: -1, n: 0, ...state };
  const cmd = String(event.tool_input?.command ?? "");
  const files = (before && after ? moved(before, after) : event.tool_name === "apply_patch" ? patchFiles(cmd) : []).filter((f) => !QUIET.test(f));
  if (files.length) {
    s.edit = s.n;
    s.changed = [...new Set([...s.changed, ...files.map((f) => path.basename(f))])];
  }
  // A command that edits and then checks (`... && npm test`) counts as checked.
  if (SHELLS.has(event.tool_name) && (CHECK.test(cmd) || (config.check && cmd.includes(config.check)))) {
    s.check = s.n + 0.5;
  }
  s.n += 1;
  return s;
}

/** The worktree's uncommitted files with their size and time, or null outside git. */
export function snapshot(cwd) {
  const r = spawnSync("git", ["-C", cwd, "status", "--porcelain=v1", "-uall", "-z"], { encoding: "utf8", windowsHide: true, timeout: 5000 });
  if (r.status !== 0) return null;
  const out = {};
  for (const entry of r.stdout.split("\0")) {
    if (entry.length < 4) continue;
    const file = entry.slice(3);
    let stamp = "gone";
    try {
      const st = fs.statSync(path.join(cwd, file));
      stamp = `${st.size}:${Math.round(st.mtimeMs)}`;
    } catch {}
    out[file] = `${entry.slice(0, 2)}|${stamp}`;
  }
  return out;
}

/** The Stop answer for a turn followed by `note`. */
export function stopFromNotes(event, state, config = {}) {
  if (event.stop_hook_active || config.verify === false || !state) return null;
  return verdict(state.changed ?? [], (state.check ?? -1) > (state.edit ?? -1), config);
}

const notesFile = (event) => {
  const safe = (s) => String(s ?? "").replace(/[^\w-]/g, "").slice(0, 80);
  return path.join(os.tmpdir(), "bytehive-guard", `${safe(event.session_id)}-${safe(event.turn_id)}.json`);
};

function readNotes(event, file = notesFile(event)) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

/** Where a tool call's before-snapshot waits for its PostToolUse. */
const beforeFile = (event) => notesFile(event).replace(/\.json$/, `-${String(event.tool_use_id ?? "").replace(/[^\w-]/g, "").slice(0, 80)}.before.json`);

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value));
}

function tail(file, bytes = 4 * 1024 * 1024) {
  try {
    const fd = fs.openSync(file, "r");
    const size = fs.fstatSync(fd).size;
    const start = Math.max(0, size - bytes);
    const buf = Buffer.alloc(size - start);
    fs.readSync(fd, buf, 0, buf.length, start);
    fs.closeSync(fd);
    const lines = buf.toString("utf8").split("\n");
    return start > 0 ? lines.slice(1) : lines;
  } catch {
    return [];
  }
}

async function main(kind) {
  let raw = "";
  for await (const chunk of process.stdin) raw += chunk;
  let event = {};
  try {
    event = JSON.parse(raw);
  } catch {}
  const config = readConfig([process.env.BYTEHIVE_WORKSPACE, event.cwd, process.cwd()]);
  if (kind === "pre" && SHELLS.has(event.tool_name)) {
    const why = refuse(String(event.tool_input?.command ?? ""), config);
    if (why) {
      const reason = `ByteHive guard: refused because ${why}. If it is really needed, ask the owner to run it.`;
      process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: reason } }));
    }
  }
  const cwd = String(event.cwd ?? process.cwd());
  // Codex (it sends a turn id): a snapshot before each tool call, compared after it.
  if (kind === "pre" && event.turn_id) {
    const snap = snapshot(cwd);
    if (snap) writeJson(beforeFile(event), snap);
  }
  if (kind === "post" && event.turn_id) {
    const bf = beforeFile(event);
    const before = readNotes(event, bf);
    fs.rmSync(bf, { force: true });
    const after = before ? snapshot(cwd) : null;
    writeJson(notesFile(event), note(readNotes(event), event, config, before, after));
  }
  if (kind === "stop") {
    // Codex sends a turn id and is followed by notes; Claude's transcript is read.
    const answer = event.turn_id ? stopFromNotes(event, readNotes(event), config) : stopAnswer(event, tail(String(event.transcript_path ?? "")), config);
    if (answer) process.stdout.write(JSON.stringify(answer));
  }
}

// Run as the hook, not when a test imports it. A guard that fails lets the agent go on.
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv[2]).catch(() => {});
}
