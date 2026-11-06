import { describe, expect, it } from "vitest";
import { defaultBoard } from "./board.js";
import { addCard, markDone, moveCard, updateCard } from "./ops.js";
import type { Board } from "./board.js";

describe("a card's Job", () => {
  it("is kept when one is given", () => {
    expect(addCard(defaultBoard(), "To do", { title: "fix bug", job: "J-0007" }).job).toBe("J-0007");
  });
  it("is left off when none is given, or it is blank", () => {
    expect(addCard(defaultBoard(), "To do", { title: "fix bug" })).not.toHaveProperty("job");
    expect(addCard(defaultBoard(), "To do", { title: "fix bug", job: "  " })).not.toHaveProperty("job");
  });
});

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

describe("ticking a checklist", () => {
  const board = () => {
    const b = { lists: [{ id: "l1", title: "Doing", cards: [] }] } as unknown as Board;
    addCard(b, "Doing", { title: "Card", checklist: ["Write it", "Test it"] });
    return b;
  };
  it("ticks by text, ignoring case, and by id", () => {
    const b = board();
    const id = b.lists[0].cards[0].checklist[1].id;
    const c = updateCard(b, "Card", { tick: ["write IT", id] });
    expect(c.checklist.map((i) => i.done)).toEqual([true, true]);
  });
  it("unticks", () => {
    const b = board();
    updateCard(b, "Card", { tick: ["Write it"] });
    expect(updateCard(b, "Card", { untick: ["Write it"] }).checklist[0].done).toBe(false);
  });
  it("names an item it cannot find", () => {
    expect(() => updateCard(board(), "Card", { tick: ["Ship it"] })).toThrow('No checklist item "Ship it" on "Card"');
  });
  it("ticks on the list a replacement just gave", () => {
    const c = updateCard(board(), "Card", { checklist: ["New"], tick: ["New"] });
    expect(c.checklist).toMatchObject([{ text: "New", done: true }]);
  });
});
