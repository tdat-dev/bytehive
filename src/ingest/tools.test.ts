import { describe, expect, it } from "vitest";
import { convertCurrencyTool, estimateCostTool } from "./tools";

describe("estimate_cost", () => {
  it("prices sonnet-5 at list price", async () => {
    const r = await estimateCostTool().run({
      model: "claude-sonnet-5",
      inputTokens: 120000,
      outputTokens: 30000,
    });
    expect(r.status).toBe("ok");
    expect(r.values.usd as number).toBeCloseTo(0.54, 9);
  });

  it("prices opus-5-5 at list price", async () => {
    const r = await estimateCostTool().run({
      model: "claude-opus-5-5",
      inputTokens: 250000,
      outputTokens: 40000,
    });
    expect(r.status).toBe("ok");
    expect(r.values.usd as number).toBeCloseTo(1.8, 9);
  });

  it("refuses a model it cannot price", async () => {
    const r = await estimateCostTool().run({ model: "gpt-6-luna", inputTokens: 1, outputTokens: 1 });
    expect(r.status).toBe("error");
    expect(r.error).toContain("no list price");
  });
});

describe("convert_currency", () => {
  const rates = { USD: 1, VND: 25000 };

  it("goes through the dollar", async () => {
    const r = await convertCurrencyTool(rates).run({ amount: 100000, from: "vnd", to: "usd" });
    expect(r.status).toBe("ok");
    expect(r.values.amount as number).toBeCloseTo(4, 9);
  });

  it("refuses a currency with no declared rate", async () => {
    const r = await convertCurrencyTool(rates).run({ amount: 1, from: "usd", to: "eur" });
    expect(r.status).toBe("error");
    expect(r.error).toContain("EUR");
  });
});
