import { describe, expect, it } from "vitest";
import { cardGist, dispatchPrompt } from "./dispatch";
import { mkCard } from "./board";

describe("cardGist", () => {
  it("is the card's own words, with nothing dispatch added", () => {
    expect(cardGist(mkCard("cảm ơn nhé"))).toBe("cảm ơn nhé");
  });

  it("keeps the description and open items, so a real card still reads as work", () => {
    const card = mkCard("thanks");
    card.desc = "then fix the login redirect";
    card.checklist = [{ id: "i1", text: "add test", done: false }];
    expect(cardGist(card)).toContain("fix the login redirect");
    expect(cardGist(card)).toContain("add test");
  });
});

describe("dispatchPrompt", () => {
  it("contains the task text and the board-tool instructions", () => {
    const card = mkCard("Fix login bug");
    card.desc = "Session cookie expires too early";
    card.checklist = [{ id: "i1", text: "add test", done: false }];
    const p = dispatchPrompt(card);
    expect(p).toContain("Task: Fix login bug");
    expect(p).toContain("Session cookie expires too early");
    expect(p).toContain("- [ ] add test");
    expect(p).toContain("card_move");
    expect(p).toContain('"Doing"');
    expect(p).toContain("card_done");
  });

  it("skips finished checklist items and empty desc", () => {
    const card = mkCard("Tidy");
    card.checklist = [{ id: "i1", text: "done thing", done: true }];
    const p = dispatchPrompt(card);
    expect(p).not.toContain("done thing");
    expect(p.startsWith("Task: Tidy")).toBe(true);
  });
});
