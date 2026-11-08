import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ICON_CLOSE, glyph, icon } from "./icons";

const html = readFileSync("index.html", "utf8");
const sprite = new Set([...html.matchAll(/<symbol id="([^"]+)"/g)].map((m) => m[1]));
const sources = [html, ...readdirSync("src").filter((f) => f.endsWith(".ts") && !f.endsWith(".test.ts")).map((f) => readFileSync(`src/${f}`, "utf8"))];

describe("the sprite", () => {
  it("has every icon the app names", () => {
    const named = sources.flatMap((s) => [...s.matchAll(/href="#(i-[a-z0-9-]+|s-[a-z]+)"/g), ...s.matchAll(/\bicon\("([a-z0-9-]+)"/g)].map((m) => (m[1].match(/^[is]-/) ? m[1] : `i-${m[1]}`)));
    expect(named.filter((n) => !sprite.has(n))).toEqual([]);
  });
  it("is in the page once", () => {
    expect(html.match(/class="sprite"/g)).toHaveLength(1);
  });
  it("gives every state a shape of its own", () => {
    const shapes = ["needs", "review", "work", "idle", "stopped", "paused", "out", "error", "merged"].map((s) => new RegExp(`<symbol id="s-${s}"[^>]*>(.*?)</symbol>`).exec(html)?.[1]);
    expect(shapes.every(Boolean)).toBe(true);
    expect(new Set(shapes).size).toBe(shapes.length);
  });
});

describe("icon", () => {
  it("draws from the sprite", () => {
    expect(icon("x")).toBe('<svg class="i" aria-hidden="true"><use href="#i-x"/></svg>');
    expect(ICON_CLOSE).toBe(icon("x"));
  });
  it("names a state's glyph by the list's word for it", () => {
    expect(glyph("working")).toContain("#s-work");
    expect(glyph("needs")).toContain("g-needs");
    expect(glyph("nobody knows")).toContain("#s-idle");
  });
});
