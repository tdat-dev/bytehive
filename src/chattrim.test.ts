import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./chatsource", () => ({ sourceOf: vi.fn() }));

import { sourceOf } from "./chatsource";
import { createChat } from "./chatmodel";
import { createCodexChat } from "./codexmodel";
import { createMuseChat } from "./musemodel";
import { createOpencodeChat } from "./opencodemodel";
import { FEED_KEEP, forgetFeed, itemsFor, refreshGlance } from "./gridcards";
import type { Pane } from "./panetypes";

// --- claude ---

const T = "2026-09-24T10:00:00.000Z";
const cline = (o: object) => JSON.stringify({ timestamp: T, ...o });
const human = (content: unknown) => cline({ type: "user", origin: { kind: "human" }, message: { role: "user", content } });

// --- codex ---

const at = (sec: number) => new Date(Date.UTC(2026, 8, 25, 10, 0, sec)).toISOString();
const xuser = (sec: number, text: string) =>
  JSON.stringify({ timestamp: at(sec), type: "response_item", payload: { type: "message", role: "user", content: [{ type: "input_text", text }] } });

// --- muse ---

const rec = (id: string, ms: number, event: Record<string, unknown>): string =>
  JSON.stringify({ schema_version: 1, id, recorded_at: ms * 1000, record_type: "event", payload: { kind: "run", event } });

// --- opencode ---

const omsg = (id: string, created: number, data: object) => JSON.stringify({ kind: "message", id, messageId: id, created, updated: created, data });
const opart = (id: string, messageId: string, created: number, data: object) => JSON.stringify({ kind: "part", id, messageId, created, updated: created, data });

describe("chat trim keeps the tail", () => {
  it("claude drops the oldest items past keep", () => {
    const chat = createChat();
    chat.feed([human("q1"), human("q2"), human("q3"), human("q4"), human("q5"), ""].join("\n"));
    expect(chat.items).toHaveLength(5);
    chat.trim(2);
    expect(chat.items.map((i) => (i.kind === "user" ? i.text : "?"))).toEqual(["q4", "q5"]);
  });

  it("claude forgets dropped steps: a late result adds no zombie", () => {
    const chat = createChat();
    chat.feed([
      cline({ type: "assistant", message: { role: "assistant", content: [{ type: "tool_use", id: "t1", name: "Bash", input: { command: "old", description: "old" } }] } }),
      human("q2"),
      human("q3"),
      "",
    ].join("\n"));
    expect(chat.items).toHaveLength(3);
    chat.trim(2);
    chat.feed(cline({ type: "user", message: { role: "user", content: [{ type: "tool_result", tool_use_id: "t1", content: "late" }] } }));
    expect(chat.items).toHaveLength(2);
  });

  it("codex drops the oldest items past keep", () => {
    const c = createCodexChat();
    c.feed([xuser(1, "q1"), xuser(2, "q2"), xuser(3, "q3"), xuser(4, "q4"), xuser(5, "q5")].join("\n"));
    expect(c.items).toHaveLength(5);
    c.trim(2);
    expect(c.items.map((i) => (i.kind === "user" ? i.text : "?"))).toEqual(["q4", "q5"]);
  });

  it("muse drops the oldest items past keep", () => {
    const chat = createMuseChat();
    chat.feed([
      rec("e1", 1000, { kind: "started", prompt: "q1" }),
      rec("e2", 2000, { kind: "started", prompt: "q2" }),
      rec("e3", 3000, { kind: "started", prompt: "q3" }),
      rec("e4", 4000, { kind: "started", prompt: "q4" }),
      rec("e5", 5000, { kind: "started", prompt: "q5" }),
    ].join("\n"));
    expect(chat.items).toHaveLength(5);
    chat.trim(2);
    expect(chat.items.map((i) => (i.kind === "user" ? i.text : "?"))).toEqual(["q4", "q5"]);
  });

  it("opencode drops the oldest items past keep", () => {
    const chat = createOpencodeChat();
    const lines: string[] = [];
    for (let k = 1; k <= 5; k++) {
      lines.push(omsg(`m${k}`, k * 1000, { role: "user", time: { created: k * 1000 } }));
      lines.push(opart(`p${k}`, `m${k}`, k * 1000 + 1, { type: "text", text: `q${k}` }));
    }
    chat.feed(lines.join("\n"));
    expect(chat.items).toHaveLength(5);
    chat.trim(2);
    expect(chat.items.map((i) => (i.kind === "user" ? i.text : "?"))).toEqual(["q4", "q5"]);
  });
});

describe("card feed stays bounded", () => {
  beforeEach(() => { forgetFeed(); vi.mocked(sourceOf).mockReset(); });

  it("trims a long conversation to FEED_KEEP items", async () => {
    const bulk = Array.from({ length: FEED_KEEP + 50 }, (_, k) => human(`q${k}`)).join("\n") + "\n";
    vi.mocked(sourceOf).mockReturnValue({
      createChat, tails: false,
      read: async (_pane: Pane, _dir: string, offset: number) => ({ path: "p", text: offset === 0 ? bulk : "", next: offset === 0 ? bulk.length : offset }),
    } as never);
    const pane = { id: "a", spawnedAt: 1000, spec: { badge: "claude", ranIn: "D:\\p" } } as Pane;
    await refreshGlance(pane);
    expect(itemsFor("a").length).toBeLessThanOrEqual(FEED_KEEP);
    expect(itemsFor("a").length).toBeGreaterThan(0);
  });
});
