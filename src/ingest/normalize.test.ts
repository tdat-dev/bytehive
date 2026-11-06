import { describe, expect, it } from "vitest";
import { collapse, expandModels, expandNumbers, normalize } from "./normalize";

describe("normalize", () => {
  it("collapses whitespace", () => {
    expect(collapse("  hai   từ \n mới ")).toBe("hai từ mới");
  });

  it("expands k, nghìn and triệu", () => {
    expect(expandNumbers("120k input")).toBe("120000 input");
    expect(expandNumbers("250 nghìn token")).toBe("250000 token");
    expect(expandNumbers("1,5 triệu")).toBe("1500000");
  });

  it("leaves an ordinary number alone", () => {
    expect(expandNumbers("model 5 và 30000")).toBe("model 5 và 30000");
  });

  it("turns a family word into a model id once", () => {
    expect(expandModels("của opus")).toBe("của claude-opus-5-5");
    expect(expandModels("claude-opus-5-5")).toBe("claude-opus-5-5");
  });

  it("normalizes in one pass", () => {
    expect(normalize("  báo giá 120k  input của sonnet ")).toBe("báo giá 120000 input của claude-sonnet-5");
  });
});
