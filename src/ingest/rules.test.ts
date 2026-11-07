import { describe, expect, it } from "vitest";
import { rules } from "./rules";

describe("the cost rule", () => {
  it("answers a question that is only about cost", () => {
    expect(rules("báo giá 120k input, 30k output, model claude-sonnet-5")?.data).toMatchObject({
      intent: "estimate_cost",
      inputTokens: 120000,
      outputTokens: 30000,
    });
    expect(rules("250 nghìn token vào với 40 nghìn token ra của opus thì hết bao nhiêu?")?.data).toMatchObject({
      intent: "estimate_cost",
      model: "claude-opus-5-5",
    });
  });

  it.each([
    "sửa hàm tính cost trong pricing.ts, test với claude-sonnet-5 120k input 30k output",
    "fix the cost bug for claude-opus-5-5 with 1000 in 2000 out",
    "thêm giá cho claude-haiku-4-5 rồi kiểm tra 5000 input 100 output",
  ])("leaves a coding task to the classifier: %s", (text) => {
    expect(rules(text)).toBeNull();
  });
});

describe("short messages", () => {
  it.each(["cảm ơn nhé", "thanks", "hi", "chào bạn nhé".replace(" bạn", "")])("blocks courtesy: %s", (text) => {
    expect(rules(text)).toEqual({ is_actionable: false, data: null });
  });

  // Each of these can be the answer to a question asked in plain text, which
  // never sets the agent's ask status, or a report that the work is finished.
  it.each(["yes", "no", "ok", "done", "rồi", "vâng", "ok cảm ơn"])("never blocks an answer: %s", (text) => {
    expect(rules(text)).toEqual({ is_actionable: true, data: null });
  });
});
