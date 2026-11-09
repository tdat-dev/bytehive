import { describe, expect, it } from "vitest";
import { collectState } from "./export";

const store = (entries: Record<string, string>) => {
  const keys = Object.keys(entries);
  return { length: keys.length, key: (i: number) => keys[i] ?? null, getItem: (k: string) => entries[k] ?? null };
};

describe("collectState", () => {
  it("hands over our entries only, as stored", () => {
    const s = store({ "maestro.session": "[]", "maestro.prefs": "{\"worktree\":true}", "other.thing": "x" });
    expect(collectState(s)).toEqual({ "maestro.session": "[]", "maestro.prefs": "{\"worktree\":true}" });
  });
});
