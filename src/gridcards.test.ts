import { describe, expect, it } from "vitest";
import { doingOf, glanceOf, plain, tail } from "./gridcards";
import type { ChatItem } from "./chatmodel";

const text = (t: string): ChatItem => ({ kind: "text", id: t, text: t, at: 1 });
const user = (t: string): ChatItem => ({ kind: "user", id: t, text: t, images: 0, at: 1 });
const step = (verb: string, target: string, done: boolean): ChatItem =>
  ({ kind: "step", id: target, tool: "Edit", verb, target, done, code: true, at: 1 }) as ChatItem;

describe("what a card says", () => {
  it("shows the last reply, and the step it is on after it", () => {
    expect(glanceOf([user("fix it"), text("Looking at the handler."), step("Edited", "handler.ts", false)]))
      .toEqual({ said: "Looking at the handler.", doing: "Editing handler.ts", asked: "" });
  });

  it("drops a step that is done", () => {
    expect(glanceOf([text("Done."), step("Ran", "npm test", true)]).doing).toBe("");
  });

  it("shows what you asked while it has not answered yet", () => {
    expect(glanceOf([text("old reply"), user("now add tests"), step("Read", "a.ts", false)]))
      .toEqual({ said: "", doing: "Reading a.ts", asked: "now add tests" });
  });

  it("says a step as what it is doing now", () => {
    expect(doingOf({ tool: "Bash", verb: "Ran", target: "npm test", code: true })).toBe("Running npm test");
    expect(doingOf({ tool: "Bash", verb: "Ran", target: "Benchmark indexing", code: false })).toBe("Benchmark indexing");
    expect(doingOf({ tool: "Grep", verb: "Searched for", target: "TODO" })).toBe("Searching for TODO");
  });

  it("reads markdown as words", () => {
    expect(plain("## Result\n\n**Fixed** the `upload` test:\n- one\n- two\n\n```ts\nx()\n```"))
      .toBe("Result\n\nFixed the upload test:\n• one\n• two\n\nx()");
  });

  it("keeps the end of a long reply, cut at a word", () => {
    const t = tail("word ".repeat(200) + "the conclusion.", 60);
    expect(t.startsWith("…")).toBe(true);
    expect(t.endsWith("the conclusion.")).toBe(true);
    expect(t.length).toBeLessThanOrEqual(61);
  });
});
