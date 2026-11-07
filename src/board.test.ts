import { describe, it, expect } from "vitest";
import {
  defaultBoard,
  normalizeLists,
  normalizeCard,
  findCardIn,
  applyDomOrder,
  doneCardIds,
  markJobStopped,
  mkCard,
  type Board,
  type Card,
} from "./board";

describe("normalizeLists", () => {
  it("returns null for non-board shapes", () => {
    expect(normalizeLists(null)).toBeNull();
    expect(normalizeLists({ todo: [] })).toBeNull();
  });

  it("normalizes sparse cards", () => {
    const b = normalizeLists({ lists: [{ id: "l1", title: "T", cards: [{ id: "c1", title: "x" }] }] })!;
    expect(b.lists[0].cards[0]).toMatchObject({ desc: "", labels: [], due: null, checklist: [] });
  });
});

describe("assignee + done.by", () => {
  it("normalizeCard keeps a string assignee and done.by, drops non-strings", () => {
    const c = normalizeCard({
      title: "t",
      assignee: "Claude Code #1",
      done: { repoRoot: "r", files: ["a.ts"], at: 1, by: "Claude Code #1" },
    } as Partial<Card>);
    expect(c.assignee).toBe("Claude Code #1");
    expect(c.done?.by).toBe("Claude Code #1");
    const bad = normalizeCard({ title: "t", assignee: 42 as unknown as string });
    expect(bad.assignee).toBeUndefined();
  });

  it("legacy cards without assignee still normalize", () => {
    const c = normalizeCard({ title: "old" });
    expect(c.assignee).toBeUndefined();
  });
});

describe("doneCardIds", () => {
  it("collects ids of cards in any list titled Done (case-insensitive)", () => {
    const b: Board = {
      lists: [
        { id: "l1", title: "To do", cards: [normalizeCard({ id: "a", title: "a" })] },
        { id: "l2", title: "DONE", cards: [normalizeCard({ id: "b", title: "b" })] },
      ],
    };
    expect([...doneCardIds(b)]).toEqual(["b"]);
  });
});

describe("findCardIn", () => {
  it("finds a card across lists", () => {
    const b = defaultBoard();
    const c = mkCard("hello");
    b.lists[1].cards.push(c);
    expect(findCardIn(b, c.id)?.list.title).toBe("Doing");
    expect(findCardIn(b, "nope")).toBeNull();
  });
});

describe("applyDomOrder", () => {
  const boardWith = (): { b: Board; ids: Record<string, string> } => {
    const b = defaultBoard();
    const a = mkCard("a");
    const c = mkCard("c");
    b.lists[0].cards.push(a, c);
    return { b, ids: { a: a.id, c: c.id, todo: b.lists[0].id, doing: b.lists[1].id, done: b.lists[2].id } };
  };

  it("applies list and card order from the snapshot", () => {
    const { b, ids } = boardWith();
    applyDomOrder(b, [
      { id: ids.doing, cardIds: [ids.c] },
      { id: ids.todo, cardIds: [ids.a] },
      { id: ids.done, cardIds: [] },
    ]);
    expect(b.lists.map((l) => l.title)).toEqual(["Doing", "To do", "Done"]);
    expect(b.lists[0].cards.map((x) => x.title)).toEqual(["c"]);
  });

  it("keeps a card the snapshot doesn't know, at its home list's end", () => {
    const { b, ids } = boardWith();
    const ghost = mkCard("agent-added");
    b.lists[0].cards.push(ghost); // added externally after the drag started
    applyDomOrder(b, [
      { id: ids.todo, cardIds: [ids.c, ids.a] },
      { id: ids.doing, cardIds: [] },
      { id: ids.done, cardIds: [] },
    ]);
    expect(b.lists[0].cards.map((x) => x.title)).toEqual(["c", "a", "agent-added"]);
  });

  it("keeps a list the snapshot doesn't know", () => {
    const { b, ids } = boardWith();
    b.lists.push({ id: "lnew", title: "Agent list", cards: [] });
    applyDomOrder(b, [
      { id: ids.todo, cardIds: [ids.a, ids.c] },
      { id: ids.doing, cardIds: [] },
      { id: ids.done, cardIds: [] },
    ]);
    expect(b.lists.map((l) => l.title)).toEqual(["To do", "Doing", "Done", "Agent list"]);
  });
});

describe("markJobStopped", () => {
  const c = (id: string, job?: string) => ({ id, title: id, desc: "", labels: [], due: null, checklist: [], job });
  const board = (): Board => ({ lists: [{ id: "a", title: "To do", cards: [c("1", "J1"), c("2", "J2")] }, { id: "b", title: "Done", cards: [c("3", "J1")] }] });
  it("marks the Job's open cards and counts them, leaving Done and other Jobs", () => {
    const b = board();
    expect(markJobStopped(b, "J1")).toBe(1);
    expect(b.lists[0].cards.map((x) => !!x.stopped)).toEqual([true, false]);
    expect(b.lists[1].cards[0].stopped).toBeUndefined();
  });
  it("counts nothing the second time, so a board with no change is not written", () => {
    const b = board();
    markJobStopped(b, "J1");
    expect(markJobStopped(b, "J1")).toBe(0);
  });
  it("makes a checklist plain, whatever the file holds", () => {
    const hostile = `"><img src=x onerror=alert(1)>`;
    const c = normalizeCard({ title: "t", checklist: [
      { id: "i1", text: "ok", done: hostile as unknown as boolean },
      { id: "i2", text: "yes", done: true },
      { id: 7 as unknown as string, text: "no id", done: false },
      { id: "i4", text: 5 as unknown as string, done: true },
      null as never,
    ] });
    expect(c.checklist.map((i) => [i.text, i.done])).toEqual([["ok", false], ["yes", true], ["no id", false]]);
    expect(typeof c.checklist[2].id).toBe("string");
  });
  it("survives a read of the file", () => {
    const b = board();
    markJobStopped(b, "J1");
    expect(normalizeCard(JSON.parse(JSON.stringify(b.lists[0].cards[0]))).stopped).toBe(true);
  });
});
