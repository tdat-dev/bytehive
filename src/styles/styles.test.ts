// The design language as rules a file can break. ux-overhaul.md section 5.
import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const here = "src/styles";
const css = readdirSync(here).filter((f) => f.endsWith(".css"));
const bare = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "");
const files = css.map((f) => [f, bare(readFileSync(`${here}/${f}`, "utf8"))] as const);
const hits = (re: RegExp) => files.flatMap(([f, s]) => [...s.matchAll(re)].map((m) => `${f}: ${m[0]}`));

describe("type", () => {
  it("has no size off the scale", () => {
    expect(hits(/font-size:\s*\d[\d.]*px/g).filter((h) => !/font-size:\s*0px/.test(h))).toEqual([]);
  });
  it("has no size in a font shorthand off the scale", () => {
    expect(hits(/\bfont:\s*(?:\d{3}\s+)?\d[\d.]*px[^;}]*/g).filter((h) => !/tokens\.css/.test(h) && !/var\(--fs-/.test(h))).toEqual([]);
  });
  it("draws no uppercase eyebrows", () => {
    expect(hits(/text-transform:\s*uppercase/g)).toEqual([]);
  });
  it("asks only for the weights it ships", () => {
    expect(hits(/font-weight:\s*(?:[1-3]00|[7-9]00|bold)\b/g)).toEqual([]);
  });
});
