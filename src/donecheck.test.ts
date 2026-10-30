import { describe, expect, it } from "vitest";
import { describeDone, describeSecrets, scanDoneDiff, scanSecrets, testRuns, testSummary } from "./donecheck";
import { parseDiff } from "./diff";
import type { ChatItem, StepItem } from "./chatmodel";

const diffOf = (path: string, body: string[]) =>
  parseDiff(["diff --git a/" + path + " b/" + path, "--- a/" + path, "+++ b/" + path, ...body].join("\n"));

describe("scanDoneDiff", () => {
  it("flags TODO markers added, not removed or pre-existing ones", () => {
    const files = diffOf("src/a.ts", ["@@ -1,2 +1,3 @@", " keep // TODO: old", "-// TODO: gone", "+// TODO: handle empty", "+// FIXME: retry"]);
    const found = scanDoneDiff(files);
    expect(found).toHaveLength(2);
    expect(found[0]).toMatchObject({ path: "src/a.ts", kind: "todo" });
    expect(found[0].text).toContain("handle empty");
  });

  it("flags crude stubs", () => {
    const files = diffOf("src/b.py", [
      "@@ -0,0 +1,5 @@",
      "+    raise NotImplementedError",
      "+    pass",
      "+    ...",
      "+    throw new Error('not implemented')",
      "+    unimplemented!()",
    ]);
    expect(scanDoneDiff(files).map((f) => f.kind)).toEqual(["stub", "stub", "stub", "stub", "stub"]);
  });

  it("reports one finding per line, stub first", () => {
    const files = diffOf("src/a.ts", ["@@ -1 +1 @@", "+// TODO: not implemented yet"]);
    const found = scanDoneDiff(files);
    expect(found).toHaveLength(1);
    expect(found[0].kind).toBe("stub");
  });

  it("skips binary files, lockfiles and minified bundles", () => {
    const bin = parseDiff("diff --git a/img.png b/img.png\nBinary files differ\n");
    const lock = diffOf("package-lock.json", ["@@ -1 +1 @@", "+// TODO: whatever"]);
    const min = diffOf("app.min.js", ["@@ -1 +1 @@", "+// TODO: whatever"]);
    expect(scanDoneDiff([...bin, ...lock, ...min])).toEqual([]);
  });

  it("an empty diff has no findings", () => {
    expect(scanDoneDiff(parseDiff(""))).toEqual([]);
  });
});

describe("describeDone", () => {
  it("counts by kind with the files", () => {
    const files = diffOf("src/a.ts", ["@@ -1 +1,2 @@", "+// TODO: one", "+// TODO: two"]);
    const more = diffOf("src/b.ts", ["@@ -1 +1 @@", "+    pass"]);
    expect(describeDone(scanDoneDiff([...files, ...more]))).toBe("2 TODOs and 1 stub in 2 files");
  });

  it("names the file when there is one", () => {
    const files = diffOf("src/a.ts", ["@@ -1 +1 @@", "+// TODO: one"]);
    expect(describeDone(scanDoneDiff(files))).toBe("1 TODO in src/a.ts");
  });

  it("says nothing when there is nothing", () => {
    expect(describeDone([])).toBe("");
  });
});

describe("testRuns", () => {
  let n = 0;
  const step = (over: Partial<StepItem> = {}): StepItem => ({
    kind: "step", id: `s${++n}`, tool: "Bash", verb: "Ran", target: "npm test", full: "npm test", code: true, done: true, at: 1, ...over,
  });

  it("keeps test commands with their outcome", () => {
    const items: ChatItem[] = [step(), step({ target: "npm run build", full: "npm run build" }), step({ target: "pytest", full: "pytest -q", error: true, output: "FAILED" })];
    expect(testRuns(items)).toEqual([
      { cmd: "npm test", ok: true },
      { cmd: "pytest -q", ok: false },
    ]);
  });

  it("ignores non-shell steps even when they name tests", () => {
    const items: ChatItem[] = [step({ tool: "Read", verb: "Read", target: "pytest.ini", full: undefined })];
    expect(testRuns(items)).toEqual([]);
  });

  it("ignores steps still running", () => {
    const items: ChatItem[] = [step({ done: false })];
    expect(testRuns(items)).toEqual([]);
  });

  it("summarizes the latest run", () => {
    expect(testSummary([])).toBeNull();
    expect(testSummary([{ cmd: "npm test", ok: true }])).toEqual({ ok: true, text: "Tests ran green (npm test)" });
    expect(testSummary([{ cmd: "npm test", ok: true }, { cmd: "cargo test", ok: false }])).toEqual({ ok: false, text: "Last test run failed (cargo test)" });
  });
});

describe("scanSecrets", () => {
  it("flags hardcoded credentials on added lines", () => {
    const files = diffOf(".env", [
      "@@ -0,0 +1,4 @@",
      "+AWS_KEY=AKIAIOSFODNN7EXAMPLE",
      '+api_key = "hunter2-secret-value"',
      "+token: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
      "+postgres://bob:hunter2x@db:5432/app",
    ]);
    const found = scanSecrets(files);
    expect(found.filter((f) => f.kind === "secret")).toHaveLength(4);
  });

  it("flags private keys and vendor tokens", () => {
    const files = diffOf("key.pem", ["@@ -0,0 +1,2 @@", "+-----BEGIN RSA PRIVATE KEY-----", "+sk-live-abcdefghijklmnop1234"]);
    expect(scanSecrets(files).map((f) => f.kind)).toEqual(["secret", "secret"]);
  });

  it("ignores removed lines and safe patterns", () => {
    const files = diffOf("src/a.ts", [
      "@@ -1,3 +1,4 @@",
      '-api_key = "old-hardcoded-secret"',
      '+const key = process.env.API_KEY;',
      '+apiKey = process.env.API_KEY;',
      '+const t = getSecret("api-key");',
      '+password = ""',
    ]);
    expect(scanSecrets(files).filter((f) => f.kind === "secret")).toEqual([]);
  });

  it("flags auth-related files and checks once per file", () => {
    const files = diffOf("src/auth/login.ts", ["@@ -1 +1,2 @@", "+verifyPassword(u, p)", "+normalize(u)"]);
    const found = scanSecrets(files);
    expect(found).toEqual([{ path: "src/auth/login.ts", kind: "auth", text: "src/auth/login.ts" }]);
  });

  it("flags auth checks in plain files", () => {
    const files = diffOf("src/app.ts", ["@@ -1 +1 @@", "+if (!isAdmin(user)) throw 0;"]);
    expect(scanSecrets(files)).toEqual([{ path: "src/app.ts", kind: "auth", text: "if (!isAdmin(user)) throw 0;" }]);
  });

  it("flags dependency manifests once per file", () => {
    const files = diffOf("package.json", ["@@ -1 +1,2 @@", '+"left-pad": "9.9.9",', "+// bump"]);
    expect(scanSecrets(files)).toEqual([{ path: "package.json", kind: "dep", text: "package.json" }]);
  });

  it("says nothing on ordinary code", () => {
    const files = diffOf("src/a.ts", ["@@ -1 +1,2 @@", "+const secretary = roster[0];", "+token: string;"]);
    expect(scanSecrets(files)).toEqual([]);
  });
});

describe("describeSecrets", () => {
  it("counts secrets with auth and dep files", () => {
    expect(describeSecrets([
      { path: ".env", kind: "secret", text: "x" },
      { path: "src/auth/a.ts", kind: "auth", text: "src/auth/a.ts" },
      { path: "src/auth/b.ts", kind: "auth", text: "src/auth/b.ts" },
      { path: "package.json", kind: "dep", text: "package.json" },
    ])).toBe("1 secret in .env and auth in 2 files and deps in package.json");
  });

  it("says nothing when there is nothing", () => {
    expect(describeSecrets([])).toBe("");
  });
});
