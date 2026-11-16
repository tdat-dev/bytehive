// ByteHive's guards for Claude Code, given to every agent ByteHive starts through
// --settings (docs/superpowers/specs/2026-11-13-harness-research.md, section 6).
// Rules a model may skip when they are only words, kept here as a script:
//
//   node guard.mjs pre    PreToolUse on Bash: refuse commands that destroy work.
//   node guard.mjs stop   Stop: a turn that changed code and ran no check after the
//                         last change is not done yet. Asked once; a second stop goes.
//
// The hook event comes on stdin as JSON; the answer goes to stdout. A project
// tunes it in <project>/.bytehive/guards.json:
//   { "verify": true, "check": "cargo test", "block": ["regex", ...] }

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** Commands that throw work away, each with the words the agent is told. */
export const BLOCK = [
  [/\brm\s+(?:-[a-z]*r[a-z]*f[a-z]*|-[a-z]*f[a-z]*r[a-z]*|--recursive\s+--force|--force\s+--recursive)\s+(?:\/|~|\$HOME|[a-z]:[\\/]?)(?:\s|\*|$)/i, "it deletes a whole drive or home folder"],
  [/\bgit\s+push\b[^\n;&|]*\s(?:--force(?!-with-lease)\b|-f\b)/, "a force push rewrites what others have"],
  [/\bgit\s+reset\s+(?:[^\n;&|]*\s)?--hard\b/, "it throws away uncommitted work"],
  [/\bgit\s+clean\s+-[a-z]*(?:f[a-z]*d|d[a-z]*f)/i, "it deletes untracked files for good"],
  [/\bgit\s+checkout\s+(?:--\s+)?\.(?:\s|$)|\bgit\s+restore\s+(?:--[\w-]+\s+)*\.(?:\s|$)/, "it throws away every uncommitted change"],
];

const EDITS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit"]);
/** A command that checks the work: tests, a build, a type check, a linter. */
const CHECK = /\b(?:test|tests|build|check|lint|clippy|tsc|vitest|jest|pytest|mocha|playwright|cargo|npm|pnpm|yarn|bun|go|make|gradle|mvn|dotnet|ruff|mypy|eslint)\b/i;
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
      if (p.name === "Bash" && (CHECK.test(cmd) || (config.check && cmd.includes(config.check)))) lastCheck = i;
    }
  });
  return { changed: [...new Set(changed)], checked: lastCheck > lastEdit };
}

/** The Stop answer: null lets the agent stop. */
export function stopAnswer(event, lines, config = {}) {
  if (event.stop_hook_active || config.verify === false) return null;
  const { changed, checked } = audit(lastTurn(lines), config);
  if (!changed.length || checked) return null;
  const how = config.check ? `\`${config.check}\`` : "the project's tests or build";
  const files = changed.slice(0, 5).join(", ") + (changed.length > 5 ? ` and ${changed.length - 5} more` : "");
  return {
    decision: "block",
    reason: `ByteHive: you changed ${files} and ran no check after the last change. Run ${how} and report what it printed before you finish. If this change needs no check, say why in one line.`,
  };
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
  if (kind === "pre" && event.tool_name === "Bash") {
    const why = refuse(String(event.tool_input?.command ?? ""), config);
    if (why) {
      const reason = `ByteHive guard: refused because ${why}. If it is really needed, ask the owner to run it.`;
      process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: reason } }));
    }
  }
  if (kind === "stop") {
    const answer = stopAnswer(event, tail(String(event.transcript_path ?? "")), config);
    if (answer) process.stdout.write(JSON.stringify(answer));
  }
}

// Run as the hook, not when a test imports it. A guard that fails lets the agent go on.
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv[2]).catch(() => {});
}
