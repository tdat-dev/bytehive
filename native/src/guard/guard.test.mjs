// node --test native/src/guard/guard.test.mjs (also run by `cargo test`, guard.rs).
// Every guard is tested both ways: it fires when it should and stays quiet when
// it should not, so tightening or loosening it never goes unseen.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { audit, lastTurn, refuse, stopAnswer } from "./guard.mjs";

const SCRIPT = fileURLToPath(new URL("./guard.mjs", import.meta.url));

test("refuses commands that throw work away", () => {
  for (const c of [
    "rm -rf /",
    "rm -rf ~",
    "rm -fr /*",
    "rm --recursive --force C:\\",
    "git push --force origin main",
    "git push -f",
    "cd x && git push origin main --force",
    "git reset --hard",
    "git reset --hard origin/main",
    "git clean -fd",
    "git clean -xdf",
    "git checkout -- .",
    "git restore .",
    "Remove-Item -Recurse -Force C:\\",
    "Remove-Item -Recurse -Force ~",
    "Remove-Item -Force -Recurse $env:USERPROFILE",
  ]) {
    assert.ok(refuse(c), `should refuse: ${c}`);
  }
});

test("lets everyday commands through", () => {
  for (const c of [
    "rm -rf node_modules",
    "rm -rf ./target/debug",
    "rm file.txt",
    "git push origin main",
    "git push --force-with-lease",
    "git reset --soft HEAD~1",
    "git reset HEAD file.rs",
    "git clean -n",
    "git checkout -- src/main.rs",
    "git checkout main",
    "git restore src/a.ts",
    "cargo test",
    "Remove-Item -Recurse -Force .\\target",
    "Remove-Item -Recurse -Force C:\\Users\\me\\proj\\dist",
    "Remove-Item notes.txt",
  ]) {
    assert.equal(refuse(c), null, `should allow: ${c}`);
  }
});

test("known limit: a blocked command quoted inside another is still refused", () => {
  // The guard reads text, not shell syntax. Over-refusing here is the safe side;
  // the agent is told why and can write the file another way.
  assert.ok(refuse("echo 'git reset --hard is dangerous' > notes.md"));
});

test("a project can block more", () => {
  assert.ok(refuse("npm publish", { block: ["\\bnpm\\s+publish\\b"] }));
  assert.equal(refuse("npm test", { block: ["\\bnpm\\s+publish\\b"] }), null);
  assert.equal(refuse("ls", { block: ["(unclosed"] }), null, "a broken pattern is skipped");
});

const user = (text) => JSON.stringify({ type: "user", message: { role: "user", content: text } });
const tool = (name, input) => JSON.stringify({ type: "assistant", message: { content: [{ type: "tool_use", id: "t", name, input }] } });
const result = () => JSON.stringify({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: "t", content: "ok" }] } });

test("a turn that changed code and checked nothing is not done", () => {
  const lines = [user("fix it"), tool("Edit", { file_path: "D:/p/src/a.rs" }), result()];
  const a = stopAnswer({}, lines);
  assert.equal(a?.decision, "block");
  assert.match(a.reason, /a\.rs/);
});

test("a PowerShell check counts, and a PowerShell command is guarded too", () => {
  const lines = [user("fix"), tool("Edit", { file_path: "a.rs" }), tool("PowerShell", { command: "npm test" })];
  assert.equal(stopAnswer({}, lines), null);
});

test("a check after the last change lets the turn end", () => {
  const lines = [user("fix it"), tool("Edit", { file_path: "a.rs" }), result(), tool("Bash", { command: "cargo test" }), result()];
  assert.equal(stopAnswer({}, lines), null);
});

test("a change after the check needs another check", () => {
  const lines = [user("fix"), tool("Edit", { file_path: "a.rs" }), tool("Bash", { command: "cargo test" }), tool("Write", { file_path: "b.rs" })];
  assert.equal(stopAnswer({}, lines)?.decision, "block");
});

test("only the turn since the person last typed counts", () => {
  const lines = [user("one"), tool("Edit", { file_path: "a.rs" }), user("now just explain"), tool("Read", { file_path: "a.rs" })];
  assert.equal(stopAnswer({}, lines), null);
});

test("docs, a second stop and a project that turned it off all go", () => {
  assert.equal(stopAnswer({}, [user("doc"), tool("Write", { file_path: "README.md" })]), null);
  const code = [user("x"), tool("Edit", { file_path: "a.ts" })];
  assert.equal(stopAnswer({ stop_hook_active: true }, code), null);
  assert.equal(stopAnswer({}, code, { verify: false }), null);
  assert.equal(stopAnswer({}, [user("x"), tool("Read", { file_path: "a.ts" })]), null, "nothing changed");
});

test("the project's own check command counts", () => {
  const lines = [user("x"), tool("Edit", { file_path: "a.py" }), tool("Bash", { command: "./ci.sh quick" })];
  assert.equal(stopAnswer({}, lines)?.decision, "block");
  assert.equal(stopAnswer({}, lines, { check: "./ci.sh" }), null);
});

test("audit names each changed file once", () => {
  const turn = lastTurn([user("x"), tool("Edit", { file_path: "a.rs" }), tool("Edit", { file_path: "a.rs" })]);
  assert.deepEqual(audit(turn).changed, ["a.rs"]);
});

test("as a hook process: it answers on stdout, and says nothing when all is well", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "guard-"));
  const transcript = path.join(dir, "t.jsonl");
  fs.writeFileSync(transcript, [user("x"), tool("Edit", { file_path: "a.rs" })].join("\n") + "\n");
  const run = (kind, event) => spawnSync(process.execPath, [SCRIPT, kind], { input: JSON.stringify(event), encoding: "utf8", env: { ...process.env, BYTEHIVE_WORKSPACE: dir } });
  const stop = run("stop", { transcript_path: transcript, cwd: dir });
  assert.equal(JSON.parse(stop.stdout).decision, "block");
  const pre = run("pre", { tool_name: "Bash", tool_input: { command: "git reset --hard" }, cwd: dir });
  assert.equal(JSON.parse(pre.stdout).hookSpecificOutput.permissionDecision, "deny");
  assert.equal(run("pre", { tool_name: "Bash", tool_input: { command: "ls" }, cwd: dir }).stdout, "");
  assert.equal(JSON.parse(run("pre", { tool_name: "PowerShell", tool_input: { command: "git push --force" }, cwd: dir }).stdout).hookSpecificOutput.permissionDecision, "deny");
  fs.mkdirSync(path.join(dir, ".bytehive"));
  fs.writeFileSync(path.join(dir, ".bytehive", "guards.json"), JSON.stringify({ verify: false }));
  assert.equal(run("stop", { transcript_path: transcript, cwd: dir }).stdout, "", "turned off by the project");
});
