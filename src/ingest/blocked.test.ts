import { describe, expect, it } from "vitest";
import { BlockedStore, liveTarget } from "./blocked";
import type { BlockedEntry } from "./types";

const entry = (id: string, at = 1): BlockedEntry => ({ id, hash: "h", source: "fleet", to: "a1", text: id, reason: "smalltalk", at });

function memory(start: string | null = null) {
  let text = start;
  return {
    read: async () => text,
    write: async (t: string) => {
      text = t;
    },
  };
}

describe("liveTarget", () => {
  const held = { ...entry("x"), to: "old-id", toName: "Bob" };

  it("uses the pane it was meant for while that pane runs", () => {
    expect(liveTarget([{ id: "old-id", name: "Bob", running: true }], held)).toBe("old-id");
  });

  it("finds the agent by name after a restart gave it a new pane", () => {
    expect(liveTarget([{ id: "new-id", name: "Bob", running: true }], held)).toBe("new-id");
  });

  it("gives up on a stopped agent, so nothing claims a delivery", () => {
    expect(liveTarget([{ id: "old-id", name: "Bob", running: false }], held)).toBeNull();
    expect(liveTarget([{ id: "other", name: "Ana", running: true }], held)).toBeNull();
  });
});

describe("BlockedStore", () => {
  it("starts empty when there is no file", async () => {
    expect(await new BlockedStore(memory()).list()).toEqual({ entries: [], damaged: false });
  });

  it("keeps both of two blocks that land together", async () => {
    const s = new BlockedStore(memory());
    await Promise.all([s.add(entry("a")), s.add(entry("b", 2))]);
    expect((await s.list()).entries.map((e) => e.id)).toEqual(["a", "b"]);
  });

  it("hands back what it removes, and null the second time", async () => {
    const s = new BlockedStore(memory());
    await s.add(entry("a"));
    expect((await s.remove("a"))?.text).toBe("a");
    expect(await s.remove("a")).toBeNull();
    expect((await s.list()).entries).toHaveLength(0);
  });

  it("puts a restored entry back in time order", async () => {
    const s = new BlockedStore(memory());
    await s.add(entry("old", 1));
    await s.add(entry("new", 3));
    const gone = await s.remove("old");
    await s.add(gone as BlockedEntry);
    expect((await s.list()).entries.map((e) => e.id)).toEqual(["old", "new"]);
  });

  it("adding an entry it already holds does not double it", async () => {
    const s = new BlockedStore(memory());
    await s.add(entry("a"));
    await s.add(entry("a"));
    expect((await s.list()).entries).toHaveLength(1);
  });

  it("says so when the file is damaged instead of showing an empty list", async () => {
    expect(await new BlockedStore(memory("{not json")).list()).toEqual({ entries: [], damaged: true });
  });

  it("a failed write does not stop the next one", async () => {
    const m = memory();
    let fail = true;
    const s = new BlockedStore({
      read: m.read,
      write: async (t) => {
        if (fail) {
          fail = false;
          throw new Error("disk");
        }
        await m.write(t);
      },
    });
    await expect(s.add(entry("a"))).rejects.toThrow("disk");
    await s.add(entry("b"));
    expect((await s.list()).entries.map((e) => e.id)).toEqual(["b"]);
  });
});
