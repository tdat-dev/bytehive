// The light Done gate for the review drawer: what the diff itself admits is
// unfinished (TODO markers, crude stubs), and the test runs the agent's
// conversation shows. Crude on purpose: a warning plus one confirm, never a
// claim that the code was verified. Pure.

import type { ChatItem } from "./chatmodel";
import type { DiffFile } from "./diff";

export type DoneKind = "todo" | "stub";

export interface DoneFinding {
  path: string;
  kind: DoneKind;
  /** The added line, trimmed. */
  text: string;
}

/** Generated files markers are never reviewed line by line. */
const SKIP_PATH = /-lock\.json$|\.lock$|-lock\.yaml$|\.min\.js$/;
const TODO = /\b(TODO|FIXME|XXX|HACK)\b/;
const STUBS = [
  /not implemented/i,
  /unimplemented/i,
  /\btodo!\s*\(/,
  /\.todo\(\)/,
  /raise NotImplementedError/,
  /^\s*pass\s*(#|$)/,
  /^\s*\.\.\.\s*(#|$)/,
];

/** Unfinished markers on lines the diff adds. Removed lines and old context
 *  don't count: they were already there. One finding per line, stub first. */
export function scanDoneDiff(files: DiffFile[]): DoneFinding[] {
  const out: DoneFinding[] = [];
  for (const f of files) {
    if (f.binary || SKIP_PATH.test(f.path)) continue;
    for (const h of f.hunks)
      for (const l of h.lines) {
        if (l.kind !== "add") continue;
        const text = l.text.trim().slice(0, 120);
        if (!text) continue;
        if (STUBS.some((re) => re.test(l.text))) out.push({ path: f.path, kind: "stub", text });
        else if (TODO.test(l.text)) out.push({ path: f.path, kind: "todo", text });
      }
  }
  return out;
}

/** "2 TODOs and 1 stub in 2 files", "1 TODO in src/a.ts", "" when clean. */
export function describeDone(findings: DoneFinding[]): string {
  if (!findings.length) return "";
  const todos = findings.filter((f) => f.kind === "todo").length;
  const stubs = findings.length - todos;
  const parts: string[] = [];
  if (todos) parts.push(`${todos} TODO${todos === 1 ? "" : "s"}`);
  if (stubs) parts.push(`${stubs} stub${stubs === 1 ? "" : "s"}`);
  const files = new Set(findings.map((f) => f.path));
  const where = files.size === 1 ? `in ${[...files][0]}` : `in ${files.size} files`;
  return `${parts.join(" and ")} ${where}`;
}

export interface TestRun {
  cmd: string;
  ok: boolean;
}

const SHELL_TOOL = /bash|shell|powershell|exec|command/i;
const TEST_CMD = /\b(npm (run )?(test|t)|npx (vitest|jest)|yarn (test|vitest)|pnpm (test|vitest)|pytest|cargo test|go test|dotnet test|ctest|jest|vitest|mocha|rspec)\b/;

/** Test commands the conversation ran to completion, oldest first. */
export function testRuns(items: ChatItem[]): TestRun[] {
  const out: TestRun[] = [];
  for (const it of items) {
    if (it.kind !== "step" || !it.done || !SHELL_TOOL.test(it.tool)) continue;
    const cmd = (it.full ?? it.target ?? "").trim();
    if (!cmd || !TEST_CMD.test(cmd)) continue;
    out.push({ cmd: cmd.slice(0, 60), ok: !it.error });
  }
  return out;
}

/** The latest run for the review header, or null when none ran. */
export function testSummary(runs: TestRun[]): { ok: boolean; text: string } | null {
  if (!runs.length) return null;
  const last = runs[runs.length - 1];
  return last.ok
    ? { ok: true, text: `Tests ran green (${last.cmd})` }
    : { ok: false, text: `Last test run failed (${last.cmd})` };
}

export type SecretKind = "secret" | "auth" | "dep";

export interface SecretFinding {
  path: string;
  kind: SecretKind;
  /** The added line, or the path for file-level findings. */
  text: string;
}

const SECRET_RES = [
  /AKIA[0-9A-Z]{16}/,
  /-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----/,
  /gh[pousr]_[A-Za-z0-9]{36,}/,
  /\bsk-(live|test|proj)-[A-Za-z0-9_-]{8,}/,
  /xox[baprs]-[A-Za-z0-9-]{10,}/,
  /(mongodb(\+srv)?|postgres(ql)?|mysql|redis):\/\/[^\s'"]+:[^\s'"]+@/i,
  /eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/,
];
const SECRET_ASSIGN = /(api[_-]?key|apikey|secret|passwd|password|pwd|token)\s*[:=]\s*['"]?[\w\-./+]{8,}['"]?/i;
/** Values that name where the secret lives instead of being one. */
const SAFE_VALUE = /process\.env|os\.environ|getenv\(|getsecret|readsecret/i;
const AUTH_PATH = /\b(auth|login|session|password|passwd|permission|acl)\b/i;
const AUTH_LINE = /verify\s*(password|token|signature)|authenticate|authorize|checkPermission|hasRole|isAdmin/i;
const DEP_PATH = /package\.json$|package-lock\.json$|yarn\.lock$|pnpm-lock\.yaml$|requirements[^/]*\.txt$|pyproject\.toml$|Cargo\.toml$|Cargo\.lock$|go\.mod$|go\.sum$|\.csproj$|Gemfile$|composer\.json$/;

/** Hardcoded secrets on added lines, auth code, and dependency manifests:
 *  flag for review, never auto-judge. One finding per line for secrets, one
 *  per file for auth paths and manifests. */
export function scanSecrets(files: DiffFile[]): SecretFinding[] {
  const out: SecretFinding[] = [];
  for (const f of files) {
    if (f.binary) continue;
    if (DEP_PATH.test(f.path)) out.push({ path: f.path, kind: "dep", text: f.path });
    const authFile = AUTH_PATH.test(f.path);
    if (authFile) out.push({ path: f.path, kind: "auth", text: f.path });
    for (const h of f.hunks)
      for (const l of h.lines) {
        if (l.kind !== "add") continue;
        if (SECRET_RES.some((re) => re.test(l.text)) || (!SAFE_VALUE.test(l.text) && SECRET_ASSIGN.test(l.text))) {
          out.push({ path: f.path, kind: "secret", text: l.text.trim().slice(0, 120) });
          continue;
        }
        if (!authFile && AUTH_LINE.test(l.text)) out.push({ path: f.path, kind: "auth", text: l.text.trim().slice(0, 120) });
      }
  }
  return out;
}

/** "1 secret in .env and auth in 2 files", "" when clean. File names stay:
 *  a flag without a where is a hunt. */
export function describeSecrets(findings: SecretFinding[]): string {
  if (!findings.length) return "";
  const parts: string[] = [];
  const secrets = findings.filter((f) => f.kind === "secret");
  const auth = new Set(findings.filter((f) => f.kind === "auth").map((f) => f.path));
  const deps = new Set(findings.filter((f) => f.kind === "dep").map((f) => f.path));
  if (secrets.length) {
    const files = new Set(secrets.map((f) => f.path));
    const where = files.size === 1 ? ` in ${[...files][0]}` : ` in ${files.size} files`;
    parts.push(`${secrets.length} secret${secrets.length === 1 ? "" : "s"}${where}`);
  }
  if (auth.size) parts.push(auth.size === 1 ? `auth in ${[...auth][0]}` : `auth in ${auth.size} files`);
  if (deps.size) parts.push(deps.size === 1 ? `deps in ${[...deps][0]}` : `deps in ${deps.size} files`);
  return parts.join(" and ");
}
