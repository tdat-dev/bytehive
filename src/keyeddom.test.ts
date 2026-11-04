// @vitest-environment happy-dom
import { describe, it, expect } from "vitest";
import { drawBlocks } from "./keyeddom";

const kids = (el: Element) => [...el.children].map((c) => c.textContent);

describe("drawBlocks", () => {
  it("keeps the nodes of blocks that didn't change and adds the new one", () => {
    const host = document.createElement("div");
    drawBlocks(host, [{ key: "a", html: "<p>a</p>" }, { key: "b", html: "<p>b</p>" }]);
    const [a, b] = [...host.children];
    drawBlocks(host, [{ key: "a", html: "<p>a</p>" }, { key: "b", html: "<p>b</p>" }, { key: "c", html: "<p>c</p>" }]);
    expect(kids(host)).toEqual(["a", "b", "c"]);
    expect(host.children[0]).toBe(a);
    expect(host.children[1]).toBe(b);
  });

  it("swaps a block whose html changed, moves, and drops what's gone", () => {
    const host = document.createElement("div");
    drawBlocks(host, [{ key: "a", html: "<p>a</p>" }, { key: "b", html: "<p>b</p>" }, { key: "c", html: "<p>c</p>" }]);
    const [a, , c] = [...host.children];
    drawBlocks(host, [{ key: "c", html: "<p>c</p>" }, { key: "a", html: "<p>A</p>" }]);
    expect(kids(host)).toEqual(["c", "A"]);
    expect(host.children[0]).toBe(c);
    expect(host.children[1]).not.toBe(a);
  });

  it("draws nested blocks inside a shell that stays", () => {
    const host = document.createElement("div");
    const group = (n: number) => ({ key: "g", html: `<div class="g"></div>`, kids: Array.from({ length: n }, (_, k) => ({ key: `s${k}`, html: `<i>${k}</i>` })) });
    drawBlocks(host, [group(2)]);
    const shell = host.firstElementChild!;
    const first = shell.firstElementChild;
    drawBlocks(host, [group(3)]);
    expect(host.firstElementChild).toBe(shell);
    expect(shell.firstElementChild).toBe(first);
    expect(kids(shell)).toEqual(["0", "1", "2"]);
  });

  it("skips empty blocks, tells repeated keys apart, and clears what was there", () => {
    const host = document.createElement("div");
    host.innerHTML = "<span>stale</span> text";
    drawBlocks(host, [{ key: "x", html: "" }, { key: "d", html: "<p>1</p>" }, { key: "d", html: "<p>2</p>" }]);
    expect(kids(host)).toEqual(["1", "2"]);
    expect(host.childNodes.length).toBe(2);
  });
});
