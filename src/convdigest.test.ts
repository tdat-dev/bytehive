import { describe, expect, it } from "vitest";
import { createChat } from "./chatmodel";
import { asksSection, digestOf, planProgress, planSection } from "./convdigest";

const T = "2026-09-27T10:00:00.000Z";
const j = (o: object) => JSON.stringify({ timestamp: T, ...o }) + "\n";
const user = (text: string) => j({ type: "user", origin: { kind: "human" }, message: { content: text } });

describe("a conversation's digest", () => {
  it("puts the first asks before the later ones, once each", () => {
    const chat = createChat();
    chat.feed(user("second") + user("third"));
    expect(digestOf(chat, ["first", "second"]).asks).toEqual(["first", "second", "third"]);
  });

  it("is empty without a conversation", () => {
    expect(digestOf(null)).toEqual({ asks: [], todos: [], files: [], lastWords: "" });
  });

  it("counts the plan", () => {
    expect(planProgress([
      { text: "a", state: "completed" }, { text: "b", state: "in_progress" }, { text: "c", state: "pending" },
    ])).toBe("1/3 done");
    expect(planProgress([])).toBe("");
  });

  it("shows the first ask and the latest ones of many", () => {
    const s = asksSection(Array.from({ length: 10 }, (_, k) => `ask ${k + 1}`), "Asked");
    expect(s).toContain("Asked (the first, then the latest 5 of 10):");
    expect(s).toContain("1. ask 1\n2. ask 6");
    expect(asksSection([], "Asked")).toBe("");
  });

  it("marks each step of the plan", () => {
    expect(planSection([{ text: "a", state: "completed" }, { text: "b", state: "pending" }], "Plan:"))
      .toBe("Plan:\n- [done] a\n- [to do] b");
  });
});
