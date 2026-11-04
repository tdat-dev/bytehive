import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// A fake file system for the jsonl the MCP server appends to, with the same
// contract as the Rust fs_read_lines_from / fs_truncate_if_size commands.
const files = new Map<string, string>();
const SLICE = 1024 * 1024;
async function fakeRead(root: string, path: string, offset: number | null) {
  const body = files.get(`${root}|${path}`);
  if (body === undefined) throw new Error("no such path");
  const size = body.length;
  if (offset === null) {
    const end = body.lastIndexOf("\n") + 1;
    return { text: "", next: end, from: end, size };
  }
  const start = offset > size ? 0 : offset;
  const buf = body.slice(start, start + SLICE);
  const cut = buf.lastIndexOf("\n") + 1;
  return { text: buf.slice(0, cut), next: start + cut, from: start, size };
}
const readLines = vi.fn(fakeRead);
const truncate = vi.fn(async (root: string, path: string, size: number) => {
  const k = `${root}|${path}`;
  if (files.get(k)?.length !== size) return false;
  files.set(k, "");
  return true;
});

vi.mock("./ipc", () => ({
  fsWriteFile: vi.fn(async () => ({ mtime: 1 })),
  fsCreateDir: vi.fn(async () => {}),
  fsCreateFile: vi.fn(async () => {}),
  fsReadLinesFrom: (root: string, path: string, offset: number | null) => readLines(root, path, offset),
  fsTruncateIfSize: (root: string, path: string, size: number) => truncate(root, path, size),
}));

import {
  serializeFleet, parseOutboxLine, parseSpawnLine, deliveryTargets, drainJsonl, fleetTick, initFleetBridge,
  COMPACT_BYTES, type FleetBridgeHost,
} from "./fleetbridge";

describe("serializeFleet", () => {
  it("emits an {agents:[...]} roster tagged with the workspace name", () => {
    const s = serializeFleet({
      dir: "D:\\demo",
      name: "demo",
      agents: [{ id: "p1", name: "Claude #1", status: "needs" }],
    });
    expect(JSON.parse(s)).toEqual({
      agents: [{ id: "p1", name: "Claude #1", status: "needs", workspace: "demo", screen: "" }],
    });
  });

  it("says each agent's CLI, conversation, folder and start, for agent_context", () => {
    const s = serializeFleet({
      dir: "D:\demo",
      name: "demo",
      agents: [{ id: "p1", name: "Ana", status: "idle", cli: "codex", session: "D:/r/rollout.jsonl", folder: "D:\demo", since: 1000 }],
    });
    expect(JSON.parse(s).agents[0]).toMatchObject({ cli: "codex", session: "D:/r/rollout.jsonl", folder: "D:\demo", since: 1000 });
  });
});

describe("parseOutboxLine", () => {
  it("parses a targeted message with its sender", () => {
    expect(parseOutboxLine('{"from":"Claude #1","to":"Codex #1","message":"do X"}')).toEqual({
      from: "Claude #1",
      to: "Codex #1",
      message: "do X",
    });
  });
  it("treats missing/blank to as a broadcast (null)", () => {
    expect(parseOutboxLine('{"message":"hello"}')).toEqual({ from: null, to: null, message: "hello" });
    expect(parseOutboxLine('{"to":"  ","message":"hi"}')).toEqual({ from: null, to: null, message: "hi" });
  });
  it("treats missing/blank/null from as unknown (null)", () => {
    expect(parseOutboxLine('{"message":"hi"}')?.from).toBeNull();
    expect(parseOutboxLine('{"from":"  ","message":"hi"}')?.from).toBeNull();
    expect(parseOutboxLine('{"from":null,"message":"hi"}')?.from).toBeNull();
  });
  it("returns null for blank, non-json, or message-less lines", () => {
    expect(parseOutboxLine("")).toBeNull();
    expect(parseOutboxLine("not json")).toBeNull();
    expect(parseOutboxLine('{"to":"x"}')).toBeNull();
    expect(parseOutboxLine('{"message":"   "}')).toBeNull();
  });
});

describe("deliveryTargets", () => {
  const info = (p: { name: string; running: boolean }) => p;
  const fleet = [
    { name: "Director", running: true },
    { name: "Ana", running: true },
    { name: "Bob", running: true },
    { name: "Ghost", running: false },
  ];

  it("a broadcast reaches the fleet but never the sender", () => {
    expect(deliveryTargets(fleet, info, "Director", null).map((p) => p.name)).toEqual(["Ana", "Bob"]);
  });

  it("matches the sender's own name loosely, so casing can't sneak it back in", () => {
    expect(deliveryTargets(fleet, info, " director ", null).map((p) => p.name)).toEqual(["Ana", "Bob"]);
  });

  it("an unidentified sender broadcasts to everyone running", () => {
    expect(deliveryTargets(fleet, info, null, null).map((p) => p.name)).toEqual([
      "Director",
      "Ana",
      "Bob",
    ]);
  });

  it("a directed message goes to that agent only, name matched loosely", () => {
    expect(deliveryTargets(fleet, info, "Director", "ana").map((p) => p.name)).toEqual(["Ana"]);
  });

  it("never targets a stopped agent", () => {
    expect(deliveryTargets(fleet, info, null, "Ghost")).toEqual([]);
  });
});

describe("parseSpawnLine", () => {
  it("parses a spawn request, clamps count, defaults task", () => {
    expect(parseSpawnLine('{"cli":"claude","task":"build X","count":3}')).toEqual({
      cli: "claude",
      task: "build X",
      count: 3,
    });
    expect(parseSpawnLine('{"cli":"codex"}')).toEqual({ cli: "codex", task: null, count: 1 });
    expect(parseSpawnLine('{"cli":"x","count":99}')?.count).toBe(6);
  });
  it("returns null without a cli or for junk", () => {
    expect(parseSpawnLine('{"task":"x"}')).toBeNull();
    expect(parseSpawnLine("nope")).toBeNull();
    expect(parseSpawnLine("")).toBeNull();
  });
});

describe("drainJsonl (outbox delivery)", () => {
  const OUT = ".maestro\\outbox.jsonl";
  const key = (dir: string) => `${dir}|${OUT}`;
  const append = (dir: string, text: string) => files.set(key(dir), (files.get(key(dir)) ?? "") + text);
  const msg = (m: string) => JSON.stringify({ to: null, message: m }) + "\n";
  const got: string[] = [];
  const take = (line: string) => void got.push(parseOutboxLine(line)?.message ?? "?");
  let offsets: Map<string, number>;

  beforeEach(() => {
    files.clear();
    got.length = 0;
    offsets = new Map();
    readLines.mockClear();
    truncate.mockClear();
  });

  it("skips the backlog on first sight, then delivers each new line once", async () => {
    append("D:/a", msg("old"));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got).toEqual([]);
    append("D:/a", msg("one") + msg("two"));
    await drainJsonl("D:/a", OUT, offsets, take);
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got).toEqual(["one", "two"]);
  });

  it("a line caught half-written is delivered once it's complete, not lost", async () => {
    append("D:/a", "");
    await drainJsonl("D:/a", OUT, offsets, take);
    const whole = msg("hello");
    append("D:/a", whole.slice(0, 10));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got).toEqual([]);
    append("D:/a", whole.slice(10));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got).toEqual(["hello"]);
  });

  it("keeps delivering past 2 MB (the old whole-file read gave up there)", async () => {
    append("D:/a", msg("x".repeat(1000)).repeat(2500)); // ~2.5 MB of backlog
    await drainJsonl("D:/a", OUT, offsets, take);
    append("D:/a", msg("after"));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got).toEqual(["after"]);
    // Only the new bytes were read, not the whole file.
    expect(readLines.mock.calls[readLines.mock.calls.length - 1]?.[2]).toBeGreaterThan(2_000_000);
  });

  it("delivers the message that creates the outbox", async () => {
    await drainJsonl("D:/a", OUT, offsets, take); // no file yet
    append("D:/a", msg("first"));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got).toEqual(["first"]);
  });

  it("starts over when the file was cut shorter", async () => {
    append("D:/a", msg("a") + msg("b"));
    await drainJsonl("D:/a", OUT, offsets, take);
    files.set(key("D:/a"), msg("new"));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got).toEqual(["new"]);
  });

  it("empties a big outbox once everything in it is read, and carries on", async () => {
    append("D:/a", "");
    await drainJsonl("D:/a", OUT, offsets, take);
    const big = msg("y".repeat(2000));
    const n = Math.ceil(COMPACT_BYTES / big.length) + 1;
    append("D:/a", big.repeat(n));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got.length).toBe(n);
    expect(files.get(key("D:/a"))).toBe("");
    expect(offsets.get("D:/a")).toBe(0);
    append("D:/a", msg("later"));
    await drainJsonl("D:/a", OUT, offsets, take);
    expect(got[got.length - 1]).toBe("later");
    expect(got.length).toBe(n + 1);
  });

  it("a handler that throws doesn't stop the rest or cause a re-delivery", async () => {
    append("D:/a", "");
    await drainJsonl("D:/a", OUT, offsets, take);
    append("D:/a", msg("boom") + msg("ok"));
    const seen: string[] = [];
    const h = (l: string) => {
      const m = parseOutboxLine(l)!.message;
      seen.push(m);
      if (m === "boom") throw new Error("x");
    };
    await drainJsonl("D:/a", OUT, offsets, h);
    await drainJsonl("D:/a", OUT, offsets, h);
    expect(seen).toEqual(["boom", "ok"]);
  });
});

describe("initFleetBridge", () => {
  beforeEach(() => {
    files.clear();
    readLines.mockClear();
  });
  afterEach(() => {
    vi.useRealTimers();
    readLines.mockImplementation(fakeRead);
  });

  it("never runs a tick while the last one is still reading", async () => {
    vi.useFakeTimers();
    let inFlight = 0;
    let most = 0;
    readLines.mockImplementation(async () => {
      inFlight += 1;
      most = Math.max(most, inFlight);
      await new Promise((r) => setTimeout(r, 5000)); // a slow disk
      inFlight -= 1;
      throw new Error("no such path");
    });
    const host: FleetBridgeHost = {
      workspaces: () => [{ dir: "D:/slow", name: "slow", agents: [] }],
      deliver: () => {},
      spawn: () => {},
    };
    const stop = initFleetBridge(host, 1500);
    await vi.advanceTimersByTimeAsync(20_000);
    stop();
    expect(most).toBe(2); // outbox + spawn requests of one tick, never two ticks at once
    expect(readLines.mock.calls.length).toBeLessThanOrEqual(8);
  });

  it("drains a folder shared by two workspaces once", async () => {
    const dir = "D:/shared";
    const out = `${dir}|.maestro\\outbox.jsonl`;
    files.set(out, "");
    const delivered: string[] = [];
    const host: FleetBridgeHost = {
      workspaces: () => [
        { dir, name: "a", agents: [] },
        { dir, name: "b", agents: [] },
      ],
      deliver: (_d, _f, _t, m) => void delivered.push(m),
      spawn: () => {},
    };
    await fleetTick(host); // first sight
    files.set(out, JSON.stringify({ message: "hi" }) + "\n");
    await fleetTick(host);
    expect(delivered).toEqual(["hi"]);
  });
});
