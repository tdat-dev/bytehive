// Code that finds an element by class needs some markup to render that class. When a
// restyle drops a class from the markup the lookup goes quiet: a `?.` binds nothing, a
// `!` throws, and the button is dead with no error on screen. This reads the source.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const walk = (d: string): string[] => readdirSync(d).flatMap((f) => {
  const p = join(d, f);
  return statSync(p).isDirectory() ? walk(p) : [p];
});
const bare = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
const files = walk("src").filter((f) => f.endsWith(".ts") && !f.endsWith(".test.ts"));
const sources = files.map((f) => [f.replace(/\\/g, "/"), bare(readFileSync(f, "utf8"))] as const);
const html = ["index.html", "office-demo.html"].map((f) => readFileSync(f, "utf8")).join("\n");

/** Other libraries draw these; no markup of ours would. */
const FOREIGN = /^(xterm|cm-|xterm-)/;
/** Lookups already dead before this test: the background picker's swatches and the old
 *  dock rail have no markup. Package D deletes both. Nothing else goes on this list. */
const KNOWN_DEAD = new Set(["bg-opt", "dr-btn", "dr-timer"]);

/** Every class our code or pages can put on an element, and the prefixes built at run time. */
function rendered(): { names: Set<string>; prefixes: string[] } {
  const names = new Set<string>();
  const prefixes: string[] = [];
  const words = (s: string) => {
    // a `${...}` in a class list: what is around it is real, what ends at it is a prefix
    const parts = s.replace(/\$\{(?:[^{}]|\{[^{}]*\})*\}/g, "\u0001");
    for (const w of parts.split(/\s+/)) {
      if (!w) continue;
      const head = w.split("\u0001")[0];
      if (head.length < w.length) { if (head) prefixes.push(head); } else names.add(w);
    }
  };
  for (const [, s] of sources) {
    for (const m of s.matchAll(/class="((?:[^"$]|\$\{(?:[^{}]|\{[^{}]*\})*\})*)"/g)) words(m[1]);
    for (const m of s.matchAll(/className\s*=\s*(?:[^;\n]*?)(["'`])([^"'`]*)\1/g)) words(m[2]);
    for (const m of s.matchAll(/className\s*=[^;\n]*/g)) for (const q of m[0].matchAll(/["'`]([^"'`]*)["'`]/g)) words(q[1]);
    for (const m of s.matchAll(/classList\.(?:add|toggle|replace)\(([^)]*)\)/g)) for (const q of m[1].matchAll(/["'`]([^"'`]+)["'`]/g)) words(q[1]);
    // the helpers that take a class as an argument: el("div", "kb-card"), and ternaries in class=""
    for (const m of s.matchAll(/\bel\(\s*["'`]\w+["'`]\s*,\s*(["'`])([^"'`]*)\1/g)) words(m[2]);
    for (const m of s.matchAll(/\$\{[^{}]*\?\s*["'`]\s*([\w -]+?)\s*["'`]\s*:\s*["'`]\s*([\w -]*?)\s*["'`]\s*\}/g)) { words(m[1]); words(m[2]); }
  }
  for (const m of html.matchAll(/class="([^"]*)"/g)) words(m[1]);
  return { names, prefixes };
}

/** Each class a lookup asks for, with the file that asks. */
function asked(): Array<[string, string]> {
  const out: Array<[string, string]> = [];
  const call = /\.(?:querySelector(?:All)?|closest|matches)(?:<[^>]*>)?\(\s*(["'`])((?:(?!\1).)*)\1/g;
  for (const [f, s] of sources) {
    for (const m of s.matchAll(call)) {
      const sel = m[2].replace(/\$\{[^}]*\}/g, "").replace(/\[[^\]]*\]/g, "");
      for (const c of sel.matchAll(/\.(-?[a-zA-Z_][\w-]*)/g)) out.push([c[1], f]);
    }
  }
  return out;
}

describe("a class that code looks for", () => {
  it("finds the lookups and the markup at all", () => {
    expect(asked().length).toBeGreaterThan(100);
    expect(rendered().names.size).toBeGreaterThan(200);
  });
  it("is rendered by something", () => {
    const { names, prefixes } = rendered();
    const lost = asked().filter(([c]) => !FOREIGN.test(c) && !KNOWN_DEAD.has(c) && !names.has(c) && !prefixes.some((p) => c.startsWith(p)));
    expect([...new Set(lost.map(([c, f]) => `${f}: .${c}`))]).toEqual([]);
  });
});
