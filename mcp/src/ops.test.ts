import { describe, expect, it } from "vitest";
import { defaultBoard } from "./board.js";
import { addCard, markDone, moveCard } from "./ops.js";

describe("actor identity", () => {
  it("markDone stamps done.by", () => {
    const b = defaultBoard();
    addCard(b, "To do", { title: "fix bug" });
    const card = markDone(b, "fix bug", { repoRoot: "r", files: [], by: "Claude Code #1" });
    expect(card.done?.by).toBe("Claude Code #1");
  });

  it("moveCard to Doing claims an unassigned card for the actor", () => {
    const b = defaultBoard();
    addCard(b, "To do", { title: "fix bug" });
    const card = moveCard(b, "fix bug", "Doing", undefined, "Claude Code #1");
    expect(card.assignee).toBe("Claude Code #1");
  });

  it("moveCard to Doing records who claimed it by id too, for a member of the company", () => {
    const b = defaultBoard();
    addCard(b, "To do", { title: "fix bug" });
    const card = moveCard(b, "fix bug", "Doing", undefined, "Dot", "p7");
    expect(card).toMatchObject({ assignee: "Dot", assigneeId: "p7" });
  });

  it("moveCard never overwrites an existing assignee", () => {
    const b = defaultBoard();
    const c = addCard(b, "To do", { title: "fix bug" });
    c.assignee = "Codex #1";
    moveCard(b, "fix bug", "Doing", undefined, "Claude Code #1");
    expect(c.assignee).toBe("Codex #1");
  });

  it("moveCard to a non-Doing list does not claim", () => {
    const b = defaultBoard();
    addCard(b, "To do", { title: "fix bug" });
    const card = moveCard(b, "fix bug", "Done", undefined, "Claude Code #1");
    expect(card.assignee).toBeUndefined();
  });
});

describe("returns", () => {
  const listed = (from: string) => {
    const b = defaultBoard();
    b.lists.push({ id: "lreview00001", title: "Review", cards: [] });
    const c = addCard(b, from, { title: "login" });
    c.job = "J-0001";
    return { b, c };
  };

  it("counts a Job card sent back from Review to Doing", () => {
    const { b, c } = listed("Review");
    moveCard(b, "login", "Doing");
    expect(c.returns).toBe(1);
    moveCard(b, "login", "Review");
    moveCard(b, "login", "To do");
    expect(c.returns).toBe(2);
  });

  it("counts a Job card sent back from Done", () => {
    const { b, c } = listed("Done");
    moveCard(b, "login", "Doing");
    expect(c.returns).toBe(1);
  });

  it("does not count forward moves, or cards that belong to no Job", () => {
    const { b, c } = listed("To do");
    moveCard(b, "login", "Doing");
    moveCard(b, "login", "Review");
    expect(c.returns).toBeUndefined();
    const plain = addCard(b, "Done", { title: "plain" });
    moveCard(b, "plain", "Doing");
    expect(plain.returns).toBeUndefined();
  });
});
