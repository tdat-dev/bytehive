import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { agentContext, detailText, listLine, nameOf, resolveWho } from "./agentcontext";
import { claudeSlug, type Conversation } from "./stores";
import type { FleetAgent } from "../fleet";
import type { Digest } from "../../../src/convdigest";

const NOW = Date.parse("2026-10-22T12:00:00Z");
const MIN = 60_000;
const conv = (over: Partial<Conversation>): Conversation => ({
  cli: "claude", id: "3f2a91c0-aaaa", folder: "D:\\app", updated: NOW - 2 * MIN, started: NOW - 60 * MIN, running: true, path: "D:/x/3f2a91c0-aaaa.jsonl", ...over,
});
const agent = (over: Partial<FleetAgent>): FleetAgent => ({ id: "p1", name: "Ana", status: "idle", workspace: "app", screen: "", ...over });
const digest = (over: Partial<Digest> = {}): Digest => ({ asks: [], todos: [], files: [], lastWords: "", ...over });

describe("whose conversation", () => {
  const claude = conv({});
  const oldCodex = conv({ cli: "codex", id: "cdx-old", updated: NOW - 90 * MIN, running: null, path: "D:/r/old.jsonl" });
  const newCodex = conv({ cli: "codex", id: "cdx-new", updated: NOW - 5 * MIN, running: null, path: "D:/r/new.jsonl" });
  const all = [claude, newCodex, oldCodex];

  it("names the one an agent says it is in", () => {
    expect(nameOf(claude, [agent({ cli: "claude", session: "3f2a91c0-aaaa" })], all)).toBe("Ana");
  });

  it("names the newest one of an agent's CLI and folder since it started", () => {
    const bo = agent({ name: "Bo", cli: "codex", folder: "d:/app/", since: NOW - 30 * MIN });
    expect(nameOf(newCodex, [bo], all)).toBe("Bo");
    expect(nameOf(oldCodex, [bo], all)).toBeNull();
  });

  it("finds an agent by name, id or CLI", () => {
    const fleet = [agent({ cli: "claude", session: "3f2a91c0-aaaa" })];
    expect(resolveWho("ana", all, fleet)).toBe(claude);
    expect(resolveWho("[cdx-o]", all, fleet)).toBe(oldCodex);
    expect(resolveWho("codex", all, fleet)).toBe(newCodex);
    expect(resolveWho("nobody", all, fleet)).toBeNull();
  });
});

describe("what it says", () => {
  const plan = [{ text: "Sticky header", state: "completed" as const }, { text: "Tests", state: "pending" as const }];

  it("lists one line per conversation", () => {
    expect(listLine(conv({}), digest({ asks: ["make the header\nsticky"], todos: plan }), "Ana", "D:/app", NOW))
      .toBe("- Ana [3f2a91c0] Claude Code, running, active 2 min ago. Task: make the header sticky. Plan 1/2 done.");
    expect(listLine(conv({ cli: "opencode", id: "ses_8k2xyz", running: null, folder: "D:/wt" }), digest(), null, "D:/app", NOW))
      .toBe("- [ses_8k2x] OpenCode, active 2 min ago. Folder: D:/wt");
  });

  it("tells how to carry on an unfinished plan", () => {
    const text = detailText(conv({ running: false }), digest({ asks: ["make the header sticky"], todos: plan, lastWords: "Header done." }), "Ana", NOW);
    expect(text).toContain("Ana [3f2a91c0] (Claude Code), stopped, started 1 h ago, last active 2 min ago.");
    expect(text).toContain("- [done] Sticky header\n- [to do] Tests");
    expect(text).toContain("> Header done.");
    expect(text).toContain("To carry this on: work in D:\\app");
  });
});

describe("agent_context", () => {
  it("reads the project's conversations end to end", async () => {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), "maestro-ac-home-"));
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "maestro-ac-proj-"));
    try {
      const file = path.join(home, ".claude", "projects", claudeSlug(dir), "abcd1234-0000.jsonl");
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, JSON.stringify({ timestamp: "2026-10-22T11:00:00.000Z", type: "user", origin: { kind: "human" }, message: { content: "fix the login" } }) + "\n");
      fs.utimesSync(file, new Date(NOW - MIN), new Date(NOW - MIN));
      fs.mkdirSync(path.join(dir, ".maestro"));
      fs.writeFileSync(path.join(dir, ".maestro", "fleet.json"), JSON.stringify({ agents: [{ id: "p1", name: "Ana", status: "stopped", cli: "claude", session: "abcd1234-0000" }] }));
      const env = { home, now: NOW };
      const list = await agentContext(dir, undefined, env);
      expect(list).toContain("- Ana [abcd1234] Claude Code, stopped, active 1 min ago. Task: fix the login.");
      expect(await agentContext(dir, "Ana", env)).toContain("1. fix the login");
      expect(await agentContext(dir, "Zed", env)).toContain('No agent "Zed" here.');
    } finally {
      fs.rmSync(home, { recursive: true, force: true });
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});
