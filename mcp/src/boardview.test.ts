import { describe, expect, it } from "vitest";
import { normalizeBoard } from "./board.js";
import { boardText, cardView, DESC_SHOWN, DONE_SHOWN } from "./boardview.js";

const done = (n: number) => ({ id: `d${n}`, title: `finished ${n}`, done: { repoRoot: "r", files: ["a.ts", "b.ts"], at: n * 86_400_000, by: "Ana" } });

const board = normalizeBoard({
  lists: [
    {
      id: "l1",
      title: "Doing",
      cards: [
        {
          id: "c1",
          title: "Record video",
          desc: "x".repeat(DESC_SHOWN + 50),
          labels: ["blue"],
          assignee: "Bob",
          checklist: [
            { id: "i1", text: "capture", done: true },
            { id: "i2", text: "encode", done: false },
          ],
        },
      ],
    },
    { id: "l2", title: "Done", cards: Array.from({ length: DONE_SHOWN + 3 }, (_, i) => done(i + 1)) },
  ],
});

describe("the short board view", () => {
  const text = boardText(board);

  it("keeps every open card with the ids an agent acts on", () => {
    expect(text).toContain("## Doing [l1] (1)");
    expect(text).toContain("[c1] Record video · @Bob · blue · 1/2");
    expect(text).toContain("- [x] i1 capture");
    expect(text).toContain("- [ ] i2 encode");
  });

  it("cuts a long description and says where the rest is", () => {
    expect(text).toContain("… (board_get card for all of it)");
    expect(text).not.toContain("x".repeat(DESC_SHOWN + 1));
  });

  it("shows the newest finished cards by title, no file lists", () => {
    expect(text).toContain(`[d${DONE_SHOWN + 3}] finished ${DONE_SHOWN + 3} (Ana, `);
    expect(text).not.toContain("[d1] ");
    expect(text).toContain("- … 3 older (board_get all for them)");
    expect(text).not.toContain("a.ts");
  });

  it("gives every finished card and the whole description when asked", () => {
    const all = boardText(board, true);
    expect(all).toContain("[d1] finished 1");
    expect(all).toContain("x".repeat(DESC_SHOWN + 50));
    expect(all).not.toContain("older");
  });

  it("is far shorter than the file it reads", () => {
    expect(text.length).toBeLessThan(JSON.stringify(board, null, 2).length / 2);
  });
});

describe("one card", () => {
  it("comes whole, with its list", () => {
    const c = board.lists[1].cards[0];
    expect(cardView(board, c)).toEqual({ list: "Done", card: c });
  });
});
