import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { claudeSlug, findConversations, readConversation, sameDir, type Env } from "./stores";

const NOW = Date.parse("2026-10-22T12:00:00Z");
const MIN = 60_000;
let home = "";
const env = (): Env => ({ home, now: NOW });

const write = (p: string, text: string, mtime: number) => {
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, text);
  fs.utimesSync(p, new Date(mtime), new Date(mtime));
};
const line = (o: object) => JSON.stringify({ timestamp: "2026-10-22T11:00:00.000Z", ...o }) + "\n";

beforeAll(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), "bytehive-ctx-"));
  // Claude: one live session, one stopped, one too old, one in another project
  const proj = path.join(home, ".claude", "projects", claudeSlug("D:\\app"));
  const claudeTalk =
    line({ type: "user", origin: { kind: "human" }, message: { content: "make the header sticky" } }) +
    line({ type: "assistant", message: { id: "a1", content: [{ type: "tool_use", id: "t1", name: "TodoWrite", input: { todos: [
      { content: "Sticky header", status: "completed", activeForm: "x" },
      { content: "Tests", status: "pending", activeForm: "x" },
    ] } }] } }) +
    line({ type: "assistant", message: { id: "a2", content: [{ type: "text", text: "Header done, tests next." }] } });
  write(path.join(proj, "live-1111.jsonl"), claudeTalk, NOW - 2 * MIN);
  write(path.join(proj, "dead-2222.jsonl"), claudeTalk, NOW - 30 * MIN);
  write(path.join(proj, "old-3333.jsonl"), claudeTalk, NOW - 40 * 86_400_000);
  write(path.join(home, ".claude", "projects", claudeSlug("E:\\other"), "else-4444.jsonl"), claudeTalk, NOW - MIN);
  write(path.join(home, ".claude", "sessions", `${process.pid}.json`), JSON.stringify({ pid: process.pid, sessionId: "live-1111" }), NOW);
  write(path.join(home, ".claude", "sessions", "999999.json"), JSON.stringify({ pid: 999999, sessionId: "dead-2222" }), NOW);
  // Codex: one rollout here, one elsewhere
  const codexTalk = (cwd: string) =>
    JSON.stringify({ timestamp: "2026-10-22T10:00:00.000Z", type: "session_meta", payload: { id: "cdx-5555", cwd, timestamp: "2026-10-22T10:00:00.000Z" } }) + "\n" +
    JSON.stringify({ timestamp: "2026-10-22T10:00:01.000Z", type: "response_item", payload: { type: "message", role: "user", content: [{ type: "input_text", text: "add the export page" }] } }) + "\n";
  const day = path.join(home, ".codex", "sessions", "2026", "10", "22");
  write(path.join(day, "rollout-2026-10-22T10-00-00-cdx-5555.jsonl"), codexTalk("D:/app/"), NOW - 10 * MIN);
  write(path.join(day, "rollout-2026-10-22T10-00-00-cdx-6666.jsonl"), codexTalk("E:\\other"), NOW - MIN);
  // OpenCode: a top-level session here, a helper session, one elsewhere
  const dbFile = path.join(home, ".local", "share", "opencode", "opencode.db");
  fs.mkdirSync(path.dirname(dbFile), { recursive: true });
  const db = new DatabaseSync(dbFile);
  db.exec(`CREATE TABLE session (id text PRIMARY KEY, parent_id text, directory text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL);
    CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);
    CREATE TABLE part (id text PRIMARY KEY, message_id text NOT NULL, session_id text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);`);
  const at = NOW - 5 * MIN;
  db.prepare("INSERT INTO session VALUES (?, ?, ?, ?, ?)").run("ses_here", null, "d:\\APP", at - 1000, at);
  db.prepare("INSERT INTO session VALUES (?, ?, ?, ?, ?)").run("ses_kid", "ses_here", "D:/app", at, at + 100);
  db.prepare("INSERT INTO session VALUES (?, ?, ?, ?, ?)").run("ses_else", null, "E:/other", at, at);
  db.prepare("INSERT INTO message VALUES (?, ?, ?, ?, ?)").run("msg_1", "ses_here", at - 900, at - 900, JSON.stringify({ role: "user", time: { created: at - 900 } }));
  db.prepare("INSERT INTO part VALUES (?, ?, ?, ?, ?, ?)").run("prt_1", "msg_1", "ses_here", at - 899, at - 899, JSON.stringify({ type: "text", text: "fix the redirect" }));
  db.close();
});

afterAll(() => fs.rmSync(home, { recursive: true, force: true }));

describe("finding conversations", () => {
  it("lists every CLI's conversations in the project, newest first", async () => {
    const { list, notes } = await findConversations(["D:\\app"], env());
    expect(list.map((c) => `${c.cli}:${c.id}`)).toEqual(["claude:live-1111", "opencode:ses_here", "codex:cdx-5555", "claude:dead-2222"]);
    expect(notes).toEqual([]);
  });

  it("knows which Claude conversations are still running", async () => {
    const { list } = await findConversations(["D:\\app"], env());
    const running = Object.fromEntries(list.map((c) => [c.id, c.running]));
    expect(running).toMatchObject({ "live-1111": true, "dead-2222": false, "ses_here": null });
  });

  it("looks in every folder it is given", async () => {
    const { list } = await findConversations(["D:\\app", "E:/other"], { ...env(), limit: 50 });
    expect(list.filter((c) => sameDir(c.folder, "E:/other")).map((c) => c.id).sort()).toEqual(["cdx-5555", "else-4444", "ses_else"]);
  });
});

describe("reading a conversation", () => {
  it("parses a Claude one into its asks, plan and last words", async () => {
    const { list } = await findConversations(["D:\\app"], env());
    const { chat, firstAsks } = await readConversation(list[0], env());
    expect(firstAsks).toEqual(["make the header sticky"]);
    expect(chat.meta.todos?.map((t) => t.state)).toEqual(["completed", "pending"]);
  });

  it("parses Codex and OpenCode ones", async () => {
    const { list } = await findConversations(["D:\\app"], env());
    const codex = await readConversation(list.find((c) => c.cli === "codex")!, env());
    expect(codex.firstAsks).toEqual(["add the export page"]);
    const oc = await readConversation(list.find((c) => c.cli === "opencode")!, env());
    expect(oc.firstAsks).toEqual(["fix the redirect"]);
  });
});
