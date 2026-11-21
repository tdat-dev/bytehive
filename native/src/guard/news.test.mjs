import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { MAX_LINES, newsLines, newsText, snapshotOf } from "./news.mjs";

const board = (lists) => ({ lists: Object.entries(lists).map(([title, cards]) => ({ id: title, title, cards })) });
const card = (id, title, extra = {}) => ({ id, title, ...extra });
const memory = (...entries) => ({ version: 1, entries });

test("the first turn says nothing", () => {
  assert.deepEqual(newsLines(null, snapshotOf(board({ "To do": [card("c1", "a")] }), null)), []);
});

test("tells an approved card, a new card, a deleted one and a new fact", () => {
  const before = snapshotOf(board({ Proposed: [card("c1", "Record video"), card("c2", "Old")], "To do": [] }), memory({ id: "m1", text: "x", by: "Ana" }));
  const after = snapshotOf(
    board({ Proposed: [card("c3", "Fresh")], "To do": [card("c1", "Record video")] }),
    memory({ id: "m1", text: "x", by: "Ana" }, { id: "m2", text: "Tests run with cargo test", by: "Cid" }),
  );
  assert.deepEqual(newsLines(before, after, "Bob"), [
    'new card [c3] "Fresh" in Proposed',
    'card [c1] "Record video" moved from Proposed to To do',
    'card [c2] "Old" was deleted',
    "memory [m2] new: Tests run with cargo test",
  ]);
});

test("leaves out what the agent did itself", () => {
  const before = snapshotOf(board({ "To do": [card("c1", "a")], Doing: [] }), memory());
  const after = snapshotOf(board({ "To do": [], Doing: [card("c1", "a", { assignee: "Bob" })] }), memory({ id: "m1", text: "mine", by: "Bob" }));
  assert.deepEqual(newsLines(before, after, "Bob"), []);
});

test("a changed fact is told, a long list is cut", () => {
  const before = snapshotOf(board({}), memory({ id: "m1", text: "old", by: "Ana" }));
  const after = snapshotOf(board({}), memory({ id: "m1", text: "new", by: "Ana" }));
  assert.deepEqual(newsLines(before, after), ["memory [m1] changed: new"]);
  const many = Array.from({ length: MAX_LINES + 3 }, (_, i) => `line ${i}`);
  const text = newsText(many);
  assert.match(text, /^ByteHive news since your last turn/);
  assert.match(text, /- and 3 more: see board_get$/);
  assert.equal(newsText([]), null);
});

test("as the hook: quiet on the first prompt, tells the next one, takes in its own turn at stop", () => {
  const ws = fs.mkdtempSync(path.join(os.tmpdir(), "news-ws-"));
  fs.mkdirSync(path.join(ws, ".bytehive"));
  const write = (b) => fs.writeFileSync(path.join(ws, ".bytehive", "board.json"), JSON.stringify(b));
  const script = fileURLToPath(new URL("./news.mjs", import.meta.url));
  const session = `t${process.pid}${Date.now()}`;
  const run = (kind) => {
    const r = spawnSync(process.execPath, [script, kind], { input: JSON.stringify({ session_id: session, cwd: ws }), encoding: "utf8", env: { ...process.env, BYTEHIVE_WORKSPACE: ws, BYTEHIVE_AGENT: "Bob" } });
    return r.stdout;
  };
  try {
    write(board({ Proposed: [card("c1", "a")], "To do": [] }));
    assert.equal(run("prompt"), "");
    write(board({ Proposed: [card("c1", "a"), card("c2", "mine")], "To do": [] }));
    run("stop");
    write(board({ Proposed: [card("c2", "mine")], "To do": [card("c1", "a")] }));
    const out = JSON.parse(run("prompt"));
    assert.equal(out.hookSpecificOutput.hookEventName, "UserPromptSubmit");
    assert.match(out.hookSpecificOutput.additionalContext, /"a" moved from Proposed to To do/);
    assert.doesNotMatch(out.hookSpecificOutput.additionalContext, /mine/);
  } finally {
    fs.rmSync(ws, { recursive: true, force: true });
    fs.rmSync(path.join(os.tmpdir(), "bytehive-news", `${session}.json`), { force: true });
  }
});
