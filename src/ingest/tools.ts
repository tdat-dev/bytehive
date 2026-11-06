/** The code that is allowed to do arithmetic. Every tool reads only; none may
 *  call a model, and none may guess a price. */
import { priceFor, usageCost } from "../pricing";
import type { Tool, ToolResult } from "./types";

const meta = (tool: string, version: string, cacheable: boolean) => ({ tool, version, cacheable });
const num = (v: unknown): number | null => {
  const n = typeof v === "number" ? v : typeof v === "string" && v.trim() !== "" ? Number(v) : NaN;
  return Number.isFinite(n) && n >= 0 ? n : null;
};
const str = (v: unknown): string => (typeof v === "string" ? v : "");

function ok(tool: string, version: string, cacheable: boolean, values: Record<string, number | string>): ToolResult {
  return { status: "ok", values, timestamp: Date.now(), metadata: meta(tool, version, cacheable) };
}

function bad(tool: string, version: string, cacheable: boolean, error: string): ToolResult {
  return { status: "error", values: {}, timestamp: Date.now(), metadata: meta(tool, version, cacheable), error };
}

/** Token counts to USD at the list prices in pricing.ts. */
export function estimateCostTool(): Tool {
  const name = "estimate_cost";
  const version = "1";
  return {
    name,
    version,
    cacheable: true,
    async run(input) {
      const model = str(input.model);
      const inputTokens = num(input.inputTokens);
      const outputTokens = num(input.outputTokens);
      const cacheRead = num(input.cacheReadTokens) ?? 0;
      const cacheWrite = num(input.cacheWriteTokens) ?? 0;
      if (!model || inputTokens === null || outputTokens === null) {
        return bad(name, version, true, "model, inputTokens and outputTokens are required");
      }
      const price = priceFor(model);
      if (!price) return bad(name, version, true, `no list price for ${model}`);
      const usd = usageCost({
        model,
        input_tokens: inputTokens,
        output_tokens: outputTokens,
        cache_creation: cacheWrite,
        cache_read: cacheRead,
        messages: 0,
      });
      if (usd === null) return bad(name, version, true, `no list price for ${model}`);
      return ok(name, version, true, {
        usd,
        inputTokens,
        outputTokens,
        inputUsd: (inputTokens * price.in) / 1e6,
        outputUsd: (outputTokens * price.out) / 1e6,
      });
    },
  };
}

/** Money at rates the user declares as units per one US dollar. */
export function convertCurrencyTool(rates: Record<string, number>, version = "1"): Tool {
  const name = "convert_currency";
  return {
    name,
    version,
    cacheable: true,
    async run(input) {
      const amount = num(input.amount);
      const from = str(input.from).toUpperCase();
      const to = str(input.to).toUpperCase();
      if (amount === null || !from || !to) return bad(name, version, true, "amount, from and to are required");
      const a = rates[from];
      const b = rates[to];
      if (!a || !b) return bad(name, version, true, `no rate for ${!a ? from : to}`);
      return ok(name, version, true, { amount: (amount / a) * b, rate: b / a, from, to });
    },
  };
}

export interface Budget {
  tokensLeft: number;
  tokenCap: number;
  jobId: string;
}

/** Live numbers, so never cached. A missing company is an error, not a zero. */
export function remainingBudgetTool(read: () => Promise<Budget | null>): Tool {
  const name = "remaining_budget";
  const version = "1";
  return {
    name,
    version,
    cacheable: false,
    async run() {
      try {
        const b = await read();
        if (!b) return bad(name, version, false, "no company or Job is running");
        return ok(name, version, false, { tokensLeft: b.tokensLeft, tokenCap: b.tokenCap, jobId: b.jobId });
      } catch (e) {
        return bad(name, version, false, String(e));
      }
    },
  };
}

/** The tools the app starts with. */
export function defaultTools(options: { rates?: Record<string, number>; budget?: () => Promise<Budget | null> } = {}): Tool[] {
  const tools = [estimateCostTool(), convertCurrencyTool(options.rates ?? {})];
  if (options.budget) tools.push(remainingBudgetTool(options.budget));
  return tools;
}
