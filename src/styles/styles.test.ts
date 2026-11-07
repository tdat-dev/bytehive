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

describe("shape", () => {
  it("has three radii, a circle and the status word", () => {
    const off = hits(/border(?:-\w+)*-radius:\s*[^;}]+/g).filter((h) => !/tokens\.css/.test(h) && !/radius:\s*((var\(--r-(control|card|dialog|word)\)|0|50%|inherit)\s*)+$/.test(h));
    expect(off).toEqual([]);
  });
  it("blurs nothing behind a panel", () => {
    expect(hits(/backdrop-filter/g)).toEqual([]);
  });
  it("stacks by the named scale, and small numbers stay inside a component", () => {
    const off = hits(/z-index:\s*[^;}]+/g).filter((h) => !/z-index:\s*(var\(--z-(rail|sheet|popover|scrim|dialog|toast)\)|-1|[0-3])\s*$/.test(h));
    expect(off).toEqual([]);
  });
});

describe("motion", () => {
  const motion = (s: string) => s.replace(/@keyframes[^{]+\{(?:[^{}]*\{[^{}]*\})*[^{}]*\}/g, "");
  it("uses the four timings and the two curves", () => {
    const off = files.filter(([f]) => f !== "splash.css" && f !== "tokens.css").flatMap(([f, s]) =>
      [...motion(s).matchAll(/\btransition:\s*([^;}]+)/g)].map((m) => `${f}: ${m[1]}`).filter((d) => !/none\s*$/.test(d) && !/^[^:]+: (?:[\w-]+ var\(--m-(press|swap|panel|level)\) var\(--ease-(in|out)\)(?:,\s*)?)+\s*$/.test(d)));
    expect(off).toEqual([]);
  });
  it("moves only what the compositor can", () => {
    expect(hits(/\btransition:[^;}]*\b(width|height|left|top|box-shadow|filter)\b/g)).toEqual([]);
  });
  it("keeps one loop", () => {
    expect(hits(/animation:[^;}]*\binfinite\b/g)).toEqual(["parts.css: animation:spin 1s linear infinite"]);
  });
});

describe("space", () => {
  it("steps by the scale", () => {
    const off = files.filter(([f]) => f !== "splash.css" && f !== "tokens.css").flatMap(([f, s]) =>
      [...s.matchAll(/\b(?:gap|row-gap|column-gap|padding(?:-\w+)?|margin(?:-\w+)?):\s*([^;}]+)/g)].flatMap((m) =>
        /calc\(|var\(--s\d+\)|min\(|max\(/.test(m[1]) ? [] : m[1].split(/\s+/).filter((t) => /^\d[\d.]*px$/.test(t) && parseFloat(t) > 2 && parseFloat(t) <= 48).map((t) => `${f}: ${m[0]}`)));
    expect(off).toEqual([]);
  });
});
