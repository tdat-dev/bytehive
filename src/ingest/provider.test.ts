import { describe, expect, it } from "vitest";
import { parseClassified } from "./provider";

describe("parseClassified", () => {
  it("reads the two fields", () => {
    expect(parseClassified('{"is_actionable":true,"data":{"intent":"estimate_cost"}}')).toEqual({
      is_actionable: true,
      data: { intent: "estimate_cost" },
    });
  });

  it("reads JSON wrapped in a code fence", () => {
    expect(parseClassified('```json\n{"is_actionable":false,"data":null}\n```')).toEqual({
      is_actionable: false,
      data: null,
    });
  });

  it("rejects a missing or wrong field", () => {
    expect(parseClassified('{"data":null}')).toBeNull();
    expect(parseClassified('{"is_actionable":"no","data":null}')).toBeNull();
    expect(parseClassified('{"is_actionable":true,"data":[]}')).toBeNull();
    expect(parseClassified("not json")).toBeNull();
  });
});
