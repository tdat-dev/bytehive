import { describe, expect, it } from "vitest";
import { money, priceFor, totalCost, usageCost } from "./pricing";

const use = (model: string, input_tokens: number, output_tokens: number, cache_creation = 0, cache_read = 0) =>
  ({ model, input_tokens, output_tokens, cache_creation, cache_read, messages: 1 });

describe("what Claude usage costs", () => {
  it("prices each model by its own id, newest first", () => {
    expect(priceFor("claude-opus-5-5")?.in).toBe(4);
    expect(priceFor("claude-opus-5")?.in).toBe(5);
    expect(priceFor("claude-opus-4-5-20251101")?.in).toBe(5);
    expect(priceFor("claude-opus-4-1-20250805")?.in).toBe(15);
    expect(priceFor("claude-sonnet-5")?.out).toBe(10);
    expect(priceFor("claude-sonnet-4-6")?.out).toBe(15);
    expect(priceFor("claude-haiku-4-5-20251001")?.in).toBe(1);
    expect(priceFor("claude-fable-5-1")?.cacheRead).toBe(0.25);
    expect(priceFor("gpt-5-codex")).toBeNull();
  });

  it("adds input, output and both kinds of cache at their own rates", () => {
    // 1M of each on Opus 5.5: 4 + 20 + 5 + 0.2
    expect(usageCost(use("claude-opus-5-5", 1e6, 1e6, 1e6, 1e6))).toBeCloseTo(29.2);
    expect(usageCost(use("some-other-model", 1e6, 1e6))).toBeNull();
  });

  it("totals the priced models, and says so when none is", () => {
    expect(totalCost([use("claude-opus-5-5", 0, 1e6), use("claude-haiku-4-5", 0, 1e6), use("mystery", 1e9, 1e9)])).toBeCloseTo(25);
    expect(totalCost([use("mystery", 1, 1)])).toBeNull();
    expect(totalCost([])).toBeNull();
  });

  it("writes an amount as short as it can be read", () => {
    expect(money(0.004)).toBe("$0.004");
    expect(money(1.234)).toBe("$1.23");
    expect(money(12.6)).toBe("$13");
  });
});
